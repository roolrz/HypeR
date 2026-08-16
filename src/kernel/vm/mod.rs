//! Virtual-machine execution policy.

mod active_vcpu;
#[cfg(target_arch = "aarch64")]
mod arch_timer;
mod device;
mod interrupt;
mod linux;
pub mod memory;
mod runtime;
mod vcpu;

#[cfg(target_arch = "aarch64")]
pub(crate) use arch_timer::{handle_interrupt as handle_arch_timer_interrupt, handle_maintenance};
pub use hyper::vm::bundle::{Error as VmBundleError, VmBundle};
pub use interrupt::{Error as VmInterruptError, VmInterruptController};
pub use linux::Error as LinuxBootError;
pub use vcpu::VcpuInterruptError;

pub fn select_default(ramdisk: &[u8]) -> Result<VmBundle<'_>, VmBundleError> {
    hyper::vm::bundle::select_default(ramdisk)
}

pub fn boot_linux(
    guest: VmBundle<'_>,
) -> Result<crate::kernel::task::thread::ThreadId, LinuxBootError> {
    linux::boot(guest)
}

/// Validates the guest-visible devices needed before loading the first VM.
pub(crate) fn initialize_virtual_devices(boot: &super::boot::Initialization) {
    #[cfg(target_arch = "aarch64")]
    {
        if let Err(error) = validate_arch_timer(boot.timer().guest_virtual_interrupt) {
            super::boot::fail("virtual architected timer validation", error);
        }
        if let Err(error) = device::console::initialize() {
            super::boot::fail("virtual console initialization", error);
        }
        crate::println!("HypeR: virtual architected timer injection validated");
        crate::println!("HypeR: virtual PL011 console initialized (host UART backend)");
    }
    #[cfg(target_arch = "riscv64")]
    {
        let _ = boot;
        crate::println!("HypeR: RISC-V guest SBI and virtual timer backend initialized");
    }
    #[cfg(target_arch = "x86_64")]
    {
        let _ = boot;
        if let Err(error) = device::legacy_pc::initialize() {
            super::boot::fail("legacy PC virtual-device initialization", error);
        }
        crate::println!("HypeR: x86 VMX and legacy PC virtual-device backends initialized");
    }
}

#[cfg(target_arch = "x86_64")]
pub(crate) fn handle_port_io(
    port: u16,
    size: usize,
    write: bool,
    value: u32,
) -> Result<Option<u32>, device::legacy_pc::Error> {
    device::legacy_pc::access(port, size, write, value)
}

#[cfg(target_arch = "x86_64")]
pub(crate) fn legacy_timer_vector() -> Result<Option<u8>, device::legacy_pc::Error> {
    device::legacy_pc::timer_vector()
}

/// Routes a byte emitted by guest firmware or a virtual UART to the host log
/// backend. Architecture exit decoding must not depend on logging internals.
#[cfg(any(target_arch = "riscv64", target_arch = "x86_64"))]
pub(crate) fn write_guest_console_byte(byte: u8) {
    crate::kernel::log::console::write_raw_byte(byte);
}

#[cfg(target_arch = "aarch64")]
pub(crate) fn receive_console_input(byte: u8) -> Result<(), device::console::Error> {
    device::console::receive(byte)
}

#[cfg(target_arch = "riscv64")]
pub(crate) fn receive_console_input(_byte: u8) -> Result<(), ()> {
    Err(())
}

#[cfg(target_arch = "x86_64")]
pub(crate) fn receive_console_input(_byte: u8) -> Result<(), ()> {
    Err(())
}

/// Loads the default VM bundle from the boot ramdisk and enters the guest.
pub(crate) fn start_default() -> ! {
    let initial_ramdisk = super::boot::with_boot_state(|state| state.initial_ramdisk);
    let ramdisk_address = match super::mm::memory::linear_address(initial_ramdisk.start()) {
        Some(address) => address,
        None => super::boot::fail("initial ramdisk address translation", initial_ramdisk),
    };
    let ramdisk_size = match usize::try_from(initial_ramdisk.size()) {
        Ok(size) => size,
        Err(_) => super::boot::fail("initial ramdisk size conversion", initial_ramdisk),
    };
    // SAFETY: Early boot reserved this firmware-owned RAM range before buddy
    // handoff, and the permanent linear map covers the validated complete range.
    let ramdisk =
        unsafe { core::slice::from_raw_parts(ramdisk_address as *const u8, ramdisk_size) };
    let guest = match select_default(ramdisk) {
        Ok(guest) => guest,
        Err(error) => super::boot::fail("VM bundle loading", error),
    };
    crate::println!(
        "HypeR: loaded VM '{}' from boot ramdisk: {} MiB RAM, {} vCPU(s)",
        guest.name(),
        guest.memory_size() / (1024 * 1024),
        guest.vcpu_count()
    );
    crate::println!("HypeR: kernel initialization complete; starting Linux guest");
    let vcpu = match boot_linux(guest) {
        Ok(vcpu) => vcpu,
        Err(error) => super::boot::fail("Linux guest boot", error),
    };
    crate::println!("HypeR: Linux boot vCPU scheduled as thread {}", vcpu.get());
    super::task::scheduler::thread_become_idle()
}

