use core::arch::asm;
use core::ptr::{read_volatile, write_volatile};

use hyper::hal::memory::KernelImageLayout;
use hyper::mm::{BootAllocator, BootAllocatorError, PAGE_SIZE, PhysicalAddress, VirtualAddress};
use hyper::platform::{PhysicalRange, PlatformInfo};

use super::super::registers;
use super::layout::{KERNEL_BASE, KERNEL_STACK_BASE, LINEAR_BASE, MMIO_BASE};

const KERNEL_STACK_PAGES: usize = 16;

const ENTRY_COUNT: usize = 512;
const TABLE_DESCRIPTOR: u64 = 0b11;
const PAGE_DESCRIPTOR: u64 = 0b11;
const ADDRESS_MASK: u64 = 0x0000_ffff_ffff_f000;
const VA_LIMIT: u64 = 1 << 48;
const PA_LIMIT: u64 = 1 << 40;
const LEVEL_SHIFTS: [u32; 4] = [39, 30, 21, 12];
const LEVEL_SIZES: [u64; 4] = [1 << 39, 1 << 30, 1 << 21, PAGE_SIZE];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    AddressOverflow,
    Allocation(BootAllocatorError),
    Conflict,
    InvalidAddress,
    InvalidRange,
}

impl From<BootAllocatorError> for Error {
    fn from(error: BootAllocatorError) -> Self {
        Self::Allocation(error)
    }
}

#[derive(Clone, Copy)]
pub struct MappingFlags {
    memory: MemoryType,
    writable: bool,
    executable: bool,
}

impl MappingFlags {
    pub const NORMAL_RW: Self = Self {
        memory: MemoryType::Normal,
        writable: true,
        executable: false,
    };

    pub const NORMAL_RO: Self = Self {
        memory: MemoryType::Normal,
        writable: false,
        executable: false,
    };

    pub const NORMAL_RX: Self = Self {
        memory: MemoryType::Normal,
        writable: false,
        executable: true,
    };

    pub const DEVICE_RW: Self = Self {
        memory: MemoryType::Device,
        writable: true,
        executable: false,
    };

    fn descriptor_bits(self) -> u64 {
        let mut bits = registers::STAGE1_DESC_ACCESS_FLAG;
        bits |= match self.memory {
            MemoryType::Normal => {
                registers::STAGE1_DESC_ATTR_NORMAL | registers::STAGE1_DESC_INNER_SHAREABLE
            }
            MemoryType::Device => registers::STAGE1_DESC_OUTER_SHAREABLE,
        };
        if !self.writable {
            bits |= 1 << 7;
        }
        if !self.executable {
            bits |= registers::STAGE1_DESC_EXECUTE_NEVER;
        }
        bits
    }
}

#[derive(Clone, Copy)]
enum MemoryType {
    Normal,
    Device,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct FinalAddressSpace {
    pub root: PhysicalAddress,
    pub stack_top: VirtualAddress,
}

pub struct PageTableBuilder<'allocator> {
    allocator: &'allocator mut BootAllocator,
    root: PhysicalAddress,
}

impl<'allocator> PageTableBuilder<'allocator> {
    /// Creates an empty 48-bit EL2 stage-1 translation hierarchy.
    ///
    /// # Safety
    ///
    /// The allocator's accessible range must be writable through the current
    /// bootstrap identity map.
    pub unsafe fn new(allocator: &'allocator mut BootAllocator) -> Result<Self, Error> {
        let root = unsafe { allocator.allocate_zeroed_pages(1, 1)? };
        Ok(Self { allocator, root })
    }

    pub fn root(&self) -> PhysicalAddress {
        self.root
    }

    /// Maps a physical interval, rounding its ends outward to page boundaries.
    ///
    /// # Safety
    ///
    /// The page-table pages allocated by this builder must remain accessible
    /// through the bootstrap identity mapping until activation completes.
    pub unsafe fn map_range(
        &mut self,
        virtual_start: VirtualAddress,
        physical: PhysicalRange,
        flags: MappingFlags,
    ) -> Result<(), Error> {
        if virtual_start.get() & (PAGE_SIZE - 1) != physical.start() & (PAGE_SIZE - 1) {
            return Err(Error::InvalidRange);
        }
        let physical_start = align_down(physical.start(), PAGE_SIZE);
        let virtual_base = virtual_start
            .get()
            .checked_sub(physical.start() - physical_start)
            .ok_or(Error::AddressOverflow)?;
        let physical_end = align_up(physical.end(), PAGE_SIZE)?;
        let mut offset = 0;
        let length = physical_end - physical_start;

        while offset < length {
            let virtual_address = virtual_base
                .checked_add(offset)
                .ok_or(Error::AddressOverflow)?;
            let physical_address = physical_start
                .checked_add(offset)
                .ok_or(Error::AddressOverflow)?;
            let remaining = length - offset;
            let level = best_mapping_level(virtual_address, physical_address, remaining);
            unsafe { self.map_leaf(virtual_address, physical_address, level, flags)? };
            offset += LEVEL_SIZES[level];
        }
        Ok(())
    }

