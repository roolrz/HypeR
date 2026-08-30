// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Selected-architecture hardware virtualization mechanisms.
//!
//! Kernel VM policy owns VM publication, vCPU scheduling, demand paging, and
//! exit disposition. This facade selects stage-2 translation, vCPU entry,
//! virtual interrupt, guest timer, and architecture-local exit mechanisms.
//! Linux image formats and boot ABI policy deliberately remain outside it.

use core::cell::UnsafeCell;

use hyper::sync::atomic::{AtomicU8, Ordering};
use hyper::vm::exit::{GuestMemoryFault, MemoryFaultAction};

#[cfg(CONFIG_ARCH_AARCH64)]
use hyper::vm::exit::{MmioAccess, MmioAction};
#[cfg(CONFIG_ARCH_X86_64)]
use hyper::vm::x86::exit::{PendingInterruptAction, PortIoAction, PortIoExit};

const SERVICES_EMPTY: u8 = 0;
const SERVICES_INSTALLING: u8 = 1;
const SERVICES_READY: u8 = 2;

pub use super::imp::{
    VcpuInterruptError, VmInterruptController as InterruptController,
    VmInterruptError as InterruptError,
};

pub(crate) use super::imp::{
    GuestValidationError as RegisterValidationError,
    InterruptVirtualizationError as InterruptInitializationError, Stage2AddressSpace, Stage2Error,
    VcpuContext, VirtualDeviceInitializationError as DeviceError, VirtualInterruptError,
};

#[cfg(any(CONFIG_ARCH_AARCH64, CONFIG_ARCH_RISCV64))]
pub(crate) use super::imp::{GuestSyncAction, GuestSyncExit, handle_guest_sync};

#[cfg(CONFIG_ARCH_AARCH64)]
pub(crate) use super::imp::update_guest_device_interrupt;

pub(crate) use super::imp::{
    handle_guest_virtual_timer_interrupt as handle_virtual_timer_interrupt,
    handle_virtualization_maintenance_interrupt as handle_maintenance_interrupt,
    initialize_interrupt_virtualization as initialize_interrupts,
    initialize_virtual_devices as initialize_devices, interrupt_virtualization_description,
    prepare_interrupts_for_guest_entry as prepare_interrupts_for_entry,
    quiesce_virtual_interrupt_delivery, validate_vsysreg as validate_register_interface,
};

#[cfg(feature = "kernel-self-test")]
pub(crate) use super::imp::guest_execution_available;

pub(crate) use super::imp::{
    activate_vcpu_hardware, deactivate_vcpu_hardware,
    virtualization_maintenance_pending as maintenance_interrupt_pending,
};

#[cfg(CONFIG_ARCH_AARCH64)]
pub(crate) use super::imp::inject_timer_for_validation;

/// Immutable kernel callbacks reachable from architecture-owned guest entry.
///
/// Every callback consumes an owned, fixed-width event. Raw exception frames,
/// VMCS/VMCB state, and architecture completion metadata remain below this
/// facade. The target-specific members exist only where their semantics are
/// real; there is deliberately no universal guest-exit enum.
#[derive(Clone, Copy)]
pub(crate) struct ExitServices {
    memory_fault: fn(GuestMemoryFault) -> MemoryFaultAction,
    #[cfg(CONFIG_ARCH_AARCH64)]
    mmio: fn(MmioAccess) -> MmioAction,
    #[cfg(any(CONFIG_ARCH_AARCH64, CONFIG_ARCH_RISCV64))]
    guest_sync: fn(GuestSyncExit) -> GuestSyncAction,
    #[cfg(CONFIG_ARCH_X86_64)]
    port_io: fn(PortIoExit) -> PortIoAction,
    #[cfg(CONFIG_ARCH_X86_64)]
    pending_interrupt: fn(bool) -> PendingInterruptAction,
}

impl ExitServices {
    #[cfg(CONFIG_ARCH_AARCH64)]
    pub(crate) const fn aarch64(
        memory_fault: fn(GuestMemoryFault) -> MemoryFaultAction,
        mmio: fn(MmioAccess) -> MmioAction,
        guest_sync: fn(GuestSyncExit) -> GuestSyncAction,
    ) -> Self {
        Self {
            memory_fault,
            mmio,
            guest_sync,
        }
    }

