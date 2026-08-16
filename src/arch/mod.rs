#[cfg(all(target_arch = "aarch64", not(CONFIG_ARCH_AARCH64)))]
compile_error!("the AArch64 target requires CONFIG_ARCH_AARCH64=y");

#[cfg(target_arch = "aarch64")]
pub(crate) mod aarch64;
#[cfg(target_arch = "riscv64")]
pub(crate) mod riscv64;
#[cfg(target_arch = "x86_64")]
pub(crate) mod x86_64;

#[cfg(all(target_arch = "riscv64", not(CONFIG_ARCH_RISCV64)))]
compile_error!("the RISC-V target requires CONFIG_ARCH_RISCV64=y");

#[cfg(all(target_arch = "x86_64", not(CONFIG_ARCH_X86_64)))]
compile_error!("the x86-64 target requires CONFIG_ARCH_X86_64=y");

#[cfg(target_arch = "aarch64")]
use aarch64 as imp;
#[cfg(target_arch = "riscv64")]
use riscv64 as imp;
#[cfg(target_arch = "x86_64")]
use x86_64 as imp;

pub(crate) use imp::{
    GuestMemoryAccess, GuestSyncAction, GuestSyncFrame, GuestTranslationFault, handle_guest_sync,
};

pub use imp::{
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
    mark_current_cpu_online, poll_guest_timer, port_io, prepare_address_space, prepare_cache,
    prepare_timekeeping, register_secondary_hardware_id, reset_stack_and_enter,
    run_on_emergency_stack, secondary_cpu_is_compatible, secondary_entry_physical,
    select_kaslr_layout, send_event, switch_thread_context, take_guest_timer_wakeup,
    validate_runtime_vectors, validate_vsysreg, wait_for_event,
};

#[cfg(target_arch = "x86_64")]
pub use imp::virtualization_backend_name;

#[cfg(CONFIG_CRASH_CONSOLE)]
pub use imp::inspect_stage1_mapping;

#[cfg(target_arch = "aarch64")]
pub(crate) use imp::GuestDataAccess;
#[cfg(target_arch = "aarch64")]
pub use imp::{
    VgicError, disable_vgic, is_crash_stop_interrupt, validate_vgic, vgic_maintenance_state,
};
