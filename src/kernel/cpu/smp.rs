// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Architecture-neutral SMP startup policy built on firmware CPU power calls.

use alloc::vec::Vec;
use core::convert::Infallible;
use core::mem::{align_of, size_of};
use core::ptr::NonNull;

use hyper::cpu::{CpuIndex, PerCpu};
use hyper::hal::cache::CacheError;
use hyper::hal::cpu_power::CpuHardwareId;
use hyper::mm::PAGE_SIZE;
use hyper::platform::PlatformInfo;
use hyper::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::kernel::mm::page_block::PageBlock;

const CPU_ON_TIMEOUT_NS: u64 = 1_000_000_000;

static ONLINE: PerCpu<AtomicBool> =
    PerCpu::new([const { AtomicBool::new(false) }; hyper::cpu::MAX_CPUS]);
static PARTICIPATING_CPU_COUNT: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Allocation,
    AlreadyInitialized,
    BootCpuMissing,
    Cache(CacheError),
    CpuIndexOverflow,
    InvalidCpuRoute(usize),
    CpuDidNotStart(usize),
    CpuPower(super::super::device::cpu_power::Error),
    InvalidHandoffLayout,
    InvalidAddress,
    InvalidCpuIndex,
    InvalidTopology,
    LocalInterrupt(super::super::irq::interrupt::Error),
    LocalTimer(super::super::time::TickError),
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

/// Exclusive, cache-line-contained storage observed by one secondary CPU.
///
/// The architecture record cannot live in a slab allocation: cache
/// publication operates on complete lines and a slab object may share those
/// lines with allocator metadata or another object. A dedicated buddy block
/// gives this owner every line touched by publication.
struct SecondaryBootHandoff {
    parameters: NonNull<crate::hal::cpu::SecondaryBootParameters>,
    context: u64,
    _block: PageBlock,
}

impl SecondaryBootHandoff {
    fn new(parameters: crate::hal::cpu::SecondaryBootParameters) -> Result<Self, Error> {
        let line_size = crate::hal::cache::data_line_size();
        if line_size == 0 || !line_size.is_power_of_two() {
            return Err(Error::Cache(CacheError::InvalidLineSize));
        }
        let parameter_size = size_of::<crate::hal::cpu::SecondaryBootParameters>();
        if parameter_size == 0 {
            return Err(Error::InvalidHandoffLayout);
        }
        let published_size =
            align_up(parameter_size, line_size).ok_or(Error::InvalidHandoffLayout)?;
        let placement_alignment =
            line_size.max(align_of::<crate::hal::cpu::SecondaryBootParameters>());
        let required_size = published_size
            .checked_add(placement_alignment - 1)
            .ok_or(Error::InvalidHandoffLayout)?;
        let page_size = usize::try_from(PAGE_SIZE).map_err(|_| Error::InvalidHandoffLayout)?;
        let pages = required_size.div_ceil(page_size);
        let allocated_pages = pages
            .checked_next_power_of_two()
            .ok_or(Error::InvalidHandoffLayout)?;
        let order = usize::try_from(allocated_pages.trailing_zeros())
            .map_err(|_| Error::InvalidHandoffLayout)?;
        let block = PageBlock::allocate(order).map_err(|_| Error::Allocation)?;
        let physical_start = block.physical().get();
        let block_start = super::super::mm::memory::linear_address(physical_start)
            .ok_or(Error::InvalidAddress)?;
        let physical_alignment =
            u64::try_from(placement_alignment).map_err(|_| Error::InvalidHandoffLayout)?;
        let context =
            align_up_u64(physical_start, physical_alignment).ok_or(Error::InvalidHandoffLayout)?;
        let offset = context
            .checked_sub(physical_start)
            .and_then(|offset| usize::try_from(offset).ok())
            .ok_or(Error::InvalidHandoffLayout)?;
        let parameters_address = block_start
            .checked_add(offset)
            .ok_or(Error::InvalidHandoffLayout)?;
        // The trampoline consumes the physical record while Rust initializes
        // it through the linear alias. Both addresses must satisfy the record
        // alignment and cache-line boundary.
        if !parameters_address.is_multiple_of(placement_alignment) {
            return Err(Error::InvalidHandoffLayout);
        }
        let block_size = page_size
            .checked_mul(allocated_pages)
            .ok_or(Error::InvalidHandoffLayout)?;
        let block_end = block_start
            .checked_add(block_size)
            .ok_or(Error::InvalidHandoffLayout)?;
        let published_end = parameters_address
            .checked_add(published_size)
            .ok_or(Error::InvalidHandoffLayout)?;
        if published_end > block_end {
            return Err(Error::InvalidHandoffLayout);
        }
        let pointer: NonNull<crate::hal::cpu::SecondaryBootParameters> =
            NonNull::new(core::ptr::with_exposed_provenance_mut(parameters_address))
                .ok_or(Error::InvalidHandoffLayout)?;
        // SAFETY: The dedicated buddy block is uniquely owned, mapped writable,
        // and the selected interior address satisfies the record alignment.
        unsafe { pointer.as_ptr().write(parameters) };
        let handoff = Self {
            parameters: pointer,
            context,
            _block: block,
        };
        // SAFETY: `handoff` uniquely owns every complete cache line intersecting
        // this record. No secondary can observe or modify it before CPU_ON.
        unsafe {
            crate::hal::cache::publish_data_range(parameters_address, parameter_size)?;
        }
        Ok(handoff)
    }