    #[cfg(CONFIG_ARCH_RISCV64)]
    pub(crate) const fn riscv64(
        memory_fault: fn(GuestMemoryFault) -> MemoryFaultAction,
        guest_sync: fn(GuestSyncExit) -> GuestSyncAction,
    ) -> Self {
        Self {
            memory_fault,
            guest_sync,
        }
    }

    #[cfg(CONFIG_ARCH_X86_64)]
    pub(crate) const fn x86_64(
        memory_fault: fn(GuestMemoryFault) -> MemoryFaultAction,
        port_io: fn(PortIoExit) -> PortIoAction,
        pending_interrupt: fn(bool) -> PendingInterruptAction,
    ) -> Self {
        Self {
            memory_fault,
            port_io,
            pending_interrupt,
        }
    }
}

struct ExitServiceSlot {
    state: AtomicU8,
    services: UnsafeCell<Option<ExitServices>>,
}

// SAFETY: Installation has one writer. Release publication makes the copied,
// immutable function table visible before architecture entry can be enabled;
// the table is never replaced or removed.
unsafe impl Sync for ExitServiceSlot {}

impl ExitServiceSlot {
    const fn new() -> Self {
        Self {
            state: AtomicU8::new(SERVICES_EMPTY),
            services: UnsafeCell::new(None),
        }
    }

    fn install(&self, services: ExitServices) -> Result<ExitServicesReady, ExitServiceError> {
        self.state
            .compare_exchange(
                SERVICES_EMPTY,
                SERVICES_INSTALLING,
                Ordering::Acquire,
                Ordering::Relaxed,
            )
            .map_err(|_| ExitServiceError::AlreadyInstalled)?;
        // SAFETY: The successful EMPTY -> INSTALLING transition grants the
        // only write, and readers cannot inspect this cell before READY.
        unsafe { *self.services.get() = Some(services) };
        self.state.store(SERVICES_READY, Ordering::Release);
        Ok(ExitServicesReady { _private: () })
    }

    fn services(&self) -> ExitServices {
        if self.state.load(Ordering::Acquire) != SERVICES_READY {
            super::imp::halt()
        }
        // SAFETY: Acquire observed the immutable table published before READY.
        let Some(services) = (unsafe { *self.services.get() }) else {
            super::imp::halt()
        };
        services
    }
}

static EXIT_SERVICES: ExitServiceSlot = ExitServiceSlot::new();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExitServiceError {
    AlreadyInstalled,
}

/// Proof that immutable VM-exit callbacks have been published.
///
/// This is intentionally weaker than complete VM-entry readiness: device,
/// interrupt, and timer initialization still follow service publication.
#[must_use]
pub(crate) struct ExitServicesReady {
    _private: (),
}

pub(crate) fn install_exit_services(
    services: ExitServices,
) -> Result<ExitServicesReady, ExitServiceError> {
    EXIT_SERVICES.install(services)
}

pub(crate) fn dispatch_memory_fault(fault: GuestMemoryFault) -> MemoryFaultAction {
    (EXIT_SERVICES.services().memory_fault)(fault)
}

#[cfg(CONFIG_ARCH_AARCH64)]
pub(crate) fn dispatch_mmio(access: MmioAccess) -> MmioAction {
    (EXIT_SERVICES.services().mmio)(access)
}

#[cfg(any(CONFIG_ARCH_AARCH64, CONFIG_ARCH_RISCV64))]
pub(crate) fn dispatch_guest_sync(exit: GuestSyncExit) -> GuestSyncAction {
    (EXIT_SERVICES.services().guest_sync)(exit)
}

#[cfg(CONFIG_ARCH_X86_64)]
pub(crate) fn dispatch_port_io(exit: PortIoExit) -> PortIoAction {
    (EXIT_SERVICES.services().port_io)(exit)
}

#[cfg(CONFIG_ARCH_X86_64)]
pub(crate) fn query_pending_interrupt(timer_pending: bool) -> PendingInterruptAction {
    (EXIT_SERVICES.services().pending_interrupt)(timer_pending)
}
