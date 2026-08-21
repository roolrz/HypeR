// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Architecture-neutral SMP startup policy built on firmware CPU power calls.

use alloc::vec::Vec;
use core::convert::Infallible;

use hyper::cpu::{CpuIndex, PerCpu};
use hyper::hal::cache::{CacheError, CacheMaintenance};
use hyper::hal::cpu_power::CpuHardwareId;
use hyper::platform::PlatformInfo;
use hyper::sync::atomic::{AtomicBool, Ordering};

const CPU_ON_TIMEOUT_NS: u64 = 1_000_000_000;

static ONLINE: PerCpu<AtomicBool> =
    PerCpu::new([const { AtomicBool::new(false) }; hyper::cpu::MAX_CPUS]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Allocation,
    BootCpuMissing,
    Cache(CacheError),
    CpuIndexOverflow,
    CpuDidNotStart(usize),
    CpuPower(super::super::device::cpu_power::Error),
    InvalidAddress,
    InvalidCpuIndex,
    LocalInterrupt(super::super::irq::interrupt::Error),
    LocalTimer(super::super::irq::timer::Error),
    Time(super::super::time::Error),
    Stack(super::super::mm::stack::Error),
    Scheduler(super::super::task::scheduler::Error),
}

impl From<super::super::device::cpu_power::Error> for Error {
    fn from(error: super::super::device::cpu_power::Error) -> Self {
        Self::CpuPower(error)
    }
}

impl From<super::super::task::scheduler::Error> for Error {
    fn from(error: super::super::task::scheduler::Error) -> Self {
        Self::Scheduler(error)
    }
}

impl From<super::super::mm::stack::Error> for Error {
    fn from(error: super::super::mm::stack::Error) -> Self {
        Self::Stack(error)
    }
}

impl From<CacheError> for Error {
    fn from(error: CacheError) -> Self {
        Self::Cache(error)
    }
}