    const fn context(&self) -> u64 {
        self.context
    }
}

impl Drop for SecondaryBootHandoff {
    fn drop(&mut self) {
        // SAFETY: This owner initialized the record exactly once and no
        // secondary can still access it on the normal all-online release path.
        unsafe { self.parameters.as_ptr().drop_in_place() };
    }
}

/// Owns all handoffs until successful secondary admission is complete.
///
/// Once firmware may have accepted a `CPU_ON` request, an error cannot prove a
/// late secondary will never consume its context. Such a fail-stop path keeps
/// every record permanently rather than returning potentially live pages to
/// the allocator. Only [`Self::release`] performs normal cleanup.
struct SecondaryBootHandoffs {
    records: Vec<SecondaryBootHandoff>,
    observable: bool,
}

impl SecondaryBootHandoffs {
    fn with_capacity(capacity: usize) -> Result<Self, Error> {
        let mut records = Vec::new();
        records
            .try_reserve(capacity)
            .map_err(|_| Error::Allocation)?;
        Ok(Self {
            records,
            observable: false,
        })
    }

    fn push(&mut self, handoff: SecondaryBootHandoff) {
        self.records.push(handoff);
    }

    fn mark_observable(&mut self) {
        self.observable = true;
    }

    fn release(mut self) {
        self.observable = false;
    }
}

impl Drop for SecondaryBootHandoffs {
    fn drop(&mut self) {
        if self.observable {
            // A late secondary may still dereference any submitted context.
            // Leak only on a fatal/failed admission path; reusing these pages
            // would be an unsafe use-after-free.
            let records = core::mem::take(&mut self.records);
            core::mem::forget(records);
        }
    }
}

fn align_up(value: usize, alignment: usize) -> Option<usize> {
    value
        .checked_add(alignment - 1)
        .map(|rounded| rounded & !(alignment - 1))
}

fn align_up_u64(value: u64, alignment: u64) -> Option<u64> {
    value
        .checked_add(alignment - 1)
        .map(|rounded| rounded & !(alignment - 1))
}

/// Returns the number of CPUs that completed their local kernel setup.
pub(crate) fn online_cpu_count() -> usize {
    ONLINE
        .iter()
        .filter(|online| online.load(Ordering::Acquire))
        .count()
}

