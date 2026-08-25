// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Typed kernel RPC transport over one architecture-reserved doorbell.
//!
//! The mailbox is allocation-free and generation tagged. A route rejection or
//! missing acknowledgement is ambiguous, so the transport poisons itself and
//! enters fail-stop instead of reusing payload storage under a late handler.

use hyper::cpu::{CpuIndex, PerCpu};
use hyper::hal::interrupt::{InterruptId, InterruptPriority, InterruptTrigger, KernelRpcReasons};
use hyper::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};

const TIMEOUT_NS: u64 = 1_000_000_000;
const IDLE: u8 = 0;
const PENDING: u8 = 1;
const WORKING: u8 = 2;
const APPLIED: u8 = 3;
const REJECTED: u8 = 4;

static OWNER: AtomicBool = AtomicBool::new(false);
static READY: AtomicBool = AtomicBool::new(false);
static POISONED: AtomicBool = AtomicBool::new(false);
static GENERATION: AtomicU32 = AtomicU32::new(0);
static OPCODE: AtomicU8 = AtomicU8::new(0);
static ARGUMENT0: AtomicU32 = AtomicU32::new(0);
static ARGUMENT1: AtomicU8 = AtomicU8::new(0);
static ARGUMENT2: AtomicU8 = AtomicU8::new(0);
static STATUS: PerCpu<AtomicU8> =
    PerCpu::new([const { AtomicU8::new(IDLE) }; hyper::cpu::MAX_CPUS]);
static ACK: PerCpu<AtomicU32> = PerCpu::new([const { AtomicU32::new(0) }; hyper::cpu::MAX_CPUS]);

#[derive(Clone, Copy)]
pub(crate) enum LocalIrqOperation {
    Configure,
    Enable,
    Disable,
}

#[derive(Clone, Copy)]
pub(crate) enum KernelRpc {
    LocalIrqLifecycle {
        hardware: InterruptId,
        priority: InterruptPriority,
        trigger: InterruptTrigger,
        operation: LocalIrqOperation,
    },
}

pub(crate) struct Outcome {
    pub(crate) rejected_cpu: Option<usize>,
}

struct Owner;

impl Owner {
    fn acquire() -> Result<Self, ()> {
        if POISONED.load(Ordering::Acquire) {
            return Err(());
        }
        OWNER
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .map(|_| Self)
            .map_err(|_| ())
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        READY.store(false, Ordering::Release);
        OWNER.store(false, Ordering::Release);
    }
}

pub(crate) fn execute(
    rpc: KernelRpc,
    count: usize,
    targets: &[bool; hyper::cpu::MAX_CPUS],
) -> Result<Outcome, ()> {
    let _owner = Owner::acquire()?;
    let generation = match GENERATION.load(Ordering::Relaxed).checked_add(1) {
        Some(generation) if generation != 0 => generation,
        _ => poison("kernel RPC generation exhausted"),
    };
    publish(rpc);
    GENERATION.store(generation, Ordering::Relaxed);
    for (cpu, targeted) in targets.iter().copied().enumerate() {
        let Some(cpu) = CpuIndex::new(cpu) else {
            continue;
        };
        STATUS[cpu].store(
            if cpu.get() < count && targeted {
                PENDING
            } else {
                IDLE
            },
            Ordering::Relaxed,
        );
        ACK[cpu].store(0, Ordering::Relaxed);
    }
    READY.store(true, Ordering::Release);
    service_local_irq_mailbox();

    let current = crate::kernel::cpu::current_index();
    for (cpu, targeted) in targets.iter().copied().enumerate().take(count) {
        let Some(cpu) = CpuIndex::new(cpu) else {
            continue;
        };
        if !targeted || Some(cpu) == current {
            continue;
        }
        if !crate::hal::irq::notify_kernel_rpc(cpu, KernelRpcReasons::LOCAL_IRQ_LIFECYCLE) {
            poison("kernel RPC route rejected");
        }
    }
    match crate::kernel::time::spin_wait_until(TIMEOUT_NS, || {
        service();
        targets
            .iter()
            .copied()
            .enumerate()
            .take(count)
            .all(|(index, targeted)| {
                !targeted
                    || CpuIndex::new(index)
                        .is_some_and(|cpu| ACK[cpu].load(Ordering::Acquire) == generation)
            })
    }) {
        Ok(true) => {}
        Ok(false) | Err(_) => poison("kernel RPC acknowledgement timed out"),
    }
    READY.store(false, Ordering::Release);
    let mut rejected_cpu = None;
    for (index, targeted) in targets.iter().copied().enumerate().take(count) {
        if !targeted {
            continue;
        }
        let Some(cpu) = CpuIndex::new(index) else {
            continue;
        };
        match STATUS[cpu].load(Ordering::Acquire) {
            APPLIED => {}
            REJECTED if rejected_cpu.is_none() => rejected_cpu = Some(index),
            _ => poison("kernel RPC completion state is inconsistent"),
        }
    }
    Ok(Outcome { rejected_cpu })
}

