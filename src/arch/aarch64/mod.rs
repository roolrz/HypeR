// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

mod address;
mod atomics;
mod barrier;
mod cache;
mod context;
mod exception;
mod gic_cpu_interface;
mod host;
mod interrupt_controller;
mod interrupt_virtualization;
mod interrupts;
mod kaslr;
mod linux;
mod memory;
mod platform;
mod protection;
mod psci;
pub mod registers;
mod smp;
mod stage2;
mod timer;
mod vgic;
mod vm_interrupt;
mod vm_timer;
mod vm_vcpu;
mod vsysreg;

pub use atomics::{AtomicCapabilities, capabilities as atomic_capabilities};
pub use barrier::Aarch64Barrier as ArchitectureBarrier;
pub use cache::Aarch64Cache as ArchitectureCache;
pub use context::{
    ThreadContext, UserContext, VcpuContext, reset_stack_and_enter, run_on_emergency_stack,
    switch_thread_context,
};
pub use exception::ValidationError as RuntimeVectorError;
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
pub use interrupt_virtualization::{
    Error as InterruptVirtualizationError, initialize as initialize_interrupt_virtualization,
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
    Aarch64AddressTranslation as ArchitectureAddressTranslation, ActivationContext,
    Error as MemoryError, PreparedAddressSpace, StackMapping, bootstrap_stack_bounds,
};
pub use platform::{
    Error as PlatformDiscoveryError, EssentialDeviceDiscovery, EssentialPlatformInfo,
    decode_platform_interrupt,
};
pub use protection::{enable_local_memory_protection, local_memory_protection_enabled};
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
pub use vm_interrupt::{Error as VmInterruptError, VmInterruptController};
pub use vm_vcpu::Error as VcpuInterruptError;
pub(crate) use vm_vcpu::{
    deliver_software_interrupt as deliver_guest_software_interrupt, handle_guest_device_access,
    update_guest_device_interrupt,
};
pub(crate) use vsysreg::{
    GuestSyncAction, GuestSyncFrame, complete_guest_mmio_access, decode_guest_memory_fault,
    decode_guest_mmio_access, handle_guest_sync,
};
pub use vsysreg::{ValidationError as GuestValidationError, validate as validate_vsysreg};

pub const fn guest_execution_available() -> bool {
    true
}

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
    address::current_cpu_is_compatible()
        && atomics::current_cpu_supports_selected_backend()
        && host::current_cpu_is_compatible()
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

pub fn decode_kernel_timer(
    info: hyper::platform::TimerInfo,
) -> Result<crate::arch::time::Description, crate::arch::time::DescriptionError> {
    if info.kind != hyper::platform::TimerKind::ArmGeneric {
        return Err(crate::arch::time::DescriptionError::UnsupportedTimer);
    }
    if info.hypervisor_physical.trigger != hyper::platform::PlatformInterruptTrigger::Level
        || info.virtual_timer.trigger != hyper::platform::PlatformInterruptTrigger::Level
    {
        return Err(crate::arch::time::DescriptionError::InvalidInterruptTrigger);
    }
    Ok(crate::arch::time::Description {
        hardware: info.hypervisor_physical,
        guest_virtual_interrupt: hyper::hal::interrupt::InterruptId::new(
            info.virtual_timer.interrupt,
        ),
        map_guest_virtual_interrupt: true,
    })
}

pub fn handle_guest_virtual_timer_interrupt() -> crate::kernel::irq::interrupt::HandlerResult {
    use crate::kernel::irq::interrupt::HandlerResult;

    match vm_timer::handle_interrupt() {
        Ok(outcome) if outcome.active && outcome.asserted => HandlerResult::HandledAndMaskLocal,
        Ok(outcome) if outcome.active => HandlerResult::Handled,
        Ok(_) => {
            crate::pr_warn!("HypeR: masked virtual timer PPI without an active vCPU");
            HandlerResult::HandledAndMaskLocal
        }
        Err(error) => {
            disable_vgic();
            crate::pr_err!("HypeR: virtual timer injection failed: {error:?}");
            HandlerResult::HandledAndMaskLocal
        }
    }
}

pub use vm_vcpu::InitializationError as VirtualDeviceInitializationError;

pub const GUEST_CONSOLE_BASE: u64 = hyper::vm::aarch64::device::pl011::REFERENCE_BASE;
pub const GUEST_CONSOLE_SIZE: u64 = hyper::vm::aarch64::device::pl011::REFERENCE_SIZE;
pub const GUEST_CONSOLE_INTERRUPT: u32 = hyper::vm::aarch64::device::pl011::REFERENCE_INTERRUPT;

pub fn initialize_virtual_devices(
    timer_interrupt: hyper::hal::interrupt::InterruptId,
) -> Result<(), VirtualDeviceInitializationError> {
    vm_vcpu::validate_arch_timer(timer_interrupt)?;
    crate::println!("HypeR: virtual architected timer injection validated");
    Ok(())
}

pub fn enable_interrupts_for_guest_entry() {
    enable_local_irq();
}

unsafe extern "C" {
    fn aarch64_activate_final_address_space(
        root: u64,
        virtual_base: u64,
        stack_top: u64,
        tcr_el2: u64,
    ) -> !;
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
    // SAFETY: This function forwards the bootstrap identity-map and allocator
    // validity contract directly to the architecture memory builder.
    unsafe { memory::prepare(allocator, platform, image, kernel_base) }
}

/// Activates final stage-1 translation and enters the high kernel alias.
///
/// # Safety
///
/// `memory` must remain installed in global boot state for the lifetime of the
/// active address space.
pub unsafe fn activate_memory(context: ActivationContext) -> ! {
    // SAFETY: The caller guarantees that the context names the globally pinned
    // final hierarchy, high alias, and mapped stack consumed by this transition.
    unsafe {
        aarch64_activate_final_address_space(
            context.root.get(),
            context.kernel_base,
            context.stack_top.get(),
            context.tcr_el2,
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
    unsafe { core::arch::asm!("wfi", options(nostack, preserves_flags)) };
}

/// Suspends until an event or interrupt; paired with `send_event` for work.
pub fn wait_for_event() {
    // SAFETY: WFE only affects the current processing element's event state.
    unsafe { core::arch::asm!("wfe", options(nostack, preserves_flags)) };
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
    address::initialize().unwrap_or_else(|_| halt());
    atomics::initialize();
    host::initialize();
    crate::kernel::boot::prepare_boot_environment(crate::kernel::boot::ProtocolInputs::new(
        dtb_address,
        None,
        None,
    ))
}

pub(crate) fn describe_runtime(mut emit: impl FnMut(core::fmt::Arguments<'_>)) {
    let address = address::capabilities();
    emit(format_args!(
        "HypeR: AArch64 host execution mode: {}",
        host_execution_mode_name()
    ));
    emit(format_args!(
        "HypeR: AArch64 execution protection: {}, WXN=on",
        if host::is_vhe() { "PXN/UXN" } else { "XN" }
    ));
    emit(format_args!(
        "HypeR: AArch64 address space: {}-bit VA/{} levels, {}-bit PA (CPU {}-bit), {}-bit IPA/{} levels",
        address.virtual_address_bits,
        address.stage1_levels(),
        address.physical_address_bits,
        address.supported_physical_address_bits,
        address.intermediate_physical_address_bits,
        address.stage2_levels(),
    ));
}

pub fn poll_guest_timer(_now: u64) {}
pub const fn take_guest_timer_wakeup() -> Option<u64> {
    None
}
