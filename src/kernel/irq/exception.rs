//! Fatal exception policy above architecture vector decoding.

use core::fmt;

use hyper::hal::exception::ExceptionReport;
use hyper::hal::interrupt::InterruptId;

pub fn fatal(report: ExceptionReport, context: crate::arch::CrashContext) -> ! {
    crate::kernel::crash::fatal_exception(report, context)
}

pub fn fatal_invalid_vector(
    vector: u64,
    syndrome: u64,
    instruction_pointer: u64,
    fault_address: u64,
    status: u64,
    context: crate::arch::CrashContext,
) -> ! {
    crate::kernel::crash::fatal_context(
        context,
        format_args!(
            "invalid exception vector {vector}, ESR {syndrome:#x}, IP {instruction_pointer:#x}, FAR {fault_address:#x}, PSTATE {status:#x}"
        ),
    )
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
    crate::kernel::crash::fatal(arguments)
}
