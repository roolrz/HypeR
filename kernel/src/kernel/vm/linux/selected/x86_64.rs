// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Linux x86 boot-protocol image loading and initial long-mode state.

use super::super::Error;
use super::super::abi::{LinuxAbi, PayloadLoadError, PayloadMemory, PayloadRange};
use hyper::vm::{bundle::VmBundle, exit::GuestPhysicalAddress};

pub(crate) const LINUX_GUEST_RAM_IPA: u64 = 0;
pub(crate) const LINUX_GUEST_KERNEL_IPA: u64 = 0x0100_0000;
pub(crate) const LINUX_GUEST_TIMER_INTERRUPT: u32 = 0xef;

const BOOT_PARAMS_IPA: u64 = 0x0001_0000;
const COMMAND_LINE_IPA: u64 = 0x0002_0000;
const GDT_IPA: u64 = 0x0005_0000;
const TSS_IPA: u64 = 0x0005_1000;
const GUEST_STACK_TOP: u64 = 0x0006_f000;
const PML4_IPA: u64 = 0x0007_0000;
const PDPT_IPA: u64 = 0x0007_1000;
const PAGE_DIRECTORY_IPA: u64 = 0x0007_2000;

pub(crate) const fn linux_guest_architecture() -> &'static str {
    "x86_64"
}

pub(crate) const fn linux_abi() -> LinuxAbi {
    LinuxAbi::new(
        linux_guest_architecture(),
        LINUX_GUEST_RAM_IPA,
        LINUX_GUEST_KERNEL_IPA,
        LINUX_GUEST_TIMER_INTERRUPT,
    )
}

pub(crate) fn validate_linux_host() -> Result<(), Error> {
    if !crate::hal::vm::guest_execution_available() {
        return Err(Error::VirtualizationUnavailable);
    }
    Ok(())
}