pub(crate) fn handle_guest_sync(frame: &mut crate::arch::GuestSyncFrame<'_>) -> bool {
    match active_vcpu::with(|execution, interrupts| {
        let action =
            crate::arch::handle_guest_sync(&mut execution.context, execution.vcpu_id, frame);
        if let Some(deadline) = crate::arch::take_guest_timer_wakeup()
            && let Err(error) = crate::kernel::time::request_hardware_wakeup(deadline)
        {
            crate::pr_err!("HypeR: failed to arm guest timer wakeup: {error:?}");
            return crate::arch::GuestSyncAction::Unhandled;
        }
        match action {
            crate::arch::GuestSyncAction::SoftwareInterrupt(request) => {
                #[cfg(target_arch = "riscv64")]
                {
                    let _ = request;
                    return crate::arch::GuestSyncAction::Unhandled;
                }
                #[cfg(target_arch = "aarch64")]
                return match vcpu::deliver_software_interrupt(execution, interrupts, request) {
                    Ok(()) => crate::arch::GuestSyncAction::Resume,
                    Err(error) => {
                        crate::pr_err!("HypeR: failed to deliver guest SGI: {error:?}");
                        crate::arch::GuestSyncAction::Unhandled
                    }
                };
                #[cfg(target_arch = "x86_64")]
                {
                    let _ = (execution, interrupts, request);
                    return crate::arch::GuestSyncAction::Unhandled;
                }
            }
            crate::arch::GuestSyncAction::Unhandled => {}
            _ => return action,
        }
        if let Some(fault) = frame.translation_fault() {
            match memory::resolve_translation_fault(execution.virtual_machine, fault) {
                Ok(true) => return crate::arch::GuestSyncAction::Resume,
                Ok(false) => {}
                Err(error) => {
                    crate::pr_err!(
                        "HypeR: guest memory fault resolution failed at {:#x} ({:?}, S1PTW={}): {error:?}",
                        fault.address,
                        fault.access,
                        fault.during_page_walk
                    );
                    return crate::arch::GuestSyncAction::Unhandled;
                }
            }
        }
        #[cfg(target_arch = "riscv64")]
        {
            let _ = interrupts;
            action
        }
        #[cfg(target_arch = "aarch64")]
        {
            let Some(access) = frame.data_access() else {
                return action;
            };
            if device::console::handles(access.address) {
                return match device::console::access(access) {
                    Ok(value) => {
                        frame.complete_data_access(access, value);
                        crate::arch::GuestSyncAction::Resume
                    }
                    Err(error) => {
                        crate::pr_err!(
                            "HypeR: unsupported guest console access at {:#x}: {error:?}",
                            access.address
                        );
                        action
                    }
                };
            }
            if !device::gicv3::handles(access.address) {
                return action;
            }
            let vcpu = VirtualCpuId::new(execution.vcpu_id);
            let result = if access.write {
                device::gicv3::write(interrupts, vcpu, access.address, access.size, access.value)
                    .map(|()| None)
            } else {
                device::gicv3::read(interrupts, vcpu, access.address, access.size).map(Some)
            };
            match result {
                Ok(value) => {
                    frame.complete_data_access(access, value);
                    crate::arch::GuestSyncAction::Resume
                }
                Err(error) => {
                    crate::pr_err!(
                        "HypeR: unsupported guest GIC access at {:#x}, FAR {:#x}: {error:?}",
                        access.address,
                        frame.fault_address()
                    );
                    action
                }
            }
        }
        #[cfg(target_arch = "x86_64")]
        {
            let _ = interrupts;
            action
        }
    }) {
        Ok(Some(crate::arch::GuestSyncAction::Resume | crate::arch::GuestSyncAction::Injected)) => {
            true
        }
        Ok(Some(
            crate::arch::GuestSyncAction::SoftwareInterrupt(_)
            | crate::arch::GuestSyncAction::Unhandled,
        ))
        | Ok(None)
        | Err(_) => false,
    }
}

#[cfg(target_arch = "aarch64")]
use hyper::hal::interrupt::InterruptId;
#[cfg(target_arch = "aarch64")]
use hyper::vm::interrupt::VirtualCpuId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(target_arch = "aarch64")]
pub enum ValidationError {
    ActiveBridge(arch_timer::Error),
    Context(crate::arch::VgicError),
    Interrupts(interrupt::Error),
    Model(hyper::vm::interrupt::Error),
    StateMismatch,
    Vcpu(vcpu::VcpuInterruptError),
}

#[cfg(target_arch = "aarch64")]
pub fn validate_arch_timer(timer_interrupt: InterruptId) -> Result<(), ValidationError> {
    let interrupts =
        VmInterruptController::new(1, timer_interrupt).map_err(ValidationError::Interrupts)?;
    let mut context = crate::arch::VcpuContext::new(0);
    let _ = context
        .initialize_virtual_interrupts()
        .map_err(ValidationError::Context)?;
    let now = crate::kernel::time::monotonic_ticks();
    context.set_virtual_count(now, now);
    context.set_virtual_timer_deadline(now.wrapping_add(1_000_000));
    context.set_virtual_timer_enabled(true);
    let mut execution = crate::kernel::task::thread::VcpuExecution {
        virtual_machine: crate::kernel::task::thread::VirtualMachineId(u64::MAX),
        vcpu_id: 0,
        context,
    };
    // SAFETY: Boot validation owns these pinned stack objects, runs with local
    // IRQs masked, and pairs activation with deactivation before returning.
    unsafe {
        execution
            .activate_virtual_hardware(&interrupts)
            .map_err(ValidationError::Vcpu)?;
    }
    if !arch_timer::inject_active_for_validation().map_err(ValidationError::ActiveBridge)? {
        return Err(ValidationError::StateMismatch);
    }
    let snapshot = interrupts
        .timer_snapshot(VirtualCpuId::new(0))
        .map_err(ValidationError::Model)?;
    if !snapshot.pending || !snapshot.listed {
        return Err(ValidationError::StateMismatch);
    }
    // SAFETY: This is the active validation vCPU and local IRQs remain masked.
    unsafe {
        execution
            .deactivate_virtual_hardware(&interrupts)
            .map_err(ValidationError::Vcpu)?;
    }
    Ok(())
}
