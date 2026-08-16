mod atomics;
mod barrier;
mod cache;
mod context;
mod exception;
mod gic_cpu_interface;
mod host;
mod interrupt_controller;
mod interrupts;
mod kaslr;
mod memory;
mod platform;
mod psci;
pub mod registers;
mod smp;
mod stage2;
mod timer;
mod vgic;
mod vsysreg;

pub use atomics::{AtomicCapabilities, capabilities as atomic_capabilities};
pub use barrier::Aarch64Barrier as ArchitectureBarrier;
pub use cache::Aarch64Cache as ArchitectureCache;
pub use context::{
    ThreadContext, UserContext, VcpuContext, reset_stack_and_enter, run_on_emergency_stack,
    switch_thread_context,
};
pub use exception::{
    CrashContext, capture_crash_context, install_exception_stacks, install_runtime_vectors,
    validate_runtime_vectors,
};
pub use gic_cpu_interface::{
    Aarch64GicCpuInterface, acknowledge_interrupt, broadcast_crash_stop, crash_stop_interrupt,
    current_gic_affinity, end_interrupt, is_crash_stop_interrupt,
};
pub use host::mode_name as host_execution_mode_name;
pub use hyper::drivers::power::psci::Error as CpuPowerError;
pub use interrupt_controller::{
    Aarch64InterruptController as ArchitectureInterruptController,
    Error as InterruptControllerError,
};
pub use interrupts::{
    LocalInterruptMask, disable_all as disable_local_interrupts, enable_irq as enable_local_irq,
    irq_enabled as local_irq_enabled,
};
pub use kaslr::select as select_kaslr_layout;
#[cfg(CONFIG_CRASH_CONSOLE)]
pub use memory::inspect_mapping as inspect_stage1_mapping;
pub use memory::{
    Aarch64AddressTranslation as ArchitectureAddressTranslation, ActivationContext,
    Error as MemoryError, PreparedAddressSpace, StackMapping, bootstrap_stack_bounds,
};
pub use platform::{EssentialDeviceDiscovery, EssentialPlatformInfo, decode_platform_interrupt};
pub use psci::Aarch64Psci as ArchitectureCpuPower;
pub use smp::{
    SecondaryBootParameters, current_cpu_index, current_hardware_id, secondary_entry_physical,
    send_event,
};
pub use stage2::{Error as Stage2Error, Stage2AddressSpace};
pub use timer::{
    ArmGenericCounter as ArchitectureCounter, El2PhysicalTimer as ArchitectureTimer,
    Error as TimerError,
};
pub use vgic::Error as VirtualInterruptError;
pub use vgic::{
    Capabilities as VgicCapabilities, CpuContext as VgicCpuContext, Error as VgicError,
};
pub use vgic::{
    disable as disable_vgic, maintenance_state as vgic_maintenance_state,
    validate_context_switch as validate_vgic,
};
pub use vsysreg::validate as validate_vsysreg;
pub(crate) use vsysreg::{
    GuestDataAccess, GuestMemoryAccess, GuestSyncAction, GuestSyncFrame, GuestTranslationFault,
    handle_guest_sync,
};

pub fn initialize_cpu_power(
    info: hyper::platform::CpuPowerInfo,
) -> Result<ArchitectureCpuPower, CpuPowerError> {
    match info {
        hyper::platform::CpuPowerInfo::Psci(info) => psci::bind(info),
        hyper::platform::CpuPowerInfo::Sbi(_) | hyper::platform::CpuPowerInfo::X86Apic(_) => {
            Err(CpuPowerError::NotSupported)
        }
    }
}

/// Checks that a secondary CPU supports the backend selected by the boot CPU.
pub fn secondary_cpu_is_compatible() -> bool {
    atomics::current_cpu_supports_selected_backend() && host::current_cpu_is_compatible()
}

pub fn interrupt_is_per_cpu(interrupt: hyper::hal::interrupt::InterruptId) -> bool {
    interrupt.get() < 32
}

pub fn register_secondary_hardware_id(_cpu_index: usize, _hardware_id: u64) -> bool {
    true
}

pub fn mark_current_cpu_online() {}

pub fn prepare_timekeeping(_platform: &EssentialPlatformInfo) -> Result<(), TimerError> {
    Ok(())
}

pub fn prepare_cache(
    _platform: &EssentialPlatformInfo,
) -> Result<(), hyper::hal::cache::CacheError> {
    Ok(())
}

unsafe extern "C" {
    fn aarch64_activate_final_address_space(root: u64, virtual_base: u64, stack_top: u64) -> !;
}

/// Prepares the architecture-owned final address space.
///
/// # Safety
///
/// The bootstrap identity map must cover every page the early allocator may
/// return.
pub unsafe fn prepare_address_space(
    allocator: &mut hyper::mm::BootAllocator,
    platform: &hyper::platform::PlatformInfo,
    image: hyper::hal::memory::KernelImageLayout,
    kernel_base: u64,
) -> Result<PreparedAddressSpace, MemoryError> {
    unsafe { memory::prepare(allocator, platform, image, kernel_base) }
}

/// Activates final stage-1 translation and enters the high kernel alias.
///
/// # Safety
///
/// `memory` must remain installed in global boot state for the lifetime of the
/// active address space.
pub unsafe fn activate_memory(context: ActivationContext) -> ! {
    unsafe {
        aarch64_activate_final_address_space(
            context.root.get(),
            context.kernel_base,
            context.stack_top.get(),
        )
    }
}

/// Keeps this processing element in a side-effect-free idle loop.
pub fn halt() -> ! {
    loop {
        wait_for_interrupt();
    }
}

/// Suspends this processing element until an interrupt or event is observed.
pub fn wait_for_interrupt() {
    // SAFETY: WFI only suspends this processing element until an event or
    // interrupt occurs and does not access memory.
    unsafe { core::arch::asm!("wfi", options(nomem, nostack, preserves_flags)) };
}

/// Suspends until an event or interrupt; paired with `send_event` for work.
pub fn wait_for_event() {
    // SAFETY: WFE only affects the current processing element's event state.
    unsafe { core::arch::asm!("wfe", options(nomem, nostack, preserves_flags)) };
}

pub const fn port_io() -> Option<hyper::hal::io::PortIo> {
    None
}

/// Rust continuation used while the architecture bootstrap is still active.
///
/// This is not the kernel entry: the permanent address space and stack are
/// prepared from here, then assembly completes the transition to
/// `start_kernel`.
#[unsafe(no_mangle)]
extern "C" fn aarch64_bootstrap(dtb_address: usize) -> ! {
    atomics::initialize();
    host::initialize();
    crate::kernel::boot::prepare_boot_environment(crate::kernel::boot::ProtocolInputs::from_dtb(
        dtb_address,
    ))
}

pub fn poll_guest_timer(_now: u64) {}
pub const fn take_guest_timer_wakeup() -> Option<u64> {
    None
}