pub(crate) fn describe_linux_host(mut emit: impl FnMut(core::fmt::Arguments<'_>)) {
    emit(format_args!(
        "HypeR: {} guest-execution backend selected",
        crate::hal::vm::virtualization_backend_name()
    ));
}

pub(crate) fn validate_linux_kernel(image: &[u8]) -> Result<(), Error> {
    let alignment = read_u32(image, 0x230).ok_or(Error::InvalidKernel)?;
    let preferred = read_u64(image, 0x258).ok_or(Error::InvalidKernel)?;
    let xloadflags = read_u16(image, 0x236).ok_or(Error::InvalidKernel)?;
    if image.len() < 0x264
        || image.get(0x202..0x206) != Some(b"HdrS")
        || image[0x234] == 0
        || xloadflags & 1 == 0
        || alignment == 0
        || !alignment.is_power_of_two()
        || !LINUX_GUEST_KERNEL_IPA.is_multiple_of(u64::from(alignment))
        || preferred != LINUX_GUEST_KERNEL_IPA
    {
        return Err(Error::InvalidKernel);
    }
    Ok(())
}

pub(crate) fn linux_kernel_occupied_size(image: &[u8]) -> Result<u64, Error> {
    let payload_size = payload(image)?.len() as u64;
    let initialized_size = u64::from(read_u32(image, 0x260).ok_or(Error::InvalidKernel)?);
    Ok(payload_size.max(initialized_size))
}

pub(crate) fn load_linux_payload<Memory: PayloadMemory>(
    guest: &VmBundle<'_>,
    address_space: &mut Memory,
    initramfs_range: Option<PayloadRange>,
) -> Result<(), PayloadLoadError<Memory::Error>> {
    const BOOT_PARAMS_SIZE: usize = 4096;
    const E820_RAM: u32 = 1;

    let image = guest.kernel();
    let kernel_payload = payload(image)?;
    let mut boot_params = [0_u8; BOOT_PARAMS_SIZE];
    let header_bytes = image.len().min(0x290).min(boot_params.len());
    boot_params[..header_bytes].copy_from_slice(&image[..header_bytes]);

    boot_params[0x210] = 0xff;
    boot_params[0x211] |= 0x80;
    write_u32(&mut boot_params, 0x214, LINUX_GUEST_KERNEL_IPA as u32)?;
    write_u32(&mut boot_params, 0x228, COMMAND_LINE_IPA as u32)?;
    if let Some(range) = initramfs_range {
        write_u32(
            &mut boot_params,
            0x218,
            u32::try_from(range.start().get()).map_err(|_| Error::InvalidLayout)?,
        )?;
        write_u32(
            &mut boot_params,
            0x21c,
            u32::try_from(range.length()).map_err(|_| Error::InvalidLayout)?,
        )?;
    }
    boot_params[0x1e8] = 2;
    write_e820_entry(&mut boot_params, 0, 0, 0x0009_f000, E820_RAM)?;
    write_e820_entry(
        &mut boot_params,
        1,
        0x0010_0000,
        guest.memory_size().saturating_sub(0x0010_0000),
        E820_RAM,
    )?;

    let command_line = guest.command_line().as_bytes();
    let command_line_limit = read_u32(image, 0x238)
        .filter(|limit| *limit != 0)
        .unwrap_or(2048) as usize;
    if command_line.len() + 1 > command_line_limit {
        return Err(Error::InvalidLayout.into());
    }
    address_space
        .copy_to(gpa(LINUX_GUEST_KERNEL_IPA), kernel_payload)
        .map_err(PayloadLoadError::Memory)?;
    address_space
        .copy_to(gpa(BOOT_PARAMS_IPA), &boot_params)
        .map_err(PayloadLoadError::Memory)?;
    address_space
        .copy_to(gpa(COMMAND_LINE_IPA), command_line)
        .map_err(PayloadLoadError::Memory)?;
    address_space
        .copy_to(gpa(COMMAND_LINE_IPA + command_line.len() as u64), &[0])
        .map_err(PayloadLoadError::Memory)?;
    write_boot_tables(address_space)?;
    if let (Some(bytes), Some(range)) = (guest.initramfs(), initramfs_range) {
        address_space
            .copy_to(range.start(), bytes)
            .map_err(PayloadLoadError::Memory)?;
        address_space
            .publish_data(range.start(), bytes.len())
            .map_err(PayloadLoadError::Memory)?;
    }
    address_space
        .publish_instruction(gpa(LINUX_GUEST_KERNEL_IPA), kernel_payload.len())
        .map_err(PayloadLoadError::Memory)?;
    address_space
        .publish_data(gpa(BOOT_PARAMS_IPA), boot_params.len())
        .map_err(PayloadLoadError::Memory)?;
    address_space
        .publish_data(gpa(COMMAND_LINE_IPA), command_line.len() + 1)
        .map_err(PayloadLoadError::Memory)?;
    Ok(())
}

pub(crate) fn prepare_linux_vcpu_context() -> Result<crate::hal::vm::VcpuContext, Error> {
    use crate::hal::vm::InitialRegisterAssignment as Register;
    const RSP: usize = 4;
    const RSI: usize = 6;

    crate::hal::vm::prepare_initial_context(
        LINUX_GUEST_KERNEL_IPA + 0x200,
        &[
            Register::new(RSP, GUEST_STACK_TOP),
            Register::new(RSI, BOOT_PARAMS_IPA),
        ],
    )
    .map_err(|_| Error::InvalidLayout)
}

pub(crate) fn describe_linux_guest_layout(
    initramfs_range: Option<PayloadRange>,
    stage2_root: u64,
    mut emit: impl FnMut(core::fmt::Arguments<'_>),
) {
    emit(format_args!(
        "HypeR: guest IPA layout: boot params {:#x}, kernel {:#x}, initramfs {:#x}-{:#x}, stage-2 root {:#x}",
        BOOT_PARAMS_IPA,
        LINUX_GUEST_KERNEL_IPA,
        initramfs_range.map_or(0, |range| range.start().get()),
        initramfs_range.map_or(0, |range| range.end().get()),
        stage2_root
    ));
}

fn payload(image: &[u8]) -> Result<&[u8], Error> {
    let setup_sectors = image.get(0x1f1).copied().ok_or(Error::InvalidKernel)?;
    let setup_sectors = if setup_sectors == 0 { 4 } else { setup_sectors };
    let offset = (usize::from(setup_sectors) + 1)
        .checked_mul(512)
        .ok_or(Error::AddressOverflow)?;
    image.get(offset..).ok_or(Error::InvalidKernel)
}

fn write_boot_tables<Memory: PayloadMemory>(
    address_space: &mut Memory,
) -> Result<(), PayloadLoadError<Memory::Error>> {
    let mut gdt = [0_u8; 40];
    write_u64(&mut gdt, 8, 0x00af_9a00_0000_ffff)?;
    write_u64(&mut gdt, 16, 0x00cf_9200_0000_ffff)?;
    let tss_limit = 0x67_u64;
    let tss_descriptor = tss_limit
        | ((TSS_IPA & 0xffff) << 16)
        | (((TSS_IPA >> 16) & 0xff) << 32)
        | (0x89 << 40)
        | (((tss_limit >> 16) & 0xf) << 48)
        | (((TSS_IPA >> 24) & 0xff) << 56);
    write_u64(&mut gdt, 24, tss_descriptor)?;
    address_space
        .copy_to(gpa(GDT_IPA), &gdt)
        .map_err(PayloadLoadError::Memory)?;
    address_space
        .copy_to(gpa(TSS_IPA), &[0; 104])
        .map_err(PayloadLoadError::Memory)?;

    let mut pml4 = [0_u8; 4096];
    let mut pdpt = [0_u8; 4096];
    let mut directory = [0_u8; 4096];
    write_u64(&mut pml4, 0, PDPT_IPA | 3)?;
    write_u64(&mut pdpt, 0, PAGE_DIRECTORY_IPA | 3)?;
    for entry in 0..512 {
        write_u64(
            &mut directory,
            entry * 8,
            (entry as u64 * 2 * 1024 * 1024) | 0x83,
        )?;
    }
    address_space
        .copy_to(gpa(PML4_IPA), &pml4)
        .map_err(PayloadLoadError::Memory)?;
    address_space
        .copy_to(gpa(PDPT_IPA), &pdpt)
        .map_err(PayloadLoadError::Memory)?;
    address_space
        .copy_to(gpa(PAGE_DIRECTORY_IPA), &directory)
        .map_err(PayloadLoadError::Memory)?;
    Ok(())
}

fn write_e820_entry(
    boot_params: &mut [u8],
    index: usize,
    address: u64,
    size: u64,
    kind: u32,
) -> Result<(), Error> {
    let offset = 0x2d0 + index * 20;
    write_u64(boot_params, offset, address)?;
    write_u64(boot_params, offset + 8, size)?;
    write_u32(boot_params, offset + 16, kind)
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    let value: [u8; 2] = bytes.get(offset..offset + 2)?.try_into().ok()?;
    Some(u16::from_le_bytes(value))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let value: [u8; 4] = bytes.get(offset..offset + 4)?.try_into().ok()?;
    Some(u32::from_le_bytes(value))
}

fn read_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    let value: [u8; 8] = bytes.get(offset..offset + 8)?.try_into().ok()?;
    Some(u64::from_le_bytes(value))
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) -> Result<(), Error> {
    let destination = bytes
        .get_mut(offset..offset + 4)
        .ok_or(Error::InvalidLayout)?;
    destination.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) -> Result<(), Error> {
    let destination = bytes
        .get_mut(offset..offset + 8)
        .ok_or(Error::InvalidLayout)?;
    destination.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

const fn gpa(address: u64) -> GuestPhysicalAddress {
    GuestPhysicalAddress::new(address)
}
