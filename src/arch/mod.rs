#[cfg(all(target_arch = "aarch64", not(CONFIG_ARCH_AARCH64)))]
compile_error!("the AArch64 target requires CONFIG_ARCH_AARCH64=y");

#[cfg(target_arch = "aarch64")]
pub(crate) mod aarch64;
#[cfg(target_arch = "riscv64")]
pub(crate) mod riscv64;

#[cfg(all(target_arch = "riscv64", not(CONFIG_ARCH_RISCV64)))]
compile_error!("the RISC-V target requires CONFIG_ARCH_RISCV64=y");

#[cfg(target_arch = "aarch64")]
pub(crate) use aarch64::{
    GuestDataAccess, GuestMemoryAccess, GuestSyncAction, GuestSyncFrame, GuestTranslationFault,
    handle_guest_sync,
};
#[cfg(target_arch = "riscv64")]
pub(crate) use riscv64::{
    GuestMemoryAccess, GuestSyncAction, GuestSyncFrame, GuestTranslationFault, handle_guest_sync,
};

#[cfg(target_arch = "aarch64")]
pub use aarch64::VgicError as VirtualInterruptError;
#[cfg(target_arch = "aarch64")]
pub use aarch64::{
    Aarch64Barrier as ArchitectureBarrier, Aarch64Cache as ArchitectureCache, ActivationContext,
    ArchitectureAddressTranslation, ArchitectureCpuPower, ArchitectureInterruptController,
    ArmGenericCounter as ArchitectureCounter, AtomicCapabilities, CpuPowerError, CrashContext,
    El2PhysicalTimer as ArchitectureTimer, EssentialDeviceDiscovery, EssentialPlatformInfo,
    InterruptControllerError, LocalInterruptMask, MemoryError, PreparedAddressSpace,
    SecondaryBootParameters, StackMapping, Stage2AddressSpace, Stage2Error, ThreadContext,
    TimerError, UserContext, VcpuContext, VgicError, activate_memory, atomic_capabilities,
    bootstrap_stack_bounds, broadcast_crash_stop, capture_crash_context, crash_stop_interrupt,
    current_cpu_index, current_hardware_id, decode_platform_interrupt, disable_local_interrupts,
    disable_vgic, enable_local_irq, halt, initialize_cpu_power, install_exception_stacks,
    install_runtime_vectors, interrupt_is_per_cpu, is_crash_stop_interrupt, local_irq_enabled,
    poll_guest_timer, prepare_address_space, prepare_timekeeping, register_secondary_hardware_id,
    reset_stack_and_enter, run_on_emergency_stack, secondary_cpu_is_compatible,
    secondary_entry_physical, select_kaslr_layout, send_event, switch_thread_context,
    take_guest_timer_wakeup, validate_runtime_vectors, validate_vgic, validate_vsysreg,
    vgic_maintenance_state, wait_for_event,
};
#[cfg(target_arch = "riscv64")]
pub use riscv64::{
    ActivationContext, ArchitectureAddressTranslation, ArchitectureBarrier, ArchitectureCache,
    ArchitectureCounter, ArchitectureCpuPower, ArchitectureInterruptController, ArchitectureTimer,
    AtomicCapabilities, CpuPowerError, CrashContext, EssentialDeviceDiscovery,
    EssentialPlatformInfo, InterruptControllerError, LocalInterruptMask, MemoryError,
    PreparedAddressSpace, SecondaryBootParameters, StackMapping, Stage2AddressSpace, Stage2Error,
    ThreadContext, TimerError, UserContext, VcpuContext, VirtualInterruptError, activate_memory,
    atomic_capabilities, bootstrap_stack_bounds, broadcast_crash_stop, capture_crash_context,
    crash_stop_interrupt, current_cpu_index, current_hardware_id, decode_platform_interrupt,
    disable_local_interrupts, enable_local_irq, halt, initialize_cpu_power,
    install_exception_stacks, install_runtime_vectors, interrupt_is_per_cpu, local_irq_enabled,
    poll_guest_timer, prepare_address_space, prepare_timekeeping, register_secondary_hardware_id,
    reset_stack_and_enter, run_on_emergency_stack, secondary_cpu_is_compatible,
    secondary_entry_physical, select_kaslr_layout, send_event, switch_thread_context,
    take_guest_timer_wakeup, validate_runtime_vectors, validate_vsysreg, wait_for_event,
};

#[cfg(all(target_arch = "aarch64", CONFIG_CRASH_CONSOLE))]
pub use aarch64::inspect_stage1_mapping;
#[cfg(all(target_arch = "riscv64", CONFIG_CRASH_CONSOLE))]
pub use riscv64::inspect_stage1_mapping;
