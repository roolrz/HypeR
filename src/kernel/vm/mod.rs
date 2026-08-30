// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Virtual-machine initialization and execution policy.
//!
//! This subsystem owns hardware-virtualization validation, virtual interrupt
//! activation, guest-visible devices, VM publication, and vCPU orchestration.

pub(crate) mod active_vcpu;
pub(crate) mod device;
pub(crate) mod linux;
pub mod memory;
pub(crate) mod registry;
mod timer;
pub(crate) mod vcpu;

use core::convert::Infallible;

use hyper::sync::InterruptSpinLock;

type EntryReadyLock =
    InterruptSpinLock<Option<crate::hal::vm::VmEntryReady>, crate::hal::irq::LocalMask>;

static ENTRY_READY: EntryReadyLock = InterruptSpinLock::new(None);

pub use crate::hal::vm::{
    InterruptController as VmInterruptController, InterruptError as VmInterruptError,
};
pub use hyper::vm::bundle::{Error as VmBundleError, VmBundle};
pub use linux::Error as LinuxBootError;
pub use registry::VmId;
pub use vcpu::{RunError as VcpuRunError, VcpuInterruptError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InitializationError {
    Devices(crate::hal::vm::DeviceError),
    Interrupts(crate::hal::vm::InterruptInitializationError),
    EntryServices(crate::hal::vm::ExitServiceError),
    EntryReadyAlreadyPublished,
    Registers(crate::hal::vm::RegisterValidationError),
    Timer(timer::Error),
    TimerValidation(timer::ValidationError),
}

#[derive(Debug)]
pub enum StartError {
    Bundle(VmBundleError),
    InitialRamdiskAddress(hyper::platform::PhysicalRange),
    InitialRamdiskSize(hyper::platform::PhysicalRange),
    Linux(LinuxBootError),
}

pub fn select_default(ramdisk: &[u8]) -> Result<VmBundle<'_>, VmBundleError> {
    hyper::vm::bundle::select_default(ramdisk)
}

pub fn boot_linux(
    guest: VmBundle<'_>,
) -> Result<crate::kernel::task::thread::ThreadId, LinuxBootError> {
    linux::boot(guest)
}

