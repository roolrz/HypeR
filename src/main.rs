#![no_std]
#![no_main]

extern crate alloc;

mod arch;
pub mod kernel;

#[cfg(feature = "kernel-self-test")]
#[path = "../tests/kernel/mod.rs"]
mod kernel_tests;

use core::panic::PanicInfo;

/// Primary kernel entry after architecture initialization is complete.
///
/// Every architecture enters here only after relocation, permanent stage-1
/// translation, runtime exception entry, and the final kernel stack are active.
#[unsafe(no_mangle)]
extern "C" fn start_kernel() -> ! {
    let mut boot = crate::kernel::boot::enter_runtime();

    crate::kernel::mm::initialize();
    crate::kernel::debug::initialize();
    crate::kernel::task::initialize();

    crate::kernel::device::initialize_cpu_power(&boot);
    crate::kernel::irq::initialize_controller(&mut boot);
    crate::kernel::crash::initialize(&boot);
    crate::kernel::irq::initialize_exceptions();
    crate::kernel::irq::initialize_virtualization(&boot);
    crate::kernel::time::initialize_timekeeping(&boot);
    crate::kernel::irq::initialize_timer(&mut boot);

    crate::kernel::vm::initialize_virtual_devices(&boot);
    crate::kernel::device::initialize_platform_devices(&boot);
    crate::kernel::cpu::initialize(&mut boot);

    #[cfg(feature = "kernel-self-test")]
    crate::kernel_tests::run();

    crate::kernel::irq::publish_online_cpu_count(&boot);
    crate::kernel::mm::finalize_address_space();
    crate::kernel::log::report_startup_state();

    crate::kernel::vm::start_default()
}

/// Rust kernel entry used by secondary CPUs after architectural setup.
#[unsafe(no_mangle)]
extern "C" fn start_secondary_cpu(cpu_index: usize) -> ! {
    if !crate::arch::secondary_cpu_is_compatible() {
        crate::arch::halt()
    }
    crate::kernel::cpu::secondary_entry(cpu_index)
}

#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    crate::kernel::crash::panic(info)
}
