//! Guarded virtual kernel stacks and per-CPU exception-stack ownership.

use core::ptr::{read_volatile, write_bytes, write_volatile};

use hyper::mm::heap::PageOwner;
use hyper::mm::{BuddyError, PAGE_SIZE, PhysicalAddress};
use hyper::sync::InterruptSpinLock;

use super::page_block::PageBlock;

const MAX_CPUS: usize = hyper::config::MAX_CPUS as usize;
const MAX_STACKS: usize = hyper::config::MAX_KERNEL_STACKS as usize;
const WATERMARK_BYTE: u8 = 0xa5;
const WATERMARK_WORD: u64 = u64::from_ne_bytes([WATERMARK_BYTE; 8]);
const STACK_CANARY: u64 = 0x4859_5045_5253_544b;
const THREAD_STACK_BYTES: usize = hyper::config::KERNEL_STACK_SIZE_KB as usize * 1024;
const IRQ_STACK_BYTES: usize = hyper::config::IRQ_STACK_SIZE_KB as usize * 1024;
const EMERGENCY_STACK_BYTES: usize = hyper::config::EMERGENCY_STACK_SIZE_KB as usize * 1024;

const _: () = {
    assert!(MAX_STACKS > MAX_CPUS * 2);
    assert!(THREAD_STACK_BYTES.is_power_of_two());
    assert!(IRQ_STACK_BYTES.is_power_of_two());
    assert!(EMERGENCY_STACK_BYTES.is_power_of_two());
    assert!(THREAD_STACK_BYTES >= 4 * PAGE_SIZE as usize);
    assert!(IRQ_STACK_BYTES >= 4 * PAGE_SIZE as usize);
    assert!(EMERGENCY_STACK_BYTES >= 4 * PAGE_SIZE as usize);
    assert!(THREAD_STACK_BYTES <= 64 * PAGE_SIZE as usize);
    assert!(IRQ_STACK_BYTES <= 64 * PAGE_SIZE as usize);
    assert!(EMERGENCY_STACK_BYTES <= 64 * PAGE_SIZE as usize);
};

type StackLock<T> = InterruptSpinLock<T, crate::arch::LocalInterruptMask>;

