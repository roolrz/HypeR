// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

#![no_std]
#![no_main]

extern crate alloc;

mod arch;
pub mod kernel;

#[cfg(feature = "kernel-self-test")]
#[path = "../tests/kernel/mod.rs"]
mod kernel_tests;

use core::convert::Infallible;
use core::panic::PanicInfo;

enum KernelStartError {
    Boot(crate::kernel::boot::RuntimeError),
    Cpu(crate::kernel::cpu::Error),
    Crash(crate::kernel::crash::InitializationError),
    Debug(crate::kernel::debug::InitializationError),
    Device(crate::kernel::device::InitializationError),
    Interrupt(crate::kernel::irq::InitializationError),
    Memory(crate::kernel::mm::InitializationError),
    MemoryFinalization(crate::kernel::mm::FinalizationError),
    Scheduler(crate::kernel::task::scheduler::Error),
    Time(crate::kernel::time::Error),
    VirtualDevices(crate::kernel::vm::InitializationError),
    VirtualMachine(crate::kernel::vm::StartError),
}

macro_rules! impl_kernel_start_error {
    ($($variant:ident($error:ty)),+ $(,)?) => {
        $(
            impl From<$error> for KernelStartError {
                fn from(error: $error) -> Self {
                    Self::$variant(error)
                }
            }
        )+
    };
}

impl_kernel_start_error! {
    Boot(crate::kernel::boot::RuntimeError),
    Cpu(crate::kernel::cpu::Error),
    Crash(crate::kernel::crash::InitializationError),
    Debug(crate::kernel::debug::InitializationError),
    Device(crate::kernel::device::InitializationError),
    Interrupt(crate::kernel::irq::InitializationError),
    Memory(crate::kernel::mm::InitializationError),
    MemoryFinalization(crate::kernel::mm::FinalizationError),
    Scheduler(crate::kernel::task::scheduler::Error),
    Time(crate::kernel::time::Error),
    VirtualDevices(crate::kernel::vm::InitializationError),
    VirtualMachine(crate::kernel::vm::StartError),
}

impl core::fmt::Debug for KernelStartError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let (stage, error): (&str, &dyn core::fmt::Debug) = match self {
            Self::Boot(error) => ("boot", error),
            Self::Cpu(error) => ("cpu", error),
            Self::Crash(error) => ("crash", error),
            Self::Debug(error) => ("debug", error),
            Self::Device(error) => ("device", error),
            Self::Interrupt(error) => ("interrupt", error),
            Self::Memory(error) => ("memory", error),
            Self::MemoryFinalization(error) => ("memory-finalization", error),
            Self::Scheduler(error) => ("scheduler", error),
            Self::Time(error) => ("time", error),
            Self::VirtualDevices(error) => ("virtual-devices", error),
            Self::VirtualMachine(error) => ("virtual-machine", error),
        };
        formatter
            .debug_struct("KernelStartError")
            .field("stage", &stage)
            .field("error", error)
            .finish()
    }
}

/// Primary kernel entry after architecture initialization is complete.
///
/// Every architecture enters here only after relocation, permanent stage-1
/// translation, runtime exception entry, and the final kernel stack are active.
#[unsafe(no_mangle)]
extern "C" fn start_kernel() -> ! {
    let result: Result<Infallible, KernelStartError> = (|| {
        let mut boot = crate::kernel::boot::enter_runtime()?;

        crate::kernel::mm::initialize()?;
        crate::kernel::debug::initialize()?;
        crate::kernel::task::initialize()?;

        crate::kernel::device::initialize_cpu_power(&boot)?;
        crate::kernel::irq::initialize_controller(&mut boot)?;
        crate::kernel::irq::initialize_exceptions()?;
        crate::kernel::crash::initialize(&boot)?;
        crate::kernel::irq::initialize_virtualization(&boot)?;
        crate::kernel::time::initialize_timekeeping(&boot)?;
        crate::kernel::irq::initialize_timer(&mut boot)?;

        crate::kernel::vm::initialize_virtual_devices(&boot)?;
        crate::kernel::device::initialize_platform_devices(&boot)?;
        crate::kernel::cpu::initialize(&mut boot)?;

        crate::kernel::irq::publish_online_cpu_count(&boot)?;
        crate::kernel::mm::finalize_address_space()?;

        #[cfg(feature = "kernel-self-test")]
        crate::kernel_tests::run();

        crate::kernel::log::report_startup_state();

        let never = crate::kernel::vm::start_default()?;
        match never {}
    })();

    match result {
        Ok(never) => match never {},
        Err(error) => crate::kernel::boot::fail("kernel startup", error),
    }
}

/// Rust kernel entry used by secondary CPUs after architectural setup.
#[unsafe(no_mangle)]
extern "C" fn start_secondary_cpu(cpu_index: usize) -> ! {
    if !crate::arch::cpu::secondary_is_compatible() {
        crate::arch::cpu::halt()
    }
    crate::arch::memory::enable_local_protection();
    if !crate::arch::memory::local_protection_enabled() {
        crate::arch::cpu::halt()
    }
    crate::kernel::cpu::secondary_entry(cpu_index)
}

#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    crate::kernel::crash::panic(info)
}
