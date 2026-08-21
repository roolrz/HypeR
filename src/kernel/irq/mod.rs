// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Exception, interrupt-domain, and kernel-timer policy.

use hyper::sync::atomic::{AtomicBool, Ordering};

pub(crate) mod exception;
pub mod interrupt;
pub mod timer;

pub use interrupt::acknowledge_external;

static EXCEPTIONS_READY: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InitializationError {
    Controller(interrupt::Error),
    GuestRegisters(crate::arch::vm::RegisterValidationError),
    MissingController,
    MissingTimer,
    RuntimeVectors(crate::arch::exception::RuntimeVectorError),
    Timer(timer::Error),
    Virtualization(crate::arch::vm::InterruptInitializationError),
}

/// Initializes the root interrupt controller and kernel IRQ domain.
pub(crate) fn initialize_controller(
    boot: &mut super::boot::Initialization,
) -> Result<(), InitializationError> {
    let info = boot
        .essential()
        .interrupt_controller()
        .ok_or(InitializationError::MissingController)?;
    let capabilities = interrupt::initialize(info).map_err(InitializationError::Controller)?;
    crate::println!(
        "HypeR: interrupt controller initialized with {} interrupt IDs; local IRQs remain masked",
        capabilities.interrupt_count
    );
    boot.set_interrupts(capabilities);
    Ok(())
}

/// Installs the permanent exception vectors and validates guest trap handling.
pub(crate) fn initialize_exceptions() -> Result<(), InitializationError> {
    // SAFETY: The final RX kernel mapping, stack, console, and interrupt
    // controller are active. IRQ delivery remains masked until timer setup.
    unsafe { crate::arch::exception::install_runtime_vectors() };
    crate::arch::exception::validate_runtime_vectors()
        .map_err(InitializationError::RuntimeVectors)?;
    crate::arch::vm::validate_register_interface().map_err(InitializationError::GuestRegisters)?;
    EXCEPTIONS_READY.store(true, Ordering::Release);
    crate::println!("HypeR: guest synchronous trap and vSysReg emulation validated");
    Ok(())
}

pub(crate) fn exceptions_ready() -> bool {
    EXCEPTIONS_READY.load(Ordering::Acquire)
}

/// Activates the interrupt-controller virtualization backend.
pub(crate) fn initialize_virtualization(
    boot: &super::boot::Initialization,
) -> Result<(), InitializationError> {
    let interrupts = boot.interrupts();
    crate::arch::vm::initialize_interrupts(interrupts.root_domain, interrupts.maintenance_interrupt)
        .map_err(InitializationError::Virtualization)
}

/// Starts the periodic kernel timer and publishes its guest-visible mapping.
pub(crate) fn initialize_timer(
    boot: &mut super::boot::Initialization,
) -> Result<(), InitializationError> {
    let info = boot
        .essential()
        .timer()
        .ok_or(InitializationError::MissingTimer)?;
    let capabilities = timer::initialize(info, boot.interrupts().root_domain)
        .map_err(InitializationError::Timer)?;
    crate::println!(
        "HypeR: architectural timer: host INTID {}, guest INTID {} (host VIRQ {}), {} Hz tick from a {} Hz counter",
        capabilities.hardware_interrupt.get(),
        capabilities.guest_virtual_interrupt.get(),
        capabilities.guest_virtual_host_interrupt.get(),
        capabilities.ticks_per_second,
        capabilities.counter_frequency_hz
    );
    crate::println!(
        "HypeR: timer mapped to dynamic VIRQ {}",
        capabilities.virtual_interrupt.get()
    );
    crate::println!("HypeR: dynamically owned per-CPU software timer queues active");
    boot.set_timer(capabilities);
    Ok(())
}

/// Updates IRQ/timer policy after all secondary CPUs are online.
pub(crate) fn publish_online_cpu_count(
    boot: &super::boot::Initialization,
) -> Result<(), InitializationError> {
    timer::set_online_cpu_count(boot.cpus().online_cpus).map_err(InitializationError::Timer)
}