pub(crate) fn service() {
    loop {
        let reasons = crate::hal::irq::take_kernel_rpc_reasons();
        if reasons.has_unknown() {
            poison("kernel RPC reason mailbox contains unknown bits");
        }
        if reasons.is_empty() {
            return;
        }
        if reasons.contains(KernelRpcReasons::STAGE1_TLB_SHOOTDOWN) {
            let _ = crate::hal::memory::service_stage1_tlb_shootdown();
        }
        if reasons.contains(KernelRpcReasons::LOCAL_IRQ_LIFECYCLE) {
            service_local_irq_mailbox();
        }
    }
}

fn service_local_irq_mailbox() {
    if !READY.load(Ordering::Acquire) {
        return;
    }
    let Some(cpu) = crate::kernel::cpu::current_index() else {
        return;
    };
    if STATUS[cpu]
        .compare_exchange(PENDING, WORKING, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        return;
    }
    let generation = GENERATION.load(Ordering::Acquire);
    let applied = match decode() {
        KernelRpc::LocalIrqLifecycle {
            hardware,
            priority,
            trigger,
            operation,
        } => super::interrupt::apply_local_irq_lifecycle(hardware, priority, trigger, operation),
    };
    STATUS[cpu].store(if applied { APPLIED } else { REJECTED }, Ordering::Relaxed);
    ACK[cpu].store(generation, Ordering::Release);
}

fn publish(rpc: KernelRpc) {
    match rpc {
        KernelRpc::LocalIrqLifecycle {
            hardware,
            priority,
            trigger,
            operation,
        } => {
            ARGUMENT0.store(hardware.get(), Ordering::Relaxed);
            ARGUMENT1.store(encode_priority(priority), Ordering::Relaxed);
            ARGUMENT2.store(encode_trigger(trigger), Ordering::Relaxed);
            OPCODE.store(encode_operation(operation), Ordering::Relaxed);
        }
    }
}

fn decode() -> KernelRpc {
    let opcode = OPCODE.load(Ordering::Relaxed);
    KernelRpc::LocalIrqLifecycle {
        hardware: InterruptId::new(ARGUMENT0.load(Ordering::Relaxed)),
        priority: decode_priority(ARGUMENT1.load(Ordering::Relaxed)),
        trigger: if ARGUMENT2.load(Ordering::Relaxed) == 0 {
            InterruptTrigger::Level
        } else {
            InterruptTrigger::Edge
        },
        operation: match opcode {
            0 => LocalIrqOperation::Configure,
            1 => LocalIrqOperation::Enable,
            _ => LocalIrqOperation::Disable,
        },
    }
}

fn poison(reason: &'static str) -> ! {
    POISONED.store(true, Ordering::Release);
    crate::kernel::crash::fatal(format_args!(
        "HypeR: poisoned kernel RPC transport: {reason}"
    ))
}

const fn encode_priority(priority: InterruptPriority) -> u8 {
    match priority {
        InterruptPriority::Critical => 0,
        InterruptPriority::High => 1,
        InterruptPriority::Normal => 2,
        InterruptPriority::Low => 3,
    }
}

const fn decode_priority(priority: u8) -> InterruptPriority {
    match priority {
        0 => InterruptPriority::Critical,
        1 => InterruptPriority::High,
        2 => InterruptPriority::Normal,
        _ => InterruptPriority::Low,
    }
}

const fn encode_trigger(trigger: InterruptTrigger) -> u8 {
    match trigger {
        InterruptTrigger::Level => 0,
        InterruptTrigger::Edge => 1,
    }
}

const fn encode_operation(operation: LocalIrqOperation) -> u8 {
    match operation {
        LocalIrqOperation::Configure => 0,
        LocalIrqOperation::Enable => 1,
        LocalIrqOperation::Disable => 2,
    }
}
