// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

#![deny(clippy::missing_safety_doc, clippy::undocumented_unsafe_blocks)]

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
mod linux;
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
mod vm_interrupt;
mod vm_vcpu;
mod vmx;

use core::arch::asm;

pub type InterruptVirtualizationError = core::convert::Infallible;

pub use barrier::X86_64Barrier as ArchitectureBarrier;
pub use cache::X86_64Cache as ArchitectureCache;
pub use context::{
    ThreadContext, UserContext, VcpuContext, VirtualInterruptError, reset_stack_and_enter,
    switch_thread_context,
};
pub use cpu_power::{Error as CpuPowerError, X2ApicCpuPower as ArchitectureCpuPower};
pub use exception::ValidationError as RuntimeVectorError;
pub use exception::{
    CrashContext, bootstrap_stack_bounds, capture_crash_context, install_exception_stacks,
    install_runtime_vectors, run_on_emergency_stack, validate_runtime_vectors,
};
pub use guest::ValidationError as GuestValidationError;
pub(crate) use guest::{GuestSyncAction, GuestSyncFrame, handle_guest_sync};
pub use interrupt_controller::{
    Error as InterruptControllerError, X2ApicController as ArchitectureInterruptController,
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
    ActivationContext, Error as MemoryError, PreparedAddressSpace, StackMapping,
    X86_64AddressTranslation as ArchitectureAddressTranslation,
};
pub use platform::{
    Error as PlatformDiscoveryError, EssentialDeviceDiscovery, EssentialPlatformInfo,
    decode_platform_interrupt,
};
pub use smp::{
    SecondaryBootParameters, current_cpu_index, current_hardware_id, secondary_entry_physical,
    send_event,
};
pub use stage2::{Error as Stage2Error, Stage2AddressSpace};
pub use timer::{
    Error as TimerError, TscCounter as ArchitectureCounter, TscDeadlineTimer as ArchitectureTimer,
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
        "x86-64 locked instructions"
    }
}

pub const fn atomic_capabilities() -> AtomicCapabilities {
    AtomicCapabilities
}

/// x86-64 enables NXE before long mode and final page tables contain no
/// writable executable leaves, so no additional per-CPU transition is needed.
pub fn enable_local_memory_protection() {}

pub const fn local_memory_protection_enabled() -> bool {
    true
}

pub fn initialize_cpu_power(
    info: hyper::platform::CpuPowerInfo,
) -> Result<ArchitectureCpuPower, CpuPowerError> {
    cpu_power::bind(info)
}

pub fn secondary_cpu_is_compatible() -> bool {
    true
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

pub fn decode_kernel_timer(
    info: hyper::platform::TimerInfo,
) -> Result<crate::arch::time::Description, crate::arch::time::DescriptionError> {
    if info.kind != hyper::platform::TimerKind::X86TscDeadline
        || info.hypervisor_physical.trigger != hyper::platform::PlatformInterruptTrigger::Edge
    {
        return Err(crate::arch::time::DescriptionError::UnsupportedTimer);
    }
    Ok(crate::arch::time::Description {
        hardware: info.hypervisor_physical,
        guest_virtual_interrupt: hyper::hal::interrupt::InterruptId::new(
            info.virtual_timer.interrupt,
        ),
        map_guest_virtual_interrupt: false,
    })
}

pub fn handle_guest_virtual_timer_interrupt() -> crate::kernel::irq::interrupt::HandlerResult {
    crate::kernel::irq::interrupt::HandlerResult::NotHandled
}

pub fn initialize_virtual_devices(
    _timer_interrupt: hyper::hal::interrupt::InterruptId,
) -> Result<(), VirtualDeviceInitializationError> {
    Ok(())
}

pub const fn enable_interrupts_for_guest_entry() {}

/// Builds the permanent host address space while using bootstrap mappings.
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
    fn x86_64_activate_final_address_space(root: u64, kernel_base: u64, stack_top: u64) -> !;
}

/// Activates a prepared address space and switches to its permanent stack.
///
/// # Safety
///
/// `context` must come from the retained prepared address space, with no live
/// reference depending on mappings removed by the transition.
pub unsafe fn activate_memory(context: ActivationContext) -> ! {
    // SAFETY: The caller guarantees this activation context remains backed and live.
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
        // SAFETY: CLI/HLT is valid at CPL0 and remains a compiler memory boundary.
        unsafe { asm!("cli", "hlt", options(nostack)) };
    }
}

pub fn wait_for_event() {
    // SAFETY: HLT is valid at CPL0 and must not move memory across interrupt wakeup.
    unsafe { asm!("hlt", options(nostack)) };
}

pub const fn port_io() -> Option<hyper::hal::io::PortIo> {
    // SAFETY: The callbacks implement byte port I/O and uphold PortIo's contract.
    Some(unsafe { hyper::hal::io::PortIo::new(read_port, write_port) })
}

unsafe fn read_port(port: u16) -> u8 {
    let value: u8;
    // SAFETY: The caller is authorized to access `port`; IN is valid at CPL0.
    unsafe { asm!("in al, dx", in("dx") port, out("al") value, options(nostack)) };
    value
}

unsafe fn write_port(port: u16, value: u8) {
    // SAFETY: The caller is authorized to access `port`; OUT is valid at CPL0.
    unsafe { asm!("out dx, al", in("dx") port, in("al") value, options(nostack)) };
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
    // SAFETY: The Linux boot protocol supplies a retained complete boot-params record.
    let inputs = match unsafe { boot_protocol::parse(boot_params) } {
        Ok(inputs) => inputs,
        Err(_) => halt(),
    };
    crate::kernel::boot::prepare_boot_environment(crate::kernel::boot::ProtocolInputs::new(
        inputs.dtb_address,
        inputs.command_line,
        inputs.initial_ramdisk,
    ))
}

pub(crate) fn describe_runtime(_emit: impl FnMut(core::fmt::Arguments<'_>)) {}

pub fn initialize_interrupt_virtualization(
    _domain: crate::kernel::irq::interrupt::IrqDomainId,
    _maintenance: Option<hyper::platform::PlatformInterrupt>,
) -> Result<(), InterruptVirtualizationError> {
    Ok(())
}
