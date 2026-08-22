// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

#![deny(clippy::missing_safety_doc, clippy::undocumented_unsafe_blocks)]

mod barrier;
mod cache;
mod context;
mod exception;
mod guest;
mod interrupt_controller;
mod interrupts;
mod kaslr;
mod linux;
mod memory;
mod platform;
#[allow(dead_code)]
pub mod registers;
mod sbi;
mod smp;
mod stage2;
mod timer;
mod vm_interrupt;
mod vm_vcpu;

use core::arch::asm;

pub type InterruptVirtualizationError = core::convert::Infallible;

pub use barrier::Riscv64Barrier as ArchitectureBarrier;
pub use cache::Riscv64Cache as ArchitectureCache;
pub use context::{
    ThreadContext, UserContext, VcpuContext, VirtualInterruptError, reset_stack_and_enter,
    switch_thread_context,
};
pub use exception::ValidationError as RuntimeVectorError;
pub use exception::{
    CrashContext, bootstrap_stack_bounds, capture_crash_context, install_exception_stacks,
    install_runtime_vectors, run_on_emergency_stack, validate_runtime_vectors,
};
pub use guest::ValidationError as GuestValidationError;
pub(crate) use guest::{GuestSyncAction, GuestSyncFrame, handle_guest_sync};
pub use interrupt_controller::{
    Error as InterruptControllerError,
    Riscv64InterruptController as ArchitectureInterruptController,
};
pub use interrupts::{
    LocalInterruptMask, disable_all as disable_local_interrupts, enable_irq as enable_local_irq,
    irq_enabled as local_irq_enabled,
};
pub use kaslr::{Error as KaslrError, select as select_kaslr_layout};
pub(crate) use linux::{
    LINUX_GUEST_KERNEL_IPA, LINUX_GUEST_RAM_IPA, LINUX_GUEST_TIMER_INTERRUPT,
    describe_linux_guest_layout, describe_linux_host, linux_guest_architecture,
    linux_kernel_occupied_size, load_linux_payload, prepare_linux_vcpu_context,
    validate_linux_host, validate_linux_kernel,
};
#[cfg(CONFIG_CRASH_CONSOLE)]
pub use memory::inspect_mapping as inspect_stage1_mapping;
pub use memory::{
    ActivationContext, Error as MemoryError, PreparedAddressSpace,
    Riscv64AddressTranslation as ArchitectureAddressTranslation, StackMapping,
};
pub use platform::{
    Error as PlatformDiscoveryError, EssentialDeviceDiscovery, EssentialPlatformInfo,
    decode_platform_interrupt,
};
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
pub use vm_interrupt::{Error as VmInterruptError, VmInterruptController};
pub use vm_vcpu::Error as VcpuInterruptError;
pub(crate) use vm_vcpu::{
    deliver_software_interrupt as deliver_guest_software_interrupt, handle_guest_device_access,
};

pub type VirtualDeviceInitializationError = core::convert::Infallible;

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

/// Sv39 permissions directly encode the kernel's RX/R/XN split; there is no
/// AArch64-style global WXN control to enable on each hart.
pub fn enable_local_memory_protection() {}

pub const fn local_memory_protection_enabled() -> bool {
    true
}

pub fn initialize_cpu_power(
    info: hyper::platform::CpuPowerInfo,
) -> Result<ArchitectureCpuPower, CpuPowerError> {
    match info {
        hyper::platform::CpuPowerInfo::Sbi(_) => sbi::bind(),
        hyper::platform::CpuPowerInfo::Psci(_) | hyper::platform::CpuPowerInfo::X86Apic(_) => {
            Err(CpuPowerError::NotSupported)
        }
    }
}

pub fn secondary_cpu_is_compatible() -> bool {
    true
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

pub fn decode_kernel_timer(
    info: hyper::platform::TimerInfo,
) -> Result<crate::arch::time::Description, crate::arch::time::DescriptionError> {
    if info.kind != hyper::platform::TimerKind::RiscvSupervisor
        || info.hypervisor_physical.trigger != hyper::platform::PlatformInterruptTrigger::Level
    {
        return Err(crate::arch::time::DescriptionError::UnsupportedTimer);
    }
    Ok(crate::arch::time::Description {
        hardware: info.hypervisor_physical,
        guest_virtual_interrupt: hyper::hal::interrupt::InterruptId::new(5),
        map_guest_virtual_interrupt: false,
    })
}

pub fn handle_guest_virtual_timer_interrupt() -> crate::kernel::irq::interrupt::HandlerResult {
    crate::kernel::irq::interrupt::HandlerResult::NotHandled
}

pub fn initialize_virtual_devices(
    _timer_interrupt: hyper::hal::interrupt::InterruptId,
) -> Result<(), VirtualDeviceInitializationError> {
    crate::println!("HypeR: RISC-V guest SBI and virtual timer backend initialized");
    Ok(())
}

pub fn enable_interrupts_for_guest_entry() {
    enable_local_irq();
}

/// Builds the permanent HS address space while translation is disabled.
///
/// # Safety
///
/// Allocator results must be directly writable, uniquely owned physical RAM.
pub unsafe fn prepare_address_space(
    allocator: &mut hyper::mm::BootAllocator,
    platform: &hyper::platform::PlatformInfo,
    image: hyper::hal::memory::KernelImageLayout,
    kernel_base: u64,
) -> Result<PreparedAddressSpace, MemoryError> {
    // SAFETY: This function forwards its directly writable allocator contract.
    unsafe { memory::prepare(allocator, platform, image, kernel_base) }
}

unsafe extern "C" {
    fn riscv64_activate_final_address_space(root: u64, kernel_base: u64, stack_top: u64) -> !;
}

/// Activates a prepared address space and switches to its permanent stack.
///
/// # Safety
///
/// `context` must come from the retained prepared address space; no live
/// reference may depend on a mapping removed by the transition.
pub unsafe fn activate_memory(context: ActivationContext) -> ! {
    // SAFETY: The caller guarantees this activation context remains backed and live.
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
        // SAFETY: WFI is valid in HS mode. A memory clobber prevents state from
        // moving across an interrupt handler that resumes this hart.
        unsafe { asm!("wfi", options(nostack)) }
    }
}
pub fn wait_for_event() {
    // SAFETY: WFI is valid in HS mode and must remain a compiler memory boundary.
    unsafe { asm!("wfi", options(nostack)) }
}

pub const fn port_io() -> Option<hyper::hal::io::PortIo> {
    None
}

pub const fn crash_stop_interrupt() -> Option<hyper::hal::interrupt::InterruptId> {
    None
}
pub const fn is_crash_stop_interrupt(_interrupt: hyper::hal::interrupt::InterruptId) -> bool {
    false
}
pub const fn broadcast_crash_stop() -> bool {
    false
}
pub fn validate_vsysreg() -> Result<(), guest::ValidationError> {
    guest::validate()
}

pub const fn guest_execution_available() -> bool {
    true
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
    crate::kernel::boot::prepare_boot_environment(crate::kernel::boot::ProtocolInputs::new(
        dtb_address,
        None,
        None,
    ))
}

pub(crate) fn describe_runtime(_emit: impl FnMut(core::fmt::Arguments<'_>)) {}

pub fn initialize_interrupt_virtualization(
    _domain: crate::kernel::irq::interrupt::IrqDomainId,
    _maintenance: Option<hyper::platform::PlatformInterrupt>,
) -> Result<(), InterruptVirtualizationError> {
    crate::println!("HypeR: RISC-V virtual interrupt injection uses H-extension HVIP state");
    Ok(())
}