    unsafe fn map_leaf(
        &mut self,
        virtual_address: u64,
        physical_address: u64,
        leaf_level: usize,
        flags: MappingFlags,
    ) -> Result<(), Error> {
        if virtual_address >= VA_LIMIT || physical_address >= PA_LIMIT || leaf_level == 0 {
            return Err(Error::InvalidAddress);
        }

        let mut table = self.root;
        for level in 0..leaf_level {
            let index = table_index(virtual_address, level);
            let entry = unsafe { read_entry(table, index)? };
            table = if entry & 0b11 == TABLE_DESCRIPTOR {
                PhysicalAddress::new(entry & ADDRESS_MASK)
            } else if entry == 0 {
                let child = unsafe { self.allocator.allocate_zeroed_pages(1, 1)? };
                unsafe { write_entry(table, index, child.get() | TABLE_DESCRIPTOR)? };
                child
            } else {
                return Err(Error::Conflict);
            };
        }

        let index = table_index(virtual_address, leaf_level);
        let descriptor_kind = if leaf_level == 3 {
            PAGE_DESCRIPTOR
        } else {
            registers::STAGE1_DESC_BLOCK
        };
        let descriptor =
            (physical_address & ADDRESS_MASK) | flags.descriptor_bits() | descriptor_kind;
        let existing = unsafe { read_entry(table, index)? };
        if existing != 0 && existing != descriptor {
            return Err(Error::Conflict);
        }
        unsafe { write_entry(table, index, descriptor)? };
        Ok(())
    }
}

/// Builds the complete post-bootstrap EL2 address space.
///
/// # Safety
///
/// The boot allocator must allocate only writable identity-mapped RAM.
pub(super) unsafe fn build_final_address_space(
    allocator: &mut BootAllocator,
    platform: &PlatformInfo,
    kernel: KernelImageLayout,
) -> Result<FinalAddressSpace, Error> {
    let stack = unsafe { allocator.allocate_zeroed_pages(KERNEL_STACK_PAGES, 1)? };
    let stack_size = KERNEL_STACK_PAGES as u64 * PAGE_SIZE;
    let mut builder = unsafe { PageTableBuilder::new(allocator)? };

    unsafe { map_discovered_ram(&mut builder, platform, kernel, 0, true)? };
    unsafe { map_discovered_ram(&mut builder, platform, kernel, LINEAR_BASE, false)? };

    for &range in platform.mmio.as_slice() {
        unsafe {
            builder.map_range(
                VirtualAddress::new(range.start()),
                range,
                MappingFlags::DEVICE_RW,
            )?;
            builder.map_range(
                VirtualAddress::new(
                    MMIO_BASE
                        .checked_add(range.start())
                        .ok_or(Error::AddressOverflow)?,
                ),
                range,
                MappingFlags::DEVICE_RW,
            )?;
        }
    }

    unsafe { map_kernel_at(&mut builder, kernel, KERNEL_BASE)? };
    unsafe {
        builder.map_range(
            VirtualAddress::new(KERNEL_STACK_BASE),
            PhysicalRange::new(stack.get(), stack_size).ok_or(Error::InvalidRange)?,
            MappingFlags::NORMAL_RW,
        )?;
    }

    Ok(FinalAddressSpace {
        root: builder.root(),
        stack_top: VirtualAddress::new(KERNEL_STACK_BASE + stack_size),
    })
}

/// Removes low transition aliases after execution, data, devices, and the stack
/// have moved to their permanent virtual addresses.
pub(super) fn retire_identity_mappings(
    root: PhysicalAddress,
    platform: &PlatformInfo,
) -> Result<(), Error> {
    for &range in platform.memory.as_slice() {
        unmap_identity_range(root, range)?;
    }
    for &range in platform.mmio.as_slice() {
        unmap_identity_range(root, range)?;
    }

    // SAFETY: Page-table writes above are complete, and this code executes from
    // the high kernel alias without using low virtual addresses.
    unsafe {
        asm!(
            "dsb ishst",
            "tlbi alle2is",
            "dsb ish",
            "isb",
            options(nostack, preserves_flags)
        );
    }
    Ok(())
}