static STACK_SLOTS: StackLock<StackSlots> = StackLock::new(StackSlots::new());
static CPU_STACKS: StackLock<[CpuStacks; MAX_CPUS]> =
    StackLock::new([const { CpuStacks::new() }; MAX_CPUS]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StackKind {
    Thread,
    Irq,
    Emergency,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StackStatistics {
    pub kind: StackKind,
    pub guard_page: usize,
    pub bottom: usize,
    pub top: usize,
    pub size: usize,
    pub used: usize,
    pub remaining: usize,
    pub canary_intact: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    AlreadyPrepared,
    InvalidConfiguration,
    InvalidCpuIndex,
    Mapping(super::memory::Error),
    OutOfStacks,
    Page(BuddyError),
}

impl From<BuddyError> for Error {
    fn from(error: BuddyError) -> Self {
        Self::Page(error)
    }
}

impl From<super::memory::Error> for Error {
    fn from(error: super::memory::Error) -> Self {
        Self::Mapping(error)
    }
}

struct StackSlots {
    occupied: [bool; MAX_STACKS],
}

impl StackSlots {
    const fn new() -> Self {
        Self {
            occupied: [false; MAX_STACKS],
        }
    }

    fn reserve(&mut self) -> Option<usize> {
        let slot = self.occupied.iter().position(|occupied| !*occupied)?;
        self.occupied[slot] = true;
        Some(slot)
    }

    fn release(&mut self, slot: usize) {
        if let Some(occupied) = self.occupied.get_mut(slot) {
            *occupied = false;
        }
    }
}

struct CpuStacks {
    irq: Option<KernelStack>,
    emergency: Option<KernelStack>,
}

impl CpuStacks {
    const fn new() -> Self {
        Self {
            irq: None,
            emergency: None,
        }
    }
}

/// A physically backed stack exposed only through a guarded kernel VA slot.
pub(crate) struct KernelStack {
    block: PageBlock,
    slot: usize,
    pages: usize,
    mapping: crate::arch::StackMapping,
    kind: StackKind,
}

impl KernelStack {
    pub fn allocate_thread() -> Result<Self, Error> {
        Self::allocate(StackKind::Thread, configured_pages(StackKind::Thread)?)
    }

    fn allocate(kind: StackKind, pages: usize) -> Result<Self, Error> {
        if !pages.is_power_of_two() {
            return Err(Error::InvalidConfiguration);
        }
        let slot = STACK_SLOTS
            .with(StackSlots::reserve)
            .ok_or(Error::OutOfStacks)?;
        let block = match PageBlock::allocate(pages.trailing_zeros() as usize) {
            Ok(block) => block,
            Err(error) => {
                STACK_SLOTS.with(|slots| slots.release(slot));
                return Err(error.into());
            }
        };
        let mapping = match STACK_SLOTS.with(|_| map_stack(slot, block.physical(), pages)) {
            Ok(mapping) => mapping,
            Err(error) => {
                STACK_SLOTS.with(|slots| slots.release(slot));
                return Err(error);
            }
        };
        initialize_watermark(mapping.bottom, mapping.top);
        Ok(Self {
            block,
            slot,
            pages,
            mapping,
            kind,
        })
    }

    pub const fn top(&self) -> usize {
        self.mapping.top
    }

    pub const fn physical_top(&self) -> u64 {
        self.block.physical().get() + self.pages as u64 * PAGE_SIZE
    }

    pub const fn bounds(&self) -> (usize, usize) {
        (self.mapping.bottom, self.mapping.top)
    }

    pub fn statistics(&self) -> StackStatistics {
        stack_statistics(self.kind, self.mapping)
    }
}

impl Drop for KernelStack {
    fn drop(&mut self) {
        let result = STACK_SLOTS.with(|slots| {
            let result = unmap_stack(self.slot, self.pages);
            if result.is_ok() {
                slots.release(self.slot);
            }
            result
        });
        if let Err(error) = result {
            crate::pr_crit!(
                "HypeR: failed to unmap kernel stack {}: {error:?}",
                self.slot
            );
            crate::arch::halt()
        }
    }
}

// SAFETY: KernelStack uniquely owns its physical pages and guarded VA slot.
unsafe impl Send for KernelStack {}

pub(crate) fn prepare_cpu(cpu: usize) -> Result<(), Error> {
    if cpu >= MAX_CPUS {
        return Err(Error::InvalidCpuIndex);
    }
    if CPU_STACKS.with(|stacks| stacks[cpu].irq.is_some()) {
        return Err(Error::AlreadyPrepared);
    }
    let irq = KernelStack::allocate(StackKind::Irq, configured_pages(StackKind::Irq)?)?;
    let emergency = KernelStack::allocate(
        StackKind::Emergency,
        configured_pages(StackKind::Emergency)?,
    )?;
    crate::arch::install_exception_stacks(cpu, irq.bounds(), emergency.bounds())
        .map_err(|_| Error::InvalidCpuIndex)?;
    CPU_STACKS.with(|stacks| {
        stacks[cpu].irq = Some(irq);
        stacks[cpu].emergency = Some(emergency);
    });
    Ok(())
}

pub fn cpu_stack_statistics(cpu: usize) -> Option<(StackStatistics, StackStatistics)> {
    if cpu != crate::arch::current_cpu_index() {
        return None;
    }
    CPU_STACKS.with(|stacks| {
        let stacks = stacks.get(cpu)?;
        Some((
            stacks.irq.as_ref()?.statistics(),
            stacks.emergency.as_ref()?.statistics(),
        ))
    })
}

pub fn guard_page_is_unmapped(statistics: StackStatistics) -> Result<bool, Error> {
    crate::kernel::boot::with_boot_state(|state| {
        state.memory.address_is_mapped(statistics.guard_page)
    })
    .map(|mapped| !mapped)
    .map_err(Error::from)
}

pub(crate) fn exception_stack_bounds(cpu: usize, pointer: usize) -> Option<(usize, usize)> {
    CPU_STACKS.with(|stacks| {
        let stacks = stacks.get(cpu)?;
        [&stacks.irq, &stacks.emergency]
            .into_iter()
            .flatten()
            .map(KernelStack::bounds)
            .find(|(bottom, top)| *bottom <= pointer && pointer <= *top)
    })
}

/// Discards the live call chain and restarts execution at a clean stack top.
///
/// # Safety
///
/// `bounds` must identify an exclusively owned stack that contains no live
/// values needed after the transfer. Local interrupts must be masked, and
/// `callback` must never return.
pub(crate) unsafe fn reset_and_enter(
    bounds: (usize, usize),
    callback: extern "C" fn(usize) -> !,
    argument: usize,
) -> ! {
    // SAFETY: The caller supplies an exclusive destination stack and
    // permanently abandons the current call chain before it is refilled.
    unsafe {
        crate::arch::reset_stack_and_enter(
            bounds.0,
            bounds.1,
            WATERMARK_WORD,
            STACK_CANARY,
            callback,
            argument,
        )
    }
}

fn configured_pages(kind: StackKind) -> Result<usize, Error> {
    let bytes = match kind {
        StackKind::Thread => THREAD_STACK_BYTES,
        StackKind::Irq => IRQ_STACK_BYTES,
        StackKind::Emergency => EMERGENCY_STACK_BYTES,
    };
    if !bytes.is_multiple_of(PAGE_SIZE as usize) {
        return Err(Error::InvalidConfiguration);
    }
    let pages = bytes / PAGE_SIZE as usize;
    if pages == 0 || pages > 64 || !pages.is_power_of_two() {
        return Err(Error::InvalidConfiguration);
    }
    Ok(pages)
}

fn map_stack(
    slot: usize,
    physical: PhysicalAddress,
    pages: usize,
) -> Result<crate::arch::StackMapping, Error> {
    crate::kernel::boot::with_boot_state(|state| {
        state
            .memory
            .map_kernel_stack(slot, physical, pages, &mut allocate_page_table)
    })
    .map_err(Error::from)
}

fn unmap_stack(slot: usize, pages: usize) -> Result<(), Error> {
    crate::kernel::boot::with_boot_state(|state| state.memory.unmap_kernel_stack(slot, pages))
        .map_err(Error::from)
}

fn allocate_page_table() -> Option<PhysicalAddress> {
    super::allocator::GLOBAL_ALLOCATOR
        .allocate_pages_for(0, PageOwner::PageTable)
        .ok()
}

fn initialize_watermark(bottom: usize, top: usize) {
    let size = top - bottom;
    // SAFETY: The mapping is a new, exclusive writable stack allocation.
    unsafe {
        write_bytes(bottom as *mut u8, WATERMARK_BYTE, size);
        write_volatile(bottom as *mut u64, STACK_CANARY);
    }
}

fn stack_statistics(kind: StackKind, mapping: crate::arch::StackMapping) -> StackStatistics {
    // SAFETY: Stack mappings remain live while their owning KernelStack exists.
    let canary = unsafe { read_volatile(mapping.bottom as *const u64) };
    let watermark = mapping.bottom + core::mem::size_of::<u64>();
    let mut cursor = watermark;
    while cursor < mapping.top {
        // SAFETY: cursor stays inside the live stack mapping.
        if unsafe { read_volatile(cursor as *const u8) } != WATERMARK_BYTE {
            break;
        }
        cursor += 1;
    }
    let size = mapping.top - mapping.bottom;
    let unused = cursor - watermark;
    let used = size
        .saturating_sub(core::mem::size_of::<u64>())
        .saturating_sub(unused);
    StackStatistics {
        kind,
        guard_page: mapping.guard_page,
        bottom: mapping.bottom,
        top: mapping.top,
        size,
        used,
        remaining: unused,
        canary_intact: canary == STACK_CANARY,
    }
}
