// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Validated construction plan for the final EL2 stage-1 address space.
//!
//! This module decides which firmware ranges and kernel image segments receive
//! identity, linear, MMIO, or high-kernel aliases. It validates the complete
//! address plan before publishing `FinalAddressSpace`; table allocation and
//! descriptor mutation remain owned by the parent page-table module.

use hyper::hal::memory::KernelImageLayout;
use hyper::mm::{BootAllocator, PAGE_SIZE, PhysicalAddress, VirtualAddress};
use hyper::platform::{PhysicalRange, PlatformInfo};

use super::super::layout::{KERNEL_BASE, KERNEL_STACK_BASE, LINEAR_BASE, MMIO_BASE};
use super::descriptor::MappingFlags;
use super::{Error, KERNEL_STACK_PAGES, PageTableBuilder, align_down, align_up};
use crate::arch::aarch64::address;

#[derive(Clone, Copy, Debug)]
pub(in crate::arch::aarch64::memory) struct FinalAddressSpace {
    pub root: PhysicalAddress,
    pub stack_top: VirtualAddress,
    pub kernel_base: u64,
}

/// Builds the complete post-bootstrap EL2 address space.
///
/// # Safety
///
/// The boot allocator must allocate only writable identity-mapped RAM.
pub(in crate::arch::aarch64::memory) unsafe fn build_final_address_space(
    allocator: &mut BootAllocator,
    platform: &PlatformInfo,
    kernel: KernelImageLayout,
    kernel_base: u64,
) -> Result<FinalAddressSpace, Error> {
    validate_platform_addressability(platform)?;
    // SAFETY: The function contract restricts the boot allocator to writable
    // identity-mapped RAM.
    let stack = unsafe { allocator.allocate_zeroed_pages(KERNEL_STACK_PAGES, 1)? };
    let stack_size = KERNEL_STACK_PAGES as u64 * PAGE_SIZE;
    // SAFETY: The same allocator contract applies to every table page.
    let mut builder = unsafe { PageTableBuilder::new(allocator)? };

    // SAFETY: Platform addressability was validated and builder tables remain
    // accessible through the bootstrap identity mapping.
    unsafe { map_discovered_ram(&mut builder, platform, kernel, 0, true)? };
    // SAFETY: The permanent linear alias uses the same validated RAM ranges
    // and builder ownership contract.
    unsafe { map_discovered_ram(&mut builder, platform, kernel, LINEAR_BASE, false)? };

    for &range in platform.mmio.as_slice() {
        // SAFETY: MMIO ranges passed validation and builder-owned table pages
        // remain identity-accessible throughout construction.
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

    if kernel_base < KERNEL_BASE
        || kernel_base
            .checked_add(kernel.total_size)
            .filter(|end| *end <= KERNEL_STACK_BASE)
            .is_none()
    {
        return Err(Error::InvalidRange);
    }
    // SAFETY: Image layout and randomized destination were validated, and the
    // builder still owns every table page.
    unsafe { map_kernel_at(&mut builder, kernel, kernel_base)? };
    // SAFETY: `stack` is a fresh writable allocation and builder ownership
    // remains exclusive until activation.
    unsafe {
        builder.map_range(
            VirtualAddress::new(KERNEL_STACK_BASE + PAGE_SIZE),
            PhysicalRange::new(stack.get(), stack_size).ok_or(Error::InvalidRange)?,
            MappingFlags::NORMAL_RW,
        )?;
    }

    Ok(FinalAddressSpace {
        root: builder.root(),
        stack_top: VirtualAddress::new(KERNEL_STACK_BASE + PAGE_SIZE + stack_size),
        kernel_base,
    })
}

fn validate_platform_addressability(platform: &PlatformInfo) -> Result<(), Error> {
    let physical_limit = address::physical_address_limit();
    for range in platform.memory.as_slice() {
        if range.end() > physical_limit
            || range.end() > address::STAGE1_VA_LIMIT
            || LINEAR_BASE
                .checked_add(range.end())
                .filter(|end| *end <= KERNEL_BASE)
                .is_none()
        {
            return Err(Error::InvalidAddress);
        }
    }
    for range in platform.mmio.as_slice() {
        if range.end() > physical_limit
            || range.end() > address::STAGE1_VA_LIMIT
            || MMIO_BASE
                .checked_add(range.end())
                .filter(|end| *end <= LINEAR_BASE)
                .is_none()
        {
            return Err(Error::InvalidAddress);
        }
    }
    Ok(())
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
                // SAFETY: The caller guarantees builder accessibility; this
                // subrange excludes every firmware no-map reservation.
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
            // SAFETY: The remaining subrange is validated RAM outside all
            // no-map reservations and inherits the builder contract.
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
        // SAFETY: This validated RAM range does not overlap the kernel image
        // and the caller guarantees builder table accessibility.
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
        // SAFETY: This prefix is validated RAM before the image and inherits
        // the caller's exclusive builder-table contract.
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
        // SAFETY: The caller guarantees image layout and builder lifetime; this
        // alias intentionally preserves the kernel RX mapping.
        unsafe { map_kernel_at(builder, image, image_virtual)? };
    } else {
        // SAFETY: The same image is mapped through its non-executable linear
        // alias while builder tables remain exclusively accessible.
        unsafe { map_kernel_linear_alias(builder, image, image_virtual)? };
    }
    if image_end < memory.end() {
        // SAFETY: This suffix is validated RAM after the image and inherits
        // the caller's exclusive builder-table contract.
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
    // SAFETY: The caller validates image layout and guarantees that all
    // builder-owned page tables remain identity-mapped and exclusive.
    unsafe {
        builder.map_range(
            VirtualAddress::new(virtual_base),
            PhysicalRange::new(image.physical_start, read_only_size).ok_or(Error::InvalidRange)?,
            MappingFlags::NORMAL_RO,
        )?;
        builder.map_range(
            VirtualAddress::new(
                virtual_base
                    .checked_add(read_only_size)
                    .ok_or(Error::AddressOverflow)?,
            ),
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

    // SAFETY: The caller validates each image segment and guarantees that all
    // builder-owned page tables remain identity-mapped and exclusive.
    unsafe {
        builder.map_range(
            VirtualAddress::new(virtual_base),
            PhysicalRange::new(image.physical_start, image.text_size).ok_or(Error::InvalidRange)?,
            MappingFlags::NORMAL_RX,
        )?;
        builder.map_range(
            VirtualAddress::new(
                virtual_base
                    .checked_add(image.text_size)
                    .ok_or(Error::AddressOverflow)?,
            ),
            PhysicalRange::new(rodata_start, image.rodata_size).ok_or(Error::InvalidRange)?,
            MappingFlags::NORMAL_RO,
        )?;
        let data_virtual = virtual_base
            .checked_add(image.text_size)
            .and_then(|address| address.checked_add(image.rodata_size))
            .ok_or(Error::AddressOverflow)?;
        builder.map_range(
            VirtualAddress::new(data_virtual),
            PhysicalRange::new(data_start, data_size).ok_or(Error::InvalidRange)?,
            MappingFlags::NORMAL_RW,
        )?;
    }
    Ok(())
}
