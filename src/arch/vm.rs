// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Selected-architecture hardware virtualization mechanisms.
//!
//! Kernel VM policy owns VM publication, vCPU scheduling, demand paging, and
//! exit disposition. This facade selects stage-2 translation, vCPU entry,
//! virtual interrupt, guest timer, and architecture-local exit mechanisms.
//! Linux image formats and boot ABI policy deliberately remain outside it.

pub use super::imp::{
    VcpuInterruptError, VmInterruptController as InterruptController,
    VmInterruptError as InterruptError,
};

pub(crate) use super::imp::{
    GuestValidationError as RegisterValidationError,
    InterruptVirtualizationError as InterruptInitializationError, Stage2AddressSpace, Stage2Error,
    VcpuContext, VirtualDeviceInitializationError as DeviceError, VirtualInterruptError,
};

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

#[cfg(CONFIG_ARCH_AARCH64)]
pub(crate) use super::imp::{
    complete_guest_mmio_access as complete_legacy_mmio,
    decode_guest_mmio_access as decode_legacy_mmio,
    update_guest_device_interrupt as update_legacy_device_interrupt,
};

/// Compatibility vocabulary for architecture-owned raw exit frames.
///
/// Frame ownership remains with the backend entry path and kernel policy may
/// borrow it only for one synchronous dispatch. New exit classes must use
/// owned events from `hyper::vm::exit` instead of extending this interface.
pub(crate) use super::imp::{
    GuestSyncAction as LegacySyncAction, GuestSyncFrame as LegacySyncFrame,
    deliver_guest_software_interrupt as deliver_legacy_software_interrupt,
    handle_guest_device_access as handle_legacy_device_access,
    handle_guest_sync as decode_legacy_sync,
};

pub(crate) use super::imp::{
    activate_vcpu_hardware, deactivate_vcpu_hardware,
    virtualization_maintenance_pending as maintenance_interrupt_pending,
};

#[cfg(CONFIG_ARCH_AARCH64)]
pub(crate) use super::imp::inject_timer_for_validation;
