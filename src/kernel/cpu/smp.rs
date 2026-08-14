//! Architecture-neutral SMP startup policy built on firmware CPU power calls.

use alloc::boxed::Box;
use alloc::format;
use alloc::vec::Vec;
use core::hint::spin_loop;

use hyper::hal::cache::{CacheError, CacheMaintenance};
use hyper::hal::cpu_power::{CpuHardwareId, ResumeAddress};
use hyper::platform::PlatformInfo;
use hyper::sync::atomic::{AtomicBool, Ordering};

const MAX_CPUS: usize = hyper::config::MAX_CPUS as usize;
const ONLINE_WAIT_LIMIT: usize = 20_000_000;

static ONLINE: [AtomicBool; MAX_CPUS] = [const { AtomicBool::new(false) }; MAX_CPUS];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Allocation,
    BootCpuMissing,
    Cache(CacheError),
    CpuIndexOverflow,
    CpuDidNotStart(usize),
    CpuPower(super::super::device::cpu_power::Error),
    InvalidAddress,
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

impl From<CacheError> for Error {
    fn from(error: CacheError) -> Self {
        Self::Cache(error)
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

/// Starts every enabled CPU described by firmware, up to CONFIG_MAX_CPUS.
pub fn initialize(
    platform: &PlatformInfo,
    root: u64,
    image_physical_start: u64,
) -> Result<Capabilities, Error> {
    let boot_hardware_id = crate::arch::current_hardware_id();
    if !platform
        .cpus
        .as_slice()
        .iter()
        .any(|cpu| cpu.hardware_id == boot_hardware_id)
    {
        return Err(Error::BootCpuMissing);
    }
    ONLINE[0].store(true, Ordering::Release);
    let entry =
        crate::arch::secondary_entry_physical(image_physical_start).ok_or(Error::InvalidAddress)?;
    let mut next_cpu_index = 1usize;
    let mut boot_parameters = Vec::new();
    boot_parameters
        .try_reserve(platform.cpus.len().saturating_sub(1))
        .map_err(|_| Error::Allocation)?;

    for cpu in platform
        .cpus
        .as_slice()
        .iter()
        .filter(|cpu| cpu.hardware_id != boot_hardware_id)
    {
        if next_cpu_index >= MAX_CPUS {
            return Err(Error::CpuIndexOverflow);
        }
        let cpu_index = next_cpu_index;
        next_cpu_index += 1;
        let name = format!("idle/{cpu_index}");
        let virtual_stack_top =
            super::super::task::scheduler::register_secondary_cpu(cpu_index, &name)?;
        let physical_stack_top =
            super::super::mm::memory::linear_physical_address(virtual_stack_top)
                .ok_or(Error::InvalidAddress)?;
        let mut parameters = Box::new(crate::arch::SecondaryBootParameters::new(
            root,
            physical_stack_top,
            virtual_stack_top as u64,
            cpu_index,
        ));
        let context = super::super::mm::memory::linear_physical_address(
            (&mut *parameters) as *mut _ as usize,
        )
        .ok_or(Error::InvalidAddress)?;

        // SAFETY: The parameter record is fully initialized, remains owned by
        // this boot path, and is not modified until the secondary has consumed
        // it. Cleaning to PoC is required because PSCI starts the secondary
        // with data caching disabled.
        unsafe {
            crate::arch::ArchitectureCache::publish_data_range(
                (&*parameters) as *const _ as usize,
                core::mem::size_of::<crate::arch::SecondaryBootParameters>(),
            )?;
        }

        super::super::device::cpu_power::cpu_on(
            CpuHardwareId::new(cpu.hardware_id),
            ResumeAddress::new(entry),
            context,
        )?;
        boot_parameters.push(parameters);
    }

    for (cpu_index, online) in ONLINE.iter().enumerate().take(next_cpu_index).skip(1) {
        let mut remaining = ONLINE_WAIT_LIMIT;
        while !online.load(Ordering::Acquire) {
            if remaining == 0 {
                return Err(Error::CpuDidNotStart(cpu_index));
            }
            remaining -= 1;
            spin_loop();
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
    if cpu_index >= MAX_CPUS {
        crate::pr_crit!("HypeR: secondary CPU index {cpu_index} exceeds CONFIG_MAX_CPUS");
        crate::arch::halt()
    }
    if let Err(error) = super::super::irq::interrupt::initialize_local_cpu() {
        crate::pr_crit!("HypeR: CPU {cpu_index} local GIC initialization failed: {error:?}");
        crate::arch::halt()
    }
    if let Err(error) = super::super::irq::timer::initialize_local_cpu() {
        crate::pr_crit!("HypeR: CPU {cpu_index} local timer initialization failed: {error:?}");
        crate::arch::halt()
    }
    if let Err(error) = super::super::task::scheduler::install_current_idle() {
        crate::pr_crit!("HypeR: CPU {cpu_index} idle-thread installation failed: {error:?}");
        crate::arch::halt()
    }
    ONLINE[cpu_index].store(true, Ordering::Release);
    crate::arch::send_event();
    crate::println!(
        "HypeR: CPU {cpu_index} online, MPIDR affinity {:#x}; entering idle",
        crate::arch::current_hardware_id()
    );
    crate::arch::enable_local_irq();
    super::super::task::scheduler::run_idle_loop()
}
