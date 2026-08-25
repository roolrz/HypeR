// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Host exception entry, interrupt domains, and cross-call transport.
//!
//! Device and timer subsystems own their interrupt handlers and lifecycle;
//! this module supplies the controller, routing, and dispatch mechanisms they
//! consume.

use hyper::sync::atomic::{AtomicBool, Ordering};

pub(crate) mod cross_call;
pub(crate) mod exception;
pub mod interrupt;
pub(crate) mod reschedule;

pub use interrupt::acknowledge_external;

static EXCEPTIONS_READY: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InitializationError {
    Controller(interrupt::Error),
    KernelRpcService(crate::hal::irq::KernelRpcServiceError),
    MissingController,
    RuntimeVectors(crate::hal::exception::RuntimeVectorError),
    Reschedule(reschedule::Error),
}

/// Initializes host interrupt delivery before interrupt consumers are started.
///
/// The scheduler already owns its per-CPU pending state at this point. This
/// subsystem owns the transport lifecycle: controller publication, runtime
/// exception entry, and the permanent reschedule cross-call mapping. Local
/// interrupts remain masked and no secondary CPU is admitted until all three
/// stages are ready.
pub(crate) fn initialize(
    boot: &mut super::boot::Initialization,
) -> Result<(), InitializationError> {
    initialize_controller(boot)?;
    crate::hal::irq::install_kernel_rpc_service(cross_call::service)
        .map_err(InitializationError::KernelRpcService)?;
    interrupt::initialize_local_rpc_transport().map_err(InitializationError::Controller)?;
    initialize_exceptions()?;
    reschedule::initialize(boot.interrupts().root_domain).map_err(InitializationError::Reschedule)
}

/// Observes reschedule-SGI dispatch for the bare-metal runtime proof.
#[cfg(all(
    feature = "kernel-self-test",
    any(CONFIG_ARCH_AARCH64, CONFIG_ARCH_X86_64)
))]
pub(crate) fn reschedule_delivery_count_for_test(cpu: hyper::cpu::CpuIndex) -> usize {
    reschedule::delivery_count_for_test(cpu)
}

/// Initializes the root interrupt controller and kernel IRQ domain.
fn initialize_controller(
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

/// Installs and validates the permanent runtime exception vectors.
fn initialize_exceptions() -> Result<(), InitializationError> {
    // SAFETY: The final RX kernel mapping, stack, console, and interrupt
    // controller are active. IRQ delivery remains masked until runtime activation.
    unsafe { crate::hal::exception::install_runtime_vectors() };
    crate::hal::exception::validate_runtime_vectors()
        .map_err(InitializationError::RuntimeVectors)?;
    EXCEPTIONS_READY.store(true, Ordering::Release);
    Ok(())
}

pub(crate) fn exceptions_ready() -> bool {
    EXCEPTIONS_READY.load(Ordering::Acquire)
}
