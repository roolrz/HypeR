//! Fatal exception policy above architecture vector decoding.

use core::fmt;

use hyper::hal::exception::ExceptionReport;
use hyper::hal::interrupt::InterruptId;
use hyper::sync::atomic::{AtomicBool, Ordering};

static FATAL_PATH_ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn fatal(report: ExceptionReport) -> ! {
    claim_fatal_path();
    match crate::kernel::debug::kallsyms::lookup(report.instruction_pointer as usize) {
        Ok(Some(symbol)) => emit_and_halt(format_args!(
            "HypeR: fatal exception: kind {:?}, origin {:?}, class {:#x} ({}), syndrome {:#x}, IP {:#x} <{}+{:#x}>, FAR {:#x}, status {:#x}, SP {:#x}",
            report.kind,
            report.origin,
            report.architecture_class,
            report.description,
            report.syndrome,
            report.instruction_pointer,
            symbol.name,
            symbol.offset,
            report.fault_address_register,
            report.status,
            report.stack_pointer
        )),
        Ok(None) | Err(_) => emit_and_halt(format_args!(
            "HypeR: fatal exception: kind {:?}, origin {:?}, class {:#x} ({}), syndrome {:#x}, IP {:#x}, FAR {:#x}, status {:#x}, SP {:#x}",
            report.kind,
            report.origin,
            report.architecture_class,
            report.description,
            report.syndrome,
            report.instruction_pointer,
            report.fault_address_register,
            report.status,
            report.stack_pointer
        )),
    }
}

pub fn fatal_invalid_vector(
    vector: u64,
    syndrome: u64,
    instruction_pointer: u64,
    fault_address: u64,
    status: u64,
) -> ! {
    enter_fatal(format_args!(
        "HypeR: invalid exception vector {vector}, syndrome {syndrome:#x}, IP {instruction_pointer:#x}, FAR {fault_address:#x}, status {status:#x}"
    ))
}

pub fn fatal_interrupt(reason: &str, interrupt: Option<InterruptId>) -> ! {
    match interrupt {
        Some(interrupt) => enter_fatal(format_args!(
            "HypeR: fatal IRQ dispatch: {reason}; INTID {}",
            interrupt.get()
        )),
        None => enter_fatal(format_args!("HypeR: fatal IRQ dispatch: {reason}")),
    }
}

pub fn fatal_timer(error: crate::arch::TimerError) -> ! {
    enter_fatal(format_args!("HypeR: fatal timer interrupt: {error:?}"))
}

pub fn fatal_interrupt_state(error: super::interrupt::Error) -> ! {
    enter_fatal(format_args!(
        "HypeR: fatal interrupt-controller state: {error:?}"
    ))
}

fn enter_fatal(arguments: fmt::Arguments<'_>) -> ! {
    claim_fatal_path();
    emit_and_halt(arguments)
}

fn claim_fatal_path() {
    if FATAL_PATH_ACTIVE.swap(true, Ordering::AcqRel) {
        crate::arch::halt()
    }
}

fn emit_and_halt(arguments: fmt::Arguments<'_>) -> ! {
    crate::kernel::log::emergency(arguments);
    crate::arch::halt()
}
