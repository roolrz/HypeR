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
mod user;
mod user_contract;
mod user_entry;
mod user_machine;
mod vgic;
mod vm_interrupt;
mod vm_timer;
mod vm_vcpu;
mod vsysreg;

pub use atomics::{AtomicCapabilities, capabilities as atomic_capabilities};
pub use cache::Aarch64Cache as ArchitectureCache;
pub use context::{
    ThreadContext, VcpuContext, reset_stack_and_enter, run_on_emergency_stack,
    switch_thread_context,
};
pub use exception::ValidationError as RuntimeVectorError;
pub use exception::{
    CrashContext, capture_crash_context, install_exception_stacks, install_local_runtime_vectors,
    install_runtime_vectors, validate_local_runtime_vectors, validate_runtime_vectors,
};
pub use gic_cpu_interface::{
    Aarch64GicCpuInterface, acknowledge_interrupt, broadcast_crash_stop, crash_stop_interrupt,
    current_gic_affinity, end_interrupt, is_crash_stop_interrupt, kernel_rpc_interrupt,
    notify_kernel_rpc, notify_reschedule, reschedule_interrupt,
};
pub fn take_kernel_rpc_reasons() -> u8 {
    smp::take_kernel_rpc()
}
pub const fn arm_kernel_rpc_source() {}
pub use host::mode_name as host_execution_mode_name;
pub use hyper::drivers::power::psci::Error as CpuPowerError;
pub use interrupt_controller::{
    Aarch64InterruptController as ArchitectureInterruptController,
    Error as InterruptControllerError,
};
pub(crate) use interrupt_virtualization::description as interrupt_virtualization_description;
pub use interrupt_virtualization::{
    Error as InterruptVirtualizationError, initialize as initialize_interrupt_virtualization,
};
pub use interrupts::{
    LocalInterruptMask, disable_all as disable_all_interrupts, enable_irq as enable_local_irq,
    irq_enabled as local_irq_enabled, mask_irq as mask_local_irq,
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
pub use user::{UserMachineContractError, user_address_limit};
pub(crate) use user::{assert_kernel_pan, uses_vhe_translation as user_uses_vhe_translation};
pub(crate) use user::{copy_from_exposed, copy_to_exposed};
pub(crate) use user_entry::{
    CompletionFailure as UserCompletionFailure, Error as UserEntryError,
    ReturnCapability as UserReturnCapability, UserContext, UserExit, run_user,
};
pub(crate) use user_machine::{
    Error as UserAddressSpaceError, LocalActivation as UserLocalActivation,
    LocalIdentity as UserLocalIdentity, LocalOperation as UserLocalOperation,
    LocalRequest as UserLocalRequest, MappingPage as UserMappingPage,
    PreparedAddressSpace as PreparedUserAddressSpace, activate_local as activate_user_local,
    deactivate_local as deactivate_user_local,
    local_identity_is_active as user_local_identity_is_active,
    prepare_nvhe as prepare_nvhe_user_address_space, prepare_vhe as prepare_vhe_user_address_space,
    service_local_request as service_user_local_request,
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
    activate as activate_vcpu_hardware, deactivate as deactivate_vcpu_hardware,
    deliver_software_interrupt as deliver_guest_software_interrupt, handle_guest_device_access,
    handle_maintenance_interrupt as handle_virtualization_maintenance_interrupt,
    handle_virtual_timer_interrupt as handle_guest_virtual_timer_interrupt,
    inject_timer_for_validation,
    maintenance_interrupt_pending as virtualization_maintenance_pending,
    quiesce_virtual_interrupt_delivery, update_guest_device_interrupt,
};
pub(crate) use vsysreg::{
    GuestSyncAction, GuestSyncFrame, complete_guest_mmio_access, decode_guest_memory_fault,
    decode_guest_mmio_access, handle_guest_sync,
};
pub use vsysreg::{ValidationError as GuestValidationError, validate as validate_vsysreg};

#[cfg(feature = "kernel-self-test")]
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
        && cache::current_cpu_is_compatible()
        && host::current_cpu_is_compatible()
        && user::current_cpu_is_compatible()
}

pub fn register_secondary_hardware_id(cpu_index: usize, hardware_id: u64) -> bool {
    smp::register_cpu(cpu_index, hardware_id)
}

pub fn mark_current_cpu_online() {}

pub fn prepare_timekeeping(_platform: &EssentialPlatformInfo) -> Result<(), TimerError> {
    Ok(())
}

pub fn prepare_cache(
    _platform: &EssentialPlatformInfo,
) -> Result<(), hyper::hal::cache::CacheError> {
    cache::prepare_boot_cpu()
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

pub use vm_vcpu::InitializationError as VirtualDeviceInitializationError;

pub const GUEST_CONSOLE_BASE: u64 = hyper::vm::aarch64::device::pl011::REFERENCE_BASE;
pub const GUEST_CONSOLE_SIZE: u64 = hyper::vm::aarch64::device::pl011::REFERENCE_SIZE;
pub const GUEST_CONSOLE_INTERRUPT: u32 = hyper::vm::aarch64::device::pl011::REFERENCE_INTERRUPT;

pub fn initialize_virtual_devices(
    _timer_interrupt: hyper::hal::interrupt::InterruptId,
    host_timer_interrupt: Option<hyper::hal::interrupt::HostInterruptBinding>,
) -> Result<(), VirtualDeviceInitializationError> {
    host_timer_interrupt.ok_or(vm_vcpu::InitializationError::MissingHostTimerInterrupt)?;
    Ok(())
}

/// Applies the local interrupt state required immediately before guest entry.
///
/// `AArch64` must keep IRQs masked until `ERET`. Once `ELR_EL2` and `SPSR_EL2`
/// contain guest state, an IRQ taken by the entry trampoline would overwrite
/// those registers with its host return state. The exception tail could then
/// save that host state as the guest context and resume through a corrupted
/// entry transaction. Interrupts routed to EL2 remain able to preempt the
/// lower-EL guest after `ERET`.
pub const fn prepare_interrupts_for_guest_entry() {}

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

/// Waits for an interrupt after scheduler work was checked with IRQs masked.
///
/// Keeping DAIF masked across the queue check and WFI closes the idle
/// lost-wakeup window. A pending IRQ wakes WFI and remains pending until the
/// caller restores its exact saved mask state.
pub fn wait_for_interrupt_masked() {
    // SAFETY: WFI is valid at EL2. The caller keeps local exceptions masked,
    // so an interrupt can wake this CPU without entering its handler here.
    unsafe { core::arch::asm!("wfi", options(nostack, preserves_flags)) };
}

pub const fn port_io() -> Option<hyper::hal::io::PortIo> {
    None
}

pub const fn service_stage1_tlb_shootdown() -> bool {
    true
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
    if !smp::initialize_boot_cpu() {
        halt()
    }
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