impl From<super::super::time::Error> for Error {
    fn from(error: super::super::time::Error) -> Self {
        Self::Time(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Capabilities {
    pub discovered_cpus: usize,
    pub online_cpus: usize,
}

/// Returns the number of CPUs that completed their local kernel setup.
pub(crate) fn online_cpu_count() -> usize {
    ONLINE
        .iter()
        .filter(|online| online.load(Ordering::Acquire))
        .count()
}

/// Starts every enabled CPU described by firmware, up to `CONFIG_MAX_CPUS`.
pub fn initialize(
    platform: &PlatformInfo,
    root: u64,
    image_physical_start: u64,
    kernel_virtual_base: u64,
) -> Result<Capabilities, Error> {
    if super::current_index() != Some(CpuIndex::BOOT) {
        return Err(Error::InvalidCpuIndex);
    }
    let boot_hardware_id = crate::arch::cpu::current_hardware_id();
    if !platform
        .cpus
        .as_slice()
        .iter()
        .any(|cpu| cpu.hardware_id == boot_hardware_id.get())
    {
        return Err(Error::BootCpuMissing);
    }
    ONLINE[CpuIndex::BOOT].store(true, Ordering::Release);
    let entry =
        crate::arch::cpu::secondary_entry_address(image_physical_start, kernel_virtual_base)
            .ok_or(Error::InvalidAddress)?;
    let mut next_cpu_index = 1usize;
    let mut boot_parameters = Vec::new();
    boot_parameters
        .try_reserve(platform.cpus.len().saturating_sub(1))
        .map_err(|_| Error::Allocation)?;

    for cpu in platform
        .cpus
        .as_slice()
        .iter()
        .filter(|cpu| cpu.hardware_id != boot_hardware_id.get())
    {
        let cpu_index = CpuIndex::new(next_cpu_index).ok_or(Error::CpuIndexOverflow)?;
        next_cpu_index += 1;
        if !crate::arch::cpu::register_secondary(cpu_index, CpuHardwareId::new(cpu.hardware_id)) {
            return Err(Error::CpuIndexOverflow);
        }
        super::super::mm::stack::prepare_cpu(cpu_index)?;
        let stack = super::super::task::scheduler::register_secondary_cpu(cpu_index, "idle")?;
        let mut parameters = hyper::mm::try_box(crate::arch::cpu::SecondaryBootParameters::new(
            root,
            stack.physical_top,
            stack.virtual_top as u64,
            cpu_index.get(),
        ))
        .map_err(|_| Error::Allocation)?;
        let context = super::super::mm::memory::linear_physical_address(
            (&mut *parameters) as *mut _ as usize,
        )
        .ok_or(Error::InvalidAddress)?;

        // SAFETY: The parameter record is fully initialized, remains owned by
        // this boot path, and is not modified until the secondary has consumed
        // it. Publish dirty cache lines before firmware starts a CPU that may
        // not yet participate in the boot CPU's coherent cache domain.
        unsafe {
            crate::arch::memory::Cache::publish_data_range(
                (&*parameters) as *const _ as usize,
                core::mem::size_of::<crate::arch::cpu::SecondaryBootParameters>(),
            )?;
        }

        // SAFETY: `entry` is the architecture-provided secondary trampoline.
        // The boxed, cache-published parameter record remains pinned in
        // `boot_parameters` until every secondary reports online.
        unsafe {
            super::super::device::cpu_power::cpu_on(
                CpuHardwareId::new(cpu.hardware_id),
                entry,
                context,
            )?
        };
        boot_parameters.push(parameters);
    }

    for (cpu_index, online) in ONLINE.iter().enumerate().take(next_cpu_index).skip(1) {
        if !super::super::time::spin_wait_until(CPU_ON_TIMEOUT_NS, || {
            online.load(Ordering::Acquire)
        })? {
            return Err(Error::CpuDidNotStart(cpu_index));
        }
    }
    drop(boot_parameters);

    Ok(Capabilities {
        discovered_cpus: platform.cpus.len(),
        online_cpus: next_cpu_index,
    })
}

/// Completes local interrupt/timer setup and becomes this CPU's idle thread.
pub fn secondary_entry(cpu_index: usize) -> ! {
    match try_secondary_entry(cpu_index) {
        Ok(never) => match never {},
        Err(error) => {
            crate::pr_crit!("HypeR: CPU {cpu_index} initialization failed: {error:?}");
            crate::arch::cpu::halt()
        }
    }
}

fn try_secondary_entry(cpu_index: usize) -> Result<Infallible, Error> {
    let cpu_index = CpuIndex::new(cpu_index).ok_or(Error::InvalidCpuIndex)?;
    if super::current_index() != Some(cpu_index) {
        return Err(Error::InvalidCpuIndex);
    }
    super::super::irq::interrupt::initialize_local_cpu().map_err(Error::LocalInterrupt)?;
    super::super::irq::timer::initialize_local_cpu().map_err(Error::LocalTimer)?;
    let stack = super::super::task::scheduler::install_current_idle()?;
    // SAFETY: This CPU still has local interrupts masked, `stack` belongs to
    // its current idle Thread, and the continuation never returns.
    unsafe { super::super::mm::stack::reset_and_enter(stack, enter_clean_idle, cpu_index.get()) }
}

extern "C" fn enter_clean_idle(cpu_index: usize) -> ! {
    let Some(cpu_index) = CpuIndex::new(cpu_index) else {
        crate::arch::cpu::halt()
    };
    if super::current_index() != Some(cpu_index) {
        crate::arch::cpu::halt()
    }
    crate::arch::cpu::mark_current_online();
    ONLINE[cpu_index].store(true, Ordering::Release);
    crate::arch::cpu::send_event();
    crate::println!(
        "HypeR: CPU {} online, hardware ID {:#x}; entering idle",
        cpu_index.get(),
        crate::arch::cpu::current_hardware_id().get()
    );
    crate::arch::irq::enable_local();
    super::super::task::scheduler::run_idle_loop()
}
