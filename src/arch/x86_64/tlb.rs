// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Synchronous invalidation for the shared host stage-1 hierarchy.
//!
//! Kernel memory policy serializes hierarchy mutation. This module supplies the
//! architecture half of that contract: a generation is published before fixed
//! IPIs are sent, every online remote CPU reloads CR3, and the initiator waits
//! for release-published acknowledgements before backing memory may be reused.

use core::arch::asm;

use hyper::cpu::{CpuIndex, MAX_CPUS};
use hyper::hal::interrupt::InterruptId;
use hyper::sync::atomic::{AtomicU64, Ordering};

pub(super) const SHOOTDOWN_VECTOR: u32 = 0xf1;

const _: () = assert!(SHOOTDOWN_VECTOR >= 32);
const _: () = assert!(SHOOTDOWN_VECTOR < 0xff);
const _: () = assert!(SHOOTDOWN_VECTOR != super::platform::TIMER_VECTOR);
const _: () = assert!(SHOOTDOWN_VECTOR != super::platform::RESCHEDULE_VECTOR);

static NEXT_GENERATION: AtomicU64 = AtomicU64::new(0);
static REQUESTED_GENERATION: AtomicU64 = AtomicU64::new(0);
static ACKNOWLEDGED_GENERATION: [AtomicU64; MAX_CPUS] = [const { AtomicU64::new(0) }; MAX_CPUS];
#[cfg(feature = "kernel-self-test")]
static COMPLETED_SHOOTDOWNS: AtomicU64 = AtomicU64::new(0);

/// Invalidates translations from the shared host hierarchy on every online CPU.
///
/// The caller must serialize this operation with every other stage-1 mutation.
/// The routine deliberately remains synchronous even when the caller has local
/// IRQs masked: remote handlers require no kernel lock, allocation, or callback.
pub(super) fn flush_all_online() {
    let current = current_cpu_or_halt();
    let previous = NEXT_GENERATION.fetch_add(1, Ordering::Relaxed);
    if previous == u64::MAX {
        // Generation zero is the unacknowledged initial state and cannot be
        // reused without making a stale acknowledgement appear current.
        super::halt()
    }
    let generation = previous + 1;

    // The release publishes all serialized page-table writes. send_fixed_ipi
    // adds Intel's required MFENCE;LFENCE ordering before the x2APIC ICR WRMSR.
    REQUESTED_GENERATION.store(generation, Ordering::Release);
    flush_local();

    let mut targets = [false; MAX_CPUS];
    super::smp::for_each_online_remote_cpu(current, |cpu, apic_id| {
        targets[cpu.get()] = true;
        if !super::interrupt_controller::send_fixed_ipi(apic_id, InterruptId::new(SHOOTDOWN_VECTOR))
        {
            // The compile-time vector checks and validated APIC route make this
            // unreachable. Continuing would permit use-after-unmap through a
            // stale translation, so fail closed if the invariant is violated.
            super::halt()
        }
    });

    for (cpu, targeted) in targets.iter().copied().enumerate() {
        if !targeted {
            continue;
        }
        while ACKNOWLEDGED_GENERATION[cpu].load(Ordering::Acquire) != generation {
            core::hint::spin_loop();
        }
    }
    #[cfg(feature = "kernel-self-test")]
    COMPLETED_SHOOTDOWNS.fetch_add(1, Ordering::Release);
}

#[cfg(feature = "kernel-self-test")]
pub(super) fn completed_count_for_test() -> u64 {
    COMPLETED_SHOOTDOWNS.load(Ordering::Acquire)
}

/// Joins a CPU to the online set without racing a concurrent shootdown snapshot.
///
/// The architecture online flag must be release-published before this call. If
/// an initiator already observed that flag, it will wait for either this local
/// acknowledgement or the subsequently delivered IPI. If it did not, this
/// local flush still observes and completes the latest published generation.
pub(super) fn synchronize_online_cpu() {
    let cpu = current_cpu_or_halt();
    let generation = REQUESTED_GENERATION.load(Ordering::Acquire);
    flush_local();
    ACKNOWLEDGED_GENERATION[cpu.get()].store(generation, Ordering::Release);
}

/// Services a request synchronously while ordinary interrupt delivery is masked.
///
/// This is the progress hook for contended IRQ-safe locks. It is intentionally
/// limited to atomic generation access and local CR3 invalidation.
pub(super) fn service_pending() {
    let Some(cpu) = CpuIndex::new(super::smp::current_cpu_index()) else {
        return;
    };
    let generation = REQUESTED_GENERATION.load(Ordering::Acquire);
    if ACKNOWLEDGED_GENERATION[cpu.get()].load(Ordering::Relaxed) == generation {
        return;
    }
    flush_local();
    ACKNOWLEDGED_GENERATION[cpu.get()].store(generation, Ordering::Release);
}

/// Consumes the architecture-private shootdown vector before kernel IRQ policy.
pub(super) fn handle_interrupt(vector: u32) -> bool {
    if vector != SHOOTDOWN_VECTOR {
        return false;
    }

    service_pending();
    super::interrupt_controller::end_local_interrupt();
    true
}

fn current_cpu_or_halt() -> CpuIndex {
    let Some(cpu) = CpuIndex::new(super::smp::current_cpu_index()) else {
        super::halt()
    };
    cpu
}

fn flush_local() {
    let root: usize;
    // SAFETY: Reading and rewriting the active CR3 is valid at CPL0. This
    // kernel does not mark host mappings global and does not enable PCID, so the
    // reload invalidates all translations and paging-structure-cache entries
    // associated with the shared hierarchy. The assembly is also a compiler
    // memory boundary for surrounding page-table publication and acknowledgement.
    unsafe { asm!("mov {}, cr3", out(reg) root, options(nostack)) };
    // SAFETY: `root` is the active valid CR3 value read immediately above.
    unsafe { asm!("mov cr3, {}", in(reg) root, options(nostack)) };
}