/// Returns the immutable CPU count whose local kernel state reached online.
pub(crate) fn participating_cpu_count() -> Option<usize> {
    let count = PARTICIPATING_CPU_COUNT.load(Ordering::Acquire);
    (count != 0).then_some(count)
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
    if PARTICIPATING_CPU_COUNT.load(Ordering::Acquire) != 0 {
        return Err(Error::AlreadyInitialized);
    }
    let boot_hardware_id = crate::hal::cpu::current_hardware_id();
    match platform
        .cpus
        .as_slice()
        .iter()
        .filter(|cpu| cpu.hardware_id == boot_hardware_id.get())
        .count()
    {
        0 => return Err(Error::BootCpuMissing),
        1 => {}
        _ => return Err(Error::InvalidTopology),
    }
    let admitted_cpu_count = platform.cpus.len();
    if admitted_cpu_count == 0
        || admitted_cpu_count > hyper::cpu::MAX_CPUS
        || CpuIndex::new(admitted_cpu_count - 1).is_none()
    {
        return Err(Error::CpuIndexOverflow);
    }
    let mut registration_index = 1usize;
    for cpu in platform
        .cpus
        .as_slice()
        .iter()
        .filter(|cpu| cpu.hardware_id != boot_hardware_id.get())
    {
        let cpu_index = CpuIndex::new(registration_index).ok_or(Error::CpuIndexOverflow)?;
        registration_index += 1;
        if !crate::hal::cpu::register_secondary(cpu_index, CpuHardwareId::new(cpu.hardware_id)) {
            return Err(Error::InvalidCpuRoute(cpu_index.get()));
        }
    }
    if registration_index != admitted_cpu_count {
        return Err(Error::InvalidTopology);
    }
    ONLINE[CpuIndex::BOOT].store(true, Ordering::Release);
    let entry = crate::hal::cpu::secondary_entry_address(image_physical_start, kernel_virtual_base)
        .ok_or(Error::InvalidAddress)?;
    let mut next_cpu_index = 1usize;
    let mut boot_parameters =
        SecondaryBootHandoffs::with_capacity(platform.cpus.len().saturating_sub(1))?;

    for cpu in platform
        .cpus
        .as_slice()
        .iter()
        .filter(|cpu| cpu.hardware_id != boot_hardware_id.get())
    {
        let cpu_index = CpuIndex::new(next_cpu_index).ok_or(Error::CpuIndexOverflow)?;
        next_cpu_index += 1;
        super::super::mm::stack::prepare_cpu(cpu_index)?;
        let stack = super::super::task::scheduler::register_secondary_cpu(cpu_index, "idle")?;
        let parameters = SecondaryBootHandoff::new(crate::hal::cpu::SecondaryBootParameters::new(
            root,
            stack.physical_top,
            stack.virtual_top as u64,
            cpu_index.get(),
        ))?;
        let context = parameters.context();
        boot_parameters.push(parameters);
        // From this point firmware may retain `context` even if CPU_ON reports
        // failure or a later secondary times out. Arm fail-stop retention
        // before making the context externally observable.
        boot_parameters.mark_observable();

        // SAFETY: `entry` is the architecture-provided secondary trampoline.
        // The dedicated, cache-published parameter record remains pinned in
        // `boot_parameters` until every secondary reports online.
        unsafe {
            super::super::device::cpu_power::cpu_on(
                CpuHardwareId::new(cpu.hardware_id),
                entry,
                context,
            )?
        };
    }

    for (cpu_index, online) in ONLINE.iter().enumerate().take(next_cpu_index).skip(1) {
        if !super::super::time::spin_wait_until(CPU_ON_TIMEOUT_NS, || {
            online.load(Ordering::Acquire)
        })? {
            return Err(Error::CpuDidNotStart(cpu_index));
        }
    }
    boot_parameters.release();

    // All admitted CPUs completed local interrupt, timer, and scheduler setup.
    // Publish the immutable participation count only after their per-CPU ONLINE
    // stores were observed above; timer diagnostics use this Release/Acquire
    // edge to distinguish the completed topology from the earlier boot plan.
    PARTICIPATING_CPU_COUNT
        .compare_exchange(0, next_cpu_index, Ordering::Release, Ordering::Relaxed)
        .map_err(|_| Error::AlreadyInitialized)?;

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
            crate::hal::cpu::halt()
        }
    }
}

fn try_secondary_entry(cpu_index: usize) -> Result<Infallible, Error> {
    let cpu_index = CpuIndex::new(cpu_index).ok_or(Error::InvalidCpuIndex)?;
    if super::current_index() != Some(cpu_index) {
        return Err(Error::InvalidCpuIndex);
    }
    super::super::irq::interrupt::initialize_local_cpu().map_err(Error::LocalInterrupt)?;
    super::super::time::initialize_local_cpu().map_err(Error::LocalTimer)?;
    let stack = super::super::task::scheduler::install_current_idle()?;
    // SAFETY: This CPU still has local interrupts masked, `stack` belongs to
    // its current idle Thread, and the continuation never returns.
    unsafe { super::super::mm::stack::reset_and_enter(stack, enter_clean_idle, cpu_index.get()) }
}

extern "C" fn enter_clean_idle(cpu_index: usize) -> ! {
    let Some(cpu_index) = CpuIndex::new(cpu_index) else {
        crate::hal::cpu::halt()
    };
    if super::current_index() != Some(cpu_index) {
        crate::hal::cpu::halt()
    }
    crate::hal::cpu::mark_current_online();
    // Architecture entry consumed every handoff field before branching into
    // Rust. Release publication therefore permits the boot CPU's Acquire wait
    // to reclaim the dedicated record only after all such reads completed.
    ONLINE[cpu_index].store(true, Ordering::Release);
    crate::hal::cpu::send_event();
    crate::println!(
        "HypeR: CPU {} online, hardware ID {:#x}; entering idle",
        cpu_index.get(),
        crate::hal::cpu::current_hardware_id().get()
    );
    crate::hal::irq::enable_local();
    super::super::task::scheduler::run_idle_loop()
}
