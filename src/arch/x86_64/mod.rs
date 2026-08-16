mod barrier;
mod boot_protocol;
mod cache;
mod context;
mod cpu_power;
mod exception;
mod features;
mod guest;
mod interrupt_controller;
mod interrupts;
mod kaslr;
mod memory;
mod platform;
#[allow(dead_code)]
pub mod registers;
mod smp;
mod stage2;
mod svm;
mod svm_registers;
mod timer;
mod virtualization;
mod vmx;

use core::arch::asm;

pub use barrier::X86_64Barrier as ArchitectureBarrier;
pub use cache::X86_64Cache as ArchitectureCache;
pub use context::{
    ThreadContext, UserContext, VcpuContext, VirtualInterruptError, reset_stack_and_enter,
    switch_thread_context,
};
pub use cpu_power::{Error as CpuPowerError, X2ApicCpuPower as ArchitectureCpuPower};
pub use exception::{
    CrashContext, bootstrap_stack_bounds, capture_crash_context, install_exception_stacks,
    install_runtime_vectors, validate_runtime_vectors,
};
pub(crate) use guest::{
    GuestMemoryAccess, GuestSyncAction, GuestSyncFrame, GuestTranslationFault, handle_guest_sync,
};
pub use interrupt_controller::{
    Error as InterruptControllerError, X2ApicController as ArchitectureInterruptController,
};
pub use interrupts::{
    LocalInterruptMask, disable_all as disable_local_interrupts, enable_irq as enable_local_irq,
    irq_enabled as local_irq_enabled,
};
pub use kaslr::select as select_kaslr_layout;
#[cfg(CONFIG_CRASH_CONSOLE)]
pub use memory::inspect_mapping as inspect_stage1_mapping;
pub use memory::{
    ActivationContext, Error as MemoryError, PreparedAddressSpace, StackMapping,
    X86_64AddressTranslation as ArchitectureAddressTranslation,
};
pub use platform::{EssentialDeviceDiscovery, EssentialPlatformInfo, decode_platform_interrupt};
pub use smp::{
    SecondaryBootParameters, current_cpu_index, current_hardware_id, secondary_entry_physical,
    send_event,
};
pub use stage2::{Error as Stage2Error, Stage2AddressSpace};
pub use timer::{
    Error as TimerError, TscCounter as ArchitectureCounter, TscDeadlineTimer as ArchitectureTimer,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AtomicCapabilities;

impl AtomicCapabilities {
    pub const fn backend_name(self) -> &'static str {
        "x86-64 locked instructions"
    }
}

pub const fn atomic_capabilities() -> AtomicCapabilities {
    AtomicCapabilities
}

pub fn initialize_cpu_power(
    info: hyper::platform::CpuPowerInfo,
) -> Result<ArchitectureCpuPower, CpuPowerError> {
    cpu_power::bind(info)
}

pub fn secondary_cpu_is_compatible() -> bool {
    true
}

pub fn interrupt_is_per_cpu(interrupt: hyper::hal::interrupt::InterruptId) -> bool {
    interrupt.get() == platform::TIMER_VECTOR
}

pub fn register_secondary_hardware_id(cpu_index: usize, hardware_id: u64) -> bool {
    smp::register_cpu(cpu_index, hardware_id)
}

pub fn mark_current_cpu_online() {
    smp::mark_current_cpu_online();
}

pub fn prepare_timekeeping(platform: &EssentialPlatformInfo) -> Result<(), TimerError> {
    timer::set_frequency(platform.tsc_frequency)
}

pub fn prepare_cache(
    _platform: &EssentialPlatformInfo,
) -> Result<(), hyper::hal::cache::CacheError> {
    cache::initialize()
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
    fn x86_64_activate_final_address_space(root: u64, kernel_base: u64, stack_top: u64) -> !;
}

pub unsafe fn activate_memory(context: ActivationContext) -> ! {
    unsafe {
        x86_64_activate_final_address_space(
            context.root.get(),
            context.kernel_base,
            context.stack_top.get(),
        )
    }
}

pub fn halt() -> ! {
    loop {
        unsafe { asm!("cli", "hlt", options(nomem, nostack)) };
    }
}

pub fn wait_for_event() {
    unsafe { asm!("hlt", options(nomem, nostack)) };
}

pub const fn port_io() -> Option<hyper::hal::io::PortIo> {
    Some(unsafe { hyper::hal::io::PortIo::new(read_port, write_port) })
}

unsafe fn read_port(port: u16) -> u8 {
    let value: u8;
    unsafe { asm!("in al, dx", in("dx") port, out("al") value, options(nomem, nostack)) };
    value
}

unsafe fn write_port(port: u16, value: u8) {
    unsafe { asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack)) };
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

pub fn virtualization_backend_name() -> &'static str {
    virtualization::backend_name()
}

pub fn poll_guest_timer(now: u64) {
    guest::poll_virtual_timer(now);
}

pub fn take_guest_timer_wakeup() -> Option<u64> {
    guest::take_timer_wakeup()
}

#[unsafe(no_mangle)]
extern "C" fn x86_64_bootstrap(boot_params: usize) -> ! {
    smp::initialize_boot_cpu();
    let inputs = match unsafe { boot_protocol::parse(boot_params) } {
        Ok(inputs) => inputs,
        Err(_) => halt(),
    };
    crate::kernel::boot::prepare_boot_environment(crate::kernel::boot::ProtocolInputs {
        dtb_address: inputs.dtb_address,
        command_line: inputs.command_line,
        initial_ramdisk: inputs.initial_ramdisk,
    })
}
