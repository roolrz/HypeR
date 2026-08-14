#[cfg(all(target_arch = "aarch64", not(CONFIG_ARCH_AARCH64)))]
compile_error!("the AArch64 target requires CONFIG_ARCH_AARCH64=y");

#[cfg(target_arch = "aarch64")]
pub(crate) mod aarch64;

#[cfg(target_arch = "aarch64")]
pub use aarch64::{
    Aarch64Barrier as ArchitectureBarrier, Aarch64Cache as ArchitectureCache,
    Aarch64GicCpuInterface as GicCpuInterface, ActivationContext, ArchitectureAddressTranslation,
    ArchitectureCpuPower, AtomicCapabilities, CpuPowerError, El2PhysicalTimer as ArchitectureTimer,
    EssentialDeviceDiscovery, EssentialPlatformInfo, LocalInterruptMask, MemoryError,
    PreparedAddressSpace, SecondaryBootParameters, ThreadContext, TimerError, UserContext,
    VcpuContext, activate_memory, atomic_capabilities, current_cpu_index, current_gic_affinity,
    current_hardware_id, enable_local_irq, halt, initialize_cpu_power, install_runtime_vectors,
    prepare_address_space, secondary_entry_physical, send_event, switch_thread_context,
    validate_runtime_vectors, wait_for_interrupt,
};