fn unmap_identity_range(root: PhysicalAddress, range: PhysicalRange) -> Result<(), Error> {
    let mut address = align_down(range.start(), PAGE_SIZE);
    let end = align_up(range.end(), PAGE_SIZE)?;
    while address < end {
        let mut table = root;
        let mut advanced = false;
        for (level, &level_size) in LEVEL_SIZES.iter().enumerate() {
            let index = table_index(address, level);
            let entry = read_runtime_entry(table, index)?;
            if entry == 0 {
                address = address
                    .checked_add(PAGE_SIZE)
                    .ok_or(Error::AddressOverflow)?;
                advanced = true;
                break;
            }
            let descriptor_kind = entry & 0b11;
            let is_leaf = (level < 3 && descriptor_kind == registers::STAGE1_DESC_BLOCK)
                || (level == 3 && descriptor_kind == PAGE_DESCRIPTOR);
            if is_leaf {
                write_runtime_entry(table, index, 0)?;
                address = address
                    .checked_add(level_size)
                    .ok_or(Error::AddressOverflow)?;
                advanced = true;
                break;
            }
            if descriptor_kind != TABLE_DESCRIPTOR {
                return Err(Error::Conflict);
            }
            table = PhysicalAddress::new(entry & ADDRESS_MASK);
        }
        if !advanced {
            return Err(Error::Conflict);
        }
    }
    Ok(())
}

fn read_runtime_entry(table: PhysicalAddress, index: usize) -> Result<u64, Error> {
    let base = runtime_table_address(table)? as *const u64;
    // SAFETY: Final page-table pages remain mapped through the RAM linear map.
    Ok(unsafe { read_volatile(base.add(index)) })
}

fn write_runtime_entry(table: PhysicalAddress, index: usize, value: u64) -> Result<(), Error> {
    let base = runtime_table_address(table)? as *mut u64;
    // SAFETY: The active architecture owns these page tables exclusively.
    unsafe { write_volatile(base.add(index), value) };
    Ok(())
}

fn runtime_table_address(table: PhysicalAddress) -> Result<usize, Error> {
    LINEAR_BASE
        .checked_add(table.get())
        .filter(|address| *address < VA_LIMIT)
        .and_then(|address| usize::try_from(address).ok())
        .ok_or(Error::InvalidAddress)
}

unsafe fn map_discovered_ram(
    builder: &mut PageTableBuilder<'_>,
    platform: &PlatformInfo,
    kernel: KernelImageLayout,
    alias_base: u64,
    executable_kernel: bool,
) -> Result<(), Error> {
    for &memory in platform.memory.as_slice() {
        let mut cursor = memory.start();
        for &excluded in platform.no_map.as_slice() {
            let excluded_start = align_down(excluded.start(), PAGE_SIZE);
            let excluded_end = align_up(excluded.end(), PAGE_SIZE)?;
            if excluded_end <= cursor {
                continue;
            }
            if excluded_start >= memory.end() {
                break;
            }
            if cursor < excluded_start {
                let end = excluded_start.min(memory.end());
                let range = PhysicalRange::new(cursor, end - cursor).ok_or(Error::InvalidRange)?;
                unsafe { map_ram_alias(builder, range, kernel, alias_base, executable_kernel)? };
            }
            cursor = cursor.max(excluded_end);
            if cursor >= memory.end() {
                break;
            }
        }
        if cursor < memory.end() {
            let range =
                PhysicalRange::new(cursor, memory.end() - cursor).ok_or(Error::InvalidRange)?;
            unsafe { map_ram_alias(builder, range, kernel, alias_base, executable_kernel)? };
        }
    }
    Ok(())
}

unsafe fn map_ram_alias(
    builder: &mut PageTableBuilder<'_>,
    memory: PhysicalRange,
    image: KernelImageLayout,
    alias_base: u64,
    executable_kernel: bool,
) -> Result<(), Error> {
    let image_end = image
        .physical_start
        .checked_add(image.total_size)
        .ok_or(Error::AddressOverflow)?;
    let overlap_start = memory.start().max(image.physical_start);
    let overlap_end = memory.end().min(image_end);
    if overlap_start >= overlap_end {
        return unsafe {
            builder.map_range(
                VirtualAddress::new(
                    alias_base
                        .checked_add(memory.start())
                        .ok_or(Error::AddressOverflow)?,
                ),
                memory,
                MappingFlags::NORMAL_RW,
            )
        };
    }
    if overlap_start != image.physical_start || overlap_end != image_end {
        return Err(Error::InvalidRange);
    }

    if memory.start() < image.physical_start {
        unsafe {
            builder.map_range(
                VirtualAddress::new(
                    alias_base
                        .checked_add(memory.start())
                        .ok_or(Error::AddressOverflow)?,
                ),
                PhysicalRange::new(memory.start(), image.physical_start - memory.start())
                    .ok_or(Error::InvalidRange)?,
                MappingFlags::NORMAL_RW,
            )?;
        }
    }
    let image_virtual = alias_base
        .checked_add(image.physical_start)
        .ok_or(Error::AddressOverflow)?;
    if executable_kernel {
        unsafe { map_kernel_at(builder, image, image_virtual)? };
    } else {
        unsafe { map_kernel_linear_alias(builder, image, image_virtual)? };
    }
    if image_end < memory.end() {
        unsafe {
            builder.map_range(
                VirtualAddress::new(
                    alias_base
                        .checked_add(image_end)
                        .ok_or(Error::AddressOverflow)?,
                ),
                PhysicalRange::new(image_end, memory.end() - image_end)
                    .ok_or(Error::InvalidRange)?,
                MappingFlags::NORMAL_RW,
            )?;
        }
    }
    Ok(())
}