/// Initializes hardware virtualization and guest-visible platform devices.
pub(crate) fn initialize(boot: &super::boot::Initialization) -> Result<(), InitializationError> {
    let exit_services = crate::hal::vm::install_exit_services({
        #[cfg(CONFIG_ARCH_AARCH64)]
        {
            crate::hal::vm::ExitServices::aarch64(
                crate::kernel::entry::vmexit::dispatch_memory_fault,
                crate::kernel::entry::vmexit::dispatch_mmio,
                crate::kernel::entry::vmexit::dispatch_guest_sync,
            )
        }
        #[cfg(CONFIG_ARCH_RISCV64)]
        {
            crate::hal::vm::ExitServices::riscv64(
                crate::kernel::entry::vmexit::dispatch_memory_fault,
                crate::kernel::entry::vmexit::dispatch_guest_sync,
            )
        }
        #[cfg(CONFIG_ARCH_X86_64)]
        {
            crate::hal::vm::ExitServices::x86_64(
                crate::kernel::entry::vmexit::dispatch_memory_fault,
                crate::kernel::entry::vmexit::dispatch_port_io,
                crate::kernel::entry::vmexit::query_pending_interrupt,
            )
        }
    })
    .map_err(InitializationError::EntryServices)?;
    crate::hal::vm::validate_register_interface().map_err(InitializationError::Registers)?;
    // Prepared physical mappings must remain non-deliverable until every VM
    // dependency below is published. Quiesce virtual delivery before exposing
    // their disabled handler records to the IRQ registry.
    crate::hal::vm::quiesce_virtual_interrupt_delivery();
    let interrupts = boot.interrupts();
    let guest_timer = boot.timer().guest_timer;
    let binding = timer::prepare(
        guest_timer,
        interrupts.root_domain,
        interrupts.maintenance_interrupt,
    )
    .map_err(InitializationError::Timer)?;
    if let Some(interrupt) = binding.host_interrupt() {
        crate::println!(
            "HypeR: guest architectural timer mapped to host VIRQ {}",
            interrupt.get()
        );
    }
    if let Err(error) =
        crate::hal::vm::initialize_devices(guest_timer.interrupt, binding.host_interrupt())
    {
        binding.rollback();
        return Err(InitializationError::Devices(error));
    }
    match timer::validate_hardware(guest_timer.interrupt, &exit_services) {
        Ok(true) => crate::println!("HypeR: virtual architected timer injection validated"),
        Ok(false) => {}
        Err(error) => {
            binding.rollback();
            return Err(InitializationError::TimerValidation(error));
        }
    }
    if let Err(error) = crate::hal::vm::initialize_interrupts(binding.host_interrupt()) {
        binding.rollback();
        return Err(InitializationError::Interrupts(error));
    }
    if let (Some(description), Some(maintenance)) = (
        crate::hal::vm::interrupt_virtualization_description(),
        binding.maintenance_interrupt(),
    ) {
        crate::println!(
            "HypeR: vGICv3 active with {} LRs, {} priority bits, {} preemption bits, {} INTID bits, maintenance VIRQ {}",
            description.list_registers,
            description.priority_bits,
            description.preemption_bits,
            description.interrupt_id_bits,
            maintenance.get(),
        );
    }
    binding.activate().map_err(InitializationError::Timer)?;
    let entry_ready = crate::hal::vm::complete_entry_initialization(exit_services);
    ENTRY_READY.with(|slot| {
        if slot.is_some() {
            Err(InitializationError::EntryReadyAlreadyPublished)
        } else {
            *slot = Some(entry_ready);
            Ok(())
        }
    })?;
    crate::println!("HypeR: guest synchronous trap and vSysReg emulation validated");
    Ok(())
}

pub(in crate::kernel) fn entry_ready() -> Option<crate::hal::vm::VmEntryReady> {
    ENTRY_READY.with(|slot| *slot)
}

/// Loads the default VM bundle from the boot ramdisk and enters the guest.
pub(crate) fn start_default() -> Result<Infallible, StartError> {
    let initial_ramdisk = super::boot::with_boot_state(|state| state.initial_ramdisk);
    let ramdisk_address = super::mm::memory::linear_address(initial_ramdisk.start())
        .ok_or(StartError::InitialRamdiskAddress(initial_ramdisk))?;
    let ramdisk_size = usize::try_from(initial_ramdisk.size())
        .map_err(|_| StartError::InitialRamdiskSize(initial_ramdisk))?;
    if ramdisk_size > isize::MAX as usize || ramdisk_address.checked_add(ramdisk_size).is_none() {
        return Err(StartError::InitialRamdiskSize(initial_ramdisk));
    }
    // SAFETY: Early boot reserved this firmware-owned RAM range before buddy
    // handoff, and the permanent linear map covers the validated complete range.
    let ramdisk =
        unsafe { core::slice::from_raw_parts(ramdisk_address as *const u8, ramdisk_size) };
    let guest = select_default(ramdisk).map_err(StartError::Bundle)?;
    crate::println!(
        "HypeR: loaded VM '{}' from boot ramdisk: {} MiB RAM, {} vCPU(s)",
        guest.name(),
        guest.memory_size() / (1024 * 1024),
        guest.vcpu_count()
    );
    crate::println!("HypeR: kernel initialization complete; starting Linux guest");
    let vcpu = match boot_linux(guest) {
        Ok(vcpu) => vcpu,
        Err(error) => {
            crate::pr_err!("HypeR: Linux guest boot failed: {error:?}");
            return Err(StartError::Linux(error));
        }
    };
    crate::println!("HypeR: Linux boot vCPU scheduled as thread {}", vcpu.get());
    super::task::scheduler::exit_current()
}

/// Routes host-console input to the virtual console owned by the active VM.
pub(crate) fn receive_console_input(byte: u8) -> bool {
    device::receive_console_input(byte)
}
