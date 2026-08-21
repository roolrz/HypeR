//! Virtual-machine execution policy.

pub(crate) mod active_vcpu;
pub(crate) mod device;
pub(crate) mod linux;
pub mod memory;
pub(crate) mod registry;
pub(crate) mod vcpu;

use core::convert::Infallible;

pub use crate::arch::vm::{
    InterruptController as VmInterruptController, InterruptError as VmInterruptError,
};
pub use hyper::vm::bundle::{Error as VmBundleError, VmBundle};
pub use linux::Error as LinuxBootError;
pub use registry::VmId;
pub use vcpu::{RunError as VcpuRunError, VcpuInterruptError};

pub(crate) type InitializationError = crate::arch::vm::DeviceError;

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

/// Validates the guest-visible devices needed before loading the first VM.
pub(crate) fn initialize_virtual_devices(
    boot: &super::boot::Initialization,
) -> Result<(), InitializationError> {
    crate::arch::vm::initialize_devices(boot.timer().guest_virtual_interrupt)
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
    let vcpu = boot_linux(guest).map_err(StartError::Linux)?;
    crate::println!("HypeR: Linux boot vCPU scheduled as thread {}", vcpu.get());
    super::task::scheduler::thread_become_idle()
}

pub(crate) fn handle_guest_sync(frame: &mut crate::arch::vm::LegacySyncFrame<'_>) -> bool {
    handle_guest_sync_inner(frame, true)
}

/// Continues architecture-local exit decoding after typed memory policy has
/// already classified the fault as outside guest RAM.
pub(crate) fn handle_guest_sync_after_memory_fault(
    frame: &mut crate::arch::vm::LegacySyncFrame<'_>,
) -> bool {
    handle_guest_sync_inner(frame, false)
}

fn handle_guest_sync_inner(
    frame: &mut crate::arch::vm::LegacySyncFrame<'_>,
    resolve_memory_fault: bool,
) -> bool {
    match active_vcpu::with(|execution, interrupts| {
        let action =
            crate::arch::vm::decode_legacy_sync(&mut execution.context, execution.vcpu_id, frame);
        if let Some(deadline) = crate::arch::vm::take_timer_wakeup()
            && let Err(error) = crate::kernel::time::request_hardware_wakeup(deadline)
        {
            crate::pr_err!("HypeR: failed to arm guest timer wakeup: {error:?}");
            return crate::arch::vm::LegacySyncAction::Unhandled;
        }
        match action {
            crate::arch::vm::LegacySyncAction::SoftwareInterrupt(request) => {
                return match crate::arch::vm::deliver_legacy_software_interrupt(
                    execution, interrupts, request,
                ) {
                    Ok(()) => crate::arch::vm::LegacySyncAction::Resume,
                    Err(error) => {
                        crate::pr_err!(
                            "HypeR: failed to deliver guest software interrupt: {error:?}"
                        );
                        crate::arch::vm::LegacySyncAction::Unhandled
                    }
                };
            }
            crate::arch::vm::LegacySyncAction::Unhandled => {}
            _ => return action,
        }
        if resolve_memory_fault && let Some(fault) = frame.guest_memory_fault() {
            let Some(vm) = execution.vm_binding() else {
                return crate::arch::vm::LegacySyncAction::Unhandled;
            };
            match memory::resolve_guest_memory_fault(vm, fault) {
                Ok(true) => return crate::arch::vm::LegacySyncAction::Resume,
                Ok(false) => {}
                Err(error) => {
                    crate::pr_err!(
                        "HypeR: guest memory fault resolution failed at {:#x} ({:?}, guest-page-walk={}): {error:?}",
                        fault.address().get(),
                        fault.access(),
                        fault.during_guest_page_walk()
                    );
                    return crate::arch::vm::LegacySyncAction::Unhandled;
                }
            }
        }
        if let Some(device_action) = device::handle_legacy_mmio(execution, interrupts, frame) {
            return device_action;
        }
        crate::arch::vm::handle_legacy_device_access(execution, interrupts, frame, action)
    }) {
        Ok(Some(
            crate::arch::vm::LegacySyncAction::Resume | crate::arch::vm::LegacySyncAction::Injected,
        )) => true,
        Ok(Some(
            crate::arch::vm::LegacySyncAction::SoftwareInterrupt(_)
            | crate::arch::vm::LegacySyncAction::Unhandled,
        ))
        | Ok(None)
        | Err(_) => false,
    }
}

/// Routes host-console input to the virtual console owned by the active VM.
pub(crate) fn receive_console_input(byte: u8) -> bool {
    device::receive_console_input(byte)
}
