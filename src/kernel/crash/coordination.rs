// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Fatal-crash ownership, CPU stopping, and emergency-stack transitions.
//!
//! This module owns the fail-stop control flow. It never allocates, blocks on
//! a kernel lock, or enters architecture exception decoding. Snapshot storage,
//! diagnostic rendering, frame walking, and optional monitor commands remain
//! in sibling modules. Local interrupts are masked before every publication.

use core::fmt::{self, Write};
use core::hint::spin_loop;
use core::panic::PanicInfo;

use hyper::hal::interrupt::{InterruptId, InterruptPriority, InterruptTrigger};

use crate::hal::exception::CrashContext;

const STOP_WAIT_TIMEOUT_NS: u64 = 100_000_000;
const STOP_WAIT_FALLBACK_ITERATIONS: usize = 10_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InitializationError {
    Interrupt(super::super::irq::interrupt::Error),
    Prerequisite(Prerequisite),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Prerequisite {
    Debug,
    Memory,
    RuntimeExceptions,
    Scheduler,
}

/// Reserves and installs the all-but-self crash-stop interrupt.
pub(crate) fn initialize(
    boot: &super::super::boot::Initialization,
) -> Result<(), InitializationError> {
    validate_prerequisites()?;
    super::console::initialize();

    let Some(hardware_interrupt) = crate::hal::exception::crash_stop_interrupt() else {
        super::state::mark_ready();
        crate::println!("HypeR: crash-stop cross-call is unavailable on this platform");
        return Ok(());
    };
    let (_, registration) = boot
        .interrupts()
        .root_domain
        .register_shared_mapping(
            hardware_interrupt,
            InterruptPriority::Critical,
            InterruptTrigger::Edge,
            0,
            crash_stop_interrupt,
        )
        .map_err(InitializationError::Interrupt)?;
    // Fatal-crash coordination remains active until power-off. There is no
    // shutdown stage in which this handler can be safely removed.
    registration.retain_permanently();
    super::state::mark_ipi_ready();
    super::state::mark_ready();
    crate::println!("HypeR: crash-stop IPI and CPU state capture initialized");
    Ok(())
}

fn validate_prerequisites() -> Result<(), InitializationError> {
    let missing = if !super::super::mm::is_ready() {
        Some(Prerequisite::Memory)
    } else if !super::super::debug::is_ready() {
        Some(Prerequisite::Debug)
    } else if !super::super::task::is_ready() {
        Some(Prerequisite::Scheduler)
    } else if !super::super::irq::exceptions_ready() {
        Some(Prerequisite::RuntimeExceptions)
    } else {
        None
    };
    match missing {
        Some(prerequisite) => Err(InitializationError::Prerequisite(prerequisite)),
        None => Ok(()),
    }
}

/// Reports whether fatal failures can safely enter the coordinated crash path.
pub(crate) fn is_ready() -> bool {
    super::state::is_ready()
}

fn crash_stop_interrupt(
    _interrupt: super::super::irq::interrupt::VirtualInterrupt,
    _context: usize,
) -> super::super::irq::interrupt::HandlerResult {
    super::super::irq::interrupt::HandlerResult::Handled
}

pub fn is_stop_interrupt(interrupt: InterruptId) -> bool {
    super::state::ipi_ready() && crate::hal::exception::is_crash_stop_interrupt(interrupt)
}

/// Handles a Rust panic without delegating crash policy to the log subsystem.
#[unsafe(export_name = "kernel_crash_panic")]
pub fn panic(info: &PanicInfo<'_>) -> ! {
    enter(
        crate::hal::exception::capture_crash_context(),
        format_args!("Kernel panic - not syncing: {info}"),
    )
}

/// Handles fatal kernel failures that do not already carry an exception frame.
pub(crate) fn fatal(arguments: fmt::Arguments<'_>) -> ! {
    enter(crate::hal::exception::capture_crash_context(), arguments)
}

/// Handles a fatal failure that already has an architecture snapshot.
pub(crate) fn fatal_context(context: CrashContext, arguments: fmt::Arguments<'_>) -> ! {
    enter(context, arguments)
}

/// Publishes a remote CPU's exact IRQ frame and permanently stops that CPU.
pub fn stop_this_cpu(context: CrashContext) -> ! {
    let mut payload = StopPayload { context };
    // SAFETY: payload remains live on the abandoned stack and the callback
    // never returns after switching to the per-CPU emergency stack.
    unsafe {
        crate::hal::exception::run_on_emergency_stack(
            stop_this_cpu_on_emergency_stack,
            (&mut payload as *mut StopPayload) as usize,
        )
    }
}

struct StopPayload {
    context: CrashContext,
}

extern "C" fn stop_this_cpu_on_emergency_stack(argument: usize) -> ! {
    // SAFETY: stop_this_cpu passes one live payload and the callback never
    // returns to outlive it.
    let payload = unsafe { &*(argument as *const StopPayload) };
    crate::hal::irq::disable_all_sources();
    publish_current_cpu(payload.context);
    super::state::mark_cpu_stopped();
    crate::hal::cpu::halt()
}

fn enter(context: CrashContext, reason: fmt::Arguments<'_>) -> ! {
    let mut owned_reason = super::state::CrashReason::new();
    let _ = owned_reason.write_fmt(reason);
    crate::hal::irq::disable_all_sources();
    let Some(cpu) = super::super::cpu::current_index() else {
        crate::hal::cpu::halt();
    };
    let payload = super::state::CrashPayload::new(context, owned_reason);
    let Some(argument) = super::state::publish_payload(cpu, payload) else {
        if is_ready() {
            super::super::log::emergency(format_args!(
                "RECURSIVE KERNEL PANIC on CPU {}; payload slot already occupied",
                cpu.get()
            ));
        }
        crate::hal::cpu::halt();
    };
    if !is_ready() {
        // Keep the failing CPU at a stable inspection point. In particular,
        // do not touch the console: an invalid device mapping may be the
        // reason this panic was raised.
        super::state::publish_context(cpu, context);
        super::state::mark_early_stopped();
        crate::hal::cpu::halt();
    }
    // SAFETY: The per-CPU static payload remains immutable after publication,
    // and fatal handling permanently owns control after switching stacks.
    unsafe { crate::hal::exception::run_on_emergency_stack(enter_on_emergency_stack, argument) }
}

extern "C" fn enter_on_emergency_stack(argument: usize) -> ! {
    // SAFETY: enter passes the unchanged argument returned by publish_payload,
    // and this callback never returns beyond the permanent slot lifetime.
    let payload = unsafe { super::state::payload_from_argument(argument) };
    crate::hal::irq::disable_all_sources();
    let Some(cpu) = super::super::cpu::current_index() else {
        crate::hal::cpu::halt();
    };
    match super::state::claim_owner(cpu) {
        super::state::OwnerClaim::Acquired => {}
        super::state::OwnerClaim::OwnedByOther => {
            super::state::publish_context(cpu, *payload.context());
            super::state::mark_cpu_stopped();
            crate::hal::cpu::halt()
        }
        super::state::OwnerClaim::Recursive => {
            super::super::log::emergency(format_args!(
                "RECURSIVE KERNEL PANIC on CPU {}; diagnostics aborted",
                cpu.get()
            ));
            super::super::log::emergency(format_args!(
                "recursive context: vector {:#x}, syndrome {:#x}, PC {:#x}, fault address {:#x}",
                payload.context().exception_vector,
                payload.context().syndrome,
                payload.context().program_counter,
                payload.context().fault_address
            ));
            crate::hal::cpu::halt()
        }
    }

    super::super::log::enter_emergency_mode();
    super::state::publish_context(cpu, *payload.context());
    let stop = stop_other_cpus();
    super::report::emit_banner(cpu.get(), payload.reason(), stop);
    super::report::dump_cpu_states(cpu.get());
    super::console::run(cpu.get(), stop);
    super::super::log::emergency(format_args!(
        "---[ end HypeR kernel panic - system halted ]---"
    ));
    crate::hal::cpu::halt()
}

fn publish_current_cpu(context: CrashContext) {
    if let Some(cpu) = super::super::cpu::current_index() {
        super::state::publish_context(cpu, context);
    }
}

fn stop_other_cpus() -> super::report::StopSummary {
    let expected = super::super::cpu::online_cpu_count().saturating_sub(1);
    let sent =
        expected != 0 && super::state::ipi_ready() && crate::hal::exception::broadcast_crash_stop();
    if sent
        && super::super::time::spin_wait_until(STOP_WAIT_TIMEOUT_NS, || {
            super::state::stopped_cpu_count() >= expected
        })
        .is_err()
    {
        // A crash can occur after crash vectors are ready but before the
        // kernel timekeeper is published. Retain a bounded last-resort
        // wait for that narrow initialization window.
        for _ in 0..STOP_WAIT_FALLBACK_ITERATIONS {
            if super::state::stopped_cpu_count() >= expected {
                break;
            }
            spin_loop();
        }
    }
    super::report::StopSummary {
        expected,
        stopped: super::state::stopped_cpu_count().min(expected),
        sent,
    }
}
