//! Linux x86 boot-protocol image loading and initial long-mode state.

use super::Error;
use crate::kernel::vm::VmBundle;
use crate::kernel::vm::memory::GuestAddressSpace;

pub(super) const GUEST_RAM_IPA: u64 = 0;
pub(super) const GUEST_KERNEL_IPA: u64 = 0x0100_0000;
pub(super) const GUEST_TIMER_INTERRUPT: u32 = 0xef;

const BOOT_PARAMS_IPA: u64 = 0x0001_0000;
const COMMAND_LINE_IPA: u64 = 0x0002_0000;
const GDT_IPA: u64 = 0x0005_0000;
const TSS_IPA: u64 = 0x0005_1000;
const GUEST_STACK_TOP: u64 = 0x0006_f000;
const PML4_IPA: u64 = 0x0007_0000;
const PDPT_IPA: u64 = 0x0007_1000;
const PAGE_DIRECTORY_IPA: u64 = 0x0007_2000;

pub(super) fn validate_kernel(image: &[u8]) -> Result<(), Error> {
    let alignment = read_u32(image, 0x230).ok_or(Error::InvalidKernel)?;
    let preferred = read_u64(image, 0x258).ok_or(Error::InvalidKernel)?;
    let xloadflags = read_u16(image, 0x236).ok_or(Error::InvalidKernel)?;
    if image.len() < 0x264
        || image.get(0x202..0x206) != Some(b"HdrS")
        || image[0x234] == 0
        || xloadflags & 1 == 0
        || alignment == 0
        || !alignment.is_power_of_two()
        || !GUEST_KERNEL_IPA.is_multiple_of(u64::from(alignment))
        || preferred != GUEST_KERNEL_IPA
    {
        return Err(Error::InvalidKernel);
    }
    Ok(())
}

pub(super) fn occupied_size(image: &[u8]) -> Result<u64, Error> {
    let payload_size = payload(image)?.len() as u64;
    let initialized_size = u64::from(read_u32(image, 0x260).ok_or(Error::InvalidKernel)?);
    Ok(payload_size.max(initialized_size))
}

pub(super) fn load(
    guest: &VmBundle<'_>,
    address_space: &mut GuestAddressSpace,
    initramfs_range: Option<(u64, u64)>,
) -> Result<(), Error> {
    const BOOT_PARAMS_SIZE: usize = 4096;
    const E820_RAM: u32 = 1;

    let image = guest.kernel();
    let kernel_payload = payload(image)?;
    let mut boot_params = [0_u8; BOOT_PARAMS_SIZE];
    let header_bytes = image.len().min(0x290).min(boot_params.len());
    boot_params[..header_bytes].copy_from_slice(&image[..header_bytes]);

    boot_params[0x210] = 0xff;
    boot_params[0x211] |= 0x80;
    write_u32(&mut boot_params, 0x214, GUEST_KERNEL_IPA as u32)?;
    write_u32(&mut boot_params, 0x228, COMMAND_LINE_IPA as u32)?;
    if let Some((start, end)) = initramfs_range {
        write_u32(
            &mut boot_params,
            0x218,
            u32::try_from(start).map_err(|_| Error::InvalidLayout)?,
        )?;
        write_u32(
            &mut boot_params,
            0x21c,
            u32::try_from(end - start).map_err(|_| Error::InvalidLayout)?,
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
        return Err(Error::InvalidLayout);
    }
    address_space.write(GUEST_KERNEL_IPA, kernel_payload)?;
    address_space.write(BOOT_PARAMS_IPA, &boot_params)?;
    address_space.write(COMMAND_LINE_IPA, command_line)?;
    address_space.write(COMMAND_LINE_IPA + command_line.len() as u64, &[0])?;
    write_boot_tables(address_space)?;
    if let (Some(bytes), Some((start, _))) = (guest.initramfs(), initramfs_range) {
        address_space.write(start, bytes)?;
        address_space.publish_data(start, bytes.len())?;
    }
    address_space.publish_instruction(GUEST_KERNEL_IPA, kernel_payload.len())?;
    address_space.publish_data(BOOT_PARAMS_IPA, boot_params.len())?;
    address_space.publish_data(COMMAND_LINE_IPA, command_line.len() + 1)?;
    Ok(())
}

pub(super) fn prepare_context(context: &mut crate::arch::VcpuContext) {
    context.general[4] = GUEST_STACK_TOP;
    context.general[6] = BOOT_PARAMS_IPA;
}

pub(super) fn report_layout(initramfs_range: Option<(u64, u64)>, stage2_root: u64) {
    crate::println!(
        "HypeR: guest IPA layout: boot params {:#x}, kernel {:#x}, initramfs {:#x}-{:#x}, EPT root {:#x}",
        BOOT_PARAMS_IPA,
        GUEST_KERNEL_IPA,
        initramfs_range.map_or(0, |(start, _)| start),
        initramfs_range.map_or(0, |(_, end)| end),
        stage2_root
    );
}

fn payload(image: &[u8]) -> Result<&[u8], Error> {
    let setup_sectors = image.get(0x1f1).copied().ok_or(Error::InvalidKernel)?;
    let setup_sectors = if setup_sectors == 0 { 4 } else { setup_sectors };
    let offset = (usize::from(setup_sectors) + 1)
        .checked_mul(512)
        .ok_or(Error::AddressOverflow)?;
    image.get(offset..).ok_or(Error::InvalidKernel)
}

fn write_boot_tables(address_space: &mut GuestAddressSpace) -> Result<(), Error> {
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
    address_space.write(GDT_IPA, &gdt)?;
    address_space.write(TSS_IPA, &[0; 104])?;

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
    address_space.write(PML4_IPA, &pml4)?;
    address_space.write(PDPT_IPA, &pdpt)?;
    address_space.write(PAGE_DIRECTORY_IPA, &directory)?;
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
