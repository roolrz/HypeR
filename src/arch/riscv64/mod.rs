mod barrier;
mod cache;
mod context;
mod exception;
mod guest;
mod interrupt_controller;
mod interrupts;
mod kaslr;
mod memory;
mod platform;
#[allow(dead_code)]
pub mod registers;
mod sbi;
mod smp;
mod stage2;
mod timer;

use core::arch::asm;

pub use barrier::Riscv64Barrier as ArchitectureBarrier;
pub use cache::Riscv64Cache as ArchitectureCache;
pub use context::{
    ThreadContext, UserContext, VcpuContext, VirtualInterruptError, reset_stack_and_enter,
    switch_thread_context,
};
pub use exception::{
    CrashContext, bootstrap_stack_bounds, capture_crash_context, install_exception_stacks,
    install_runtime_vectors, validate_runtime_vectors,
};
pub(crate) use guest::{
    GuestMemoryAccess, GuestSyncAction, GuestSyncFrame, GuestTranslationFault, handle_guest_sync,
};
pub use interrupt_controller::{
    Error as InterruptControllerError,
    Riscv64InterruptController as ArchitectureInterruptController,
};
pub use interrupts::{
    LocalInterruptMask, disable_all as disable_local_interrupts, enable_irq as enable_local_irq,
    irq_enabled as local_irq_enabled,
};
pub use kaslr::select as select_kaslr_layout;
#[cfg(CONFIG_CRASH_CONSOLE)]
pub use memory::inspect_mapping as inspect_stage1_mapping;
pub use memory::{
    ActivationContext, Error as MemoryError, PreparedAddressSpace,
    Riscv64AddressTranslation as ArchitectureAddressTranslation, StackMapping,
};
pub use platform::{EssentialDeviceDiscovery, EssentialPlatformInfo, decode_platform_interrupt};
pub use sbi::{Error as CpuPowerError, Sbi as ArchitectureCpuPower};
pub use smp::{
    SecondaryBootParameters, current_cpu_index, current_hardware_id, secondary_entry_physical,
    send_event,
};
pub use stage2::{Error as Stage2Error, Stage2AddressSpace};
pub use timer::{
    Error as TimerError, RiscvTimeCounter as ArchitectureCounter,
    SupervisorTimer as ArchitectureTimer,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AtomicCapabilities;

impl AtomicCapabilities {
    pub const fn backend_name(self) -> &'static str {
        "RV64A AMO/LR-SC"
    }
}
pub const fn atomic_capabilities() -> AtomicCapabilities {
    AtomicCapabilities
}

pub fn initialize_cpu_power(
    info: hyper::platform::CpuPowerInfo,
) -> Result<ArchitectureCpuPower, CpuPowerError> {
    match info {
        hyper::platform::CpuPowerInfo::Sbi(_) => sbi::bind(),
        hyper::platform::CpuPowerInfo::Psci(_) => Err(CpuPowerError::NotSupported),
    }
}

pub fn secondary_cpu_is_compatible() -> bool {
    true
}
pub fn interrupt_is_per_cpu(interrupt: hyper::hal::interrupt::InterruptId) -> bool {
    interrupt.get() == 0
}
pub fn register_secondary_hardware_id(cpu_index: usize, hardware_id: u64) -> bool {
    smp::register_hart(cpu_index, hardware_id)
}
pub fn mark_current_cpu_online() {
    smp::mark_current_hart_online();
}
pub fn prepare_timekeeping(platform: &EssentialPlatformInfo) -> Result<(), TimerError> {
    timer::set_frequency(platform.timebase_frequency)
}
pub fn prepare_cache(
    platform: &EssentialPlatformInfo,
) -> Result<(), hyper::hal::cache::CacheError> {
    cache::initialize(platform.cache_block_size)
}

pub unsafe fn prepare_address_space(
    allocator: &mut hyper::mm::BootAllocator,
    platform: &hyper::platform::PlatformInfo,
    image: hyper::hal::memory::KernelImageLayout,
    kernel_base: u64,
) -> Result<PreparedAddressSpace, MemoryError> {
    unsafe { memory::prepare(allocator, platform, image, kernel_base) }
}

unsafe extern "C" {
    fn riscv64_activate_final_address_space(root: u64, kernel_base: u64, stack_top: u64) -> !;
}

pub unsafe fn activate_memory(context: ActivationContext) -> ! {
    unsafe {
        riscv64_activate_final_address_space(
            context.root.get(),
            context.kernel_base,
            context.stack_top.get(),
        )
    }
}

pub fn halt() -> ! {
    loop {
        unsafe { asm!("wfi", options(nomem, nostack)) }
    }
}
pub fn wait_for_event() {
    unsafe { asm!("wfi", options(nomem, nostack)) }
}

pub unsafe fn run_on_emergency_stack(callback: extern "C" fn(usize) -> !, argument: usize) -> ! {
    unsafe { exception::run_on_emergency_stack(callback, argument) }
}

pub const fn crash_stop_interrupt() -> Option<hyper::hal::interrupt::InterruptId> {
    None
}
pub const fn broadcast_crash_stop() -> bool {
    false
}
pub fn validate_vsysreg() -> Result<(), guest::ValidationError> {
    guest::validate()
}
pub fn poll_guest_timer(now: u64) {
    guest::poll_virtual_timer(now)
}
pub fn take_guest_timer_wakeup() -> Option<u64> {
    guest::take_timer_wakeup()
}

#[unsafe(no_mangle)]
extern "C" fn riscv64_bootstrap(hart_id: usize, dtb_address: usize) -> ! {
    smp::initialize_boot_hart(hart_id as u64);
    crate::kernel::prepare_boot_environment(dtb_address)
}