unsafe fn map_kernel_linear_alias(
    builder: &mut PageTableBuilder<'_>,
    image: KernelImageLayout,
    virtual_base: u64,
) -> Result<(), Error> {
    let read_only_size = image
        .text_size
        .checked_add(image.rodata_size)
        .ok_or(Error::AddressOverflow)?;
    let data_start = image
        .physical_start
        .checked_add(read_only_size)
        .ok_or(Error::AddressOverflow)?;
    let data_size = image
        .total_size
        .checked_sub(read_only_size)
        .ok_or(Error::InvalidRange)?;
    unsafe {
        builder.map_range(
            VirtualAddress::new(virtual_base),
            PhysicalRange::new(image.physical_start, read_only_size).ok_or(Error::InvalidRange)?,
            MappingFlags::NORMAL_RO,
        )?;
        builder.map_range(
            VirtualAddress::new(virtual_base + read_only_size),
            PhysicalRange::new(data_start, data_size).ok_or(Error::InvalidRange)?,
            MappingFlags::NORMAL_RW,
        )?;
    }
    Ok(())
}

unsafe fn map_kernel_at(
    builder: &mut PageTableBuilder<'_>,
    image: KernelImageLayout,
    virtual_base: u64,
) -> Result<(), Error> {
    let rodata_start = image
        .physical_start
        .checked_add(image.text_size)
        .ok_or(Error::AddressOverflow)?;
    let data_start = rodata_start
        .checked_add(image.rodata_size)
        .ok_or(Error::AddressOverflow)?;
    let data_size = image
        .total_size
        .checked_sub(image.text_size)
        .and_then(|size| size.checked_sub(image.rodata_size))
        .ok_or(Error::InvalidRange)?;

    unsafe {
        builder.map_range(
            VirtualAddress::new(virtual_base),
            PhysicalRange::new(image.physical_start, image.text_size).ok_or(Error::InvalidRange)?,
            MappingFlags::NORMAL_RX,
        )?;
        builder.map_range(
            VirtualAddress::new(virtual_base + image.text_size),
            PhysicalRange::new(rodata_start, image.rodata_size).ok_or(Error::InvalidRange)?,
            MappingFlags::NORMAL_RO,
        )?;
        builder.map_range(
            VirtualAddress::new(virtual_base + image.text_size + image.rodata_size),
            PhysicalRange::new(data_start, data_size).ok_or(Error::InvalidRange)?,
            MappingFlags::NORMAL_RW,
        )?;
    }
    Ok(())
}

fn best_mapping_level(virtual_address: u64, physical_address: u64, remaining: u64) -> usize {
    for (level, &size) in LEVEL_SIZES.iter().enumerate().skip(1) {
        if virtual_address.is_multiple_of(size)
            && physical_address.is_multiple_of(size)
            && remaining >= size
        {
            return level;
        }
    }
    3
}

fn table_index(address: u64, level: usize) -> usize {
    ((address >> LEVEL_SHIFTS[level]) & (ENTRY_COUNT as u64 - 1)) as usize
}

unsafe fn read_entry(table: PhysicalAddress, index: usize) -> Result<u64, Error> {
    let base = table.as_usize().ok_or(Error::InvalidAddress)? as *const u64;
    // SAFETY: The builder owns a zeroed, identity-mapped table page.
    Ok(unsafe { read_volatile(base.add(index)) })
}

unsafe fn write_entry(table: PhysicalAddress, index: usize, value: u64) -> Result<(), Error> {
    let base = table.as_usize().ok_or(Error::InvalidAddress)? as *mut u64;
    // SAFETY: The builder owns a writable, identity-mapped table page.
    unsafe { write_volatile(base.add(index), value) };
    Ok(())
}

fn align_down(value: u64, alignment: u64) -> u64 {
    value & !(alignment - 1)
}

fn align_up(value: u64, alignment: u64) -> Result<u64, Error> {
    value
        .checked_add(alignment - 1)
        .map(|rounded| align_down(rounded, alignment))
        .ok_or(Error::AddressOverflow)
}
