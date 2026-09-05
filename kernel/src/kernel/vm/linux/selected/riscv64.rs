// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! RISC-V Linux Image boot convention for virtual machines.

use super::super::Error;
use super::super::abi::{LinuxAbi, PayloadLoadError, PayloadMemory, PayloadRange};
use hyper::vm::fdt::{self, Builder};
use hyper::vm::{bundle::VmBundle, exit::GuestPhysicalAddress};

pub(crate) const LINUX_GUEST_RAM_IPA: u64 = 0x8000_0000;
pub(crate) const LINUX_GUEST_KERNEL_IPA: u64 = LINUX_GUEST_RAM_IPA + 0x0020_0000;
pub(crate) const LINUX_GUEST_TIMER_INTERRUPT: u32 = 5;

const GUEST_DTB_IPA: u64 = LINUX_GUEST_RAM_IPA + 0x0001_0000;
const IMAGE_SIZE_OFFSET: usize = 16;
const IMAGE_MAGIC_OFFSET: usize = 56;
const IMAGE_HEADER_SIZE: usize = 64;

pub(crate) const fn linux_guest_architecture() -> &'static str {
    "riscv64"
}

pub(crate) const fn linux_abi() -> LinuxAbi {
    LinuxAbi::new(
        linux_guest_architecture(),
        LINUX_GUEST_RAM_IPA,
        LINUX_GUEST_KERNEL_IPA,
        LINUX_GUEST_TIMER_INTERRUPT,
    )
}

pub(crate) const fn validate_linux_host() -> Result<(), Error> {
    Ok(())
}

pub(crate) fn describe_linux_host(_emit: impl FnMut(core::fmt::Arguments<'_>)) {}

pub(crate) fn validate_linux_kernel(image: &[u8]) -> Result<(), Error> {
    if image.len() < IMAGE_HEADER_SIZE
        || image.get(IMAGE_MAGIC_OFFSET..IMAGE_MAGIC_OFFSET + 4) != Some(&[0x52, 0x53, 0x43, 0x05])
    {
        return Err(Error::InvalidKernel);
    }
    Ok(())
}

pub(crate) fn linux_kernel_occupied_size(image: &[u8]) -> Result<u64, Error> {
    let declared = read_u64(image, IMAGE_SIZE_OFFSET).ok_or(Error::InvalidKernel)?;
    Ok((image.len() as u64).max(declared))
}

pub(crate) fn load_linux_payload<Memory: PayloadMemory>(
    guest: &VmBundle<'_>,
    address_space: &mut Memory,
    initramfs_range: Option<PayloadRange>,
) -> Result<(), PayloadLoadError<Memory::Error>> {
    let image = guest.kernel();
    let initramfs = guest.initramfs();
    let device_tree = build_device_tree(
        LINUX_GUEST_RAM_IPA,
        guest.memory_size(),
        initramfs_range,
        guest.command_line(),
        guest.vcpu_count(),
    )?;
    if GUEST_DTB_IPA + device_tree.len() as u64 > LINUX_GUEST_KERNEL_IPA {
        return Err(Error::InvalidLayout.into());
    }

    address_space
        .copy_to(gpa(LINUX_GUEST_KERNEL_IPA), image)
        .map_err(PayloadLoadError::Memory)?;
    if let (Some(bytes), Some(range)) = (initramfs, initramfs_range) {
        address_space
            .copy_to(range.start(), bytes)
            .map_err(PayloadLoadError::Memory)?;
        address_space
            .publish_data(range.start(), bytes.len())
            .map_err(PayloadLoadError::Memory)?;
    }
    address_space
        .copy_to(gpa(GUEST_DTB_IPA), &device_tree)
        .map_err(PayloadLoadError::Memory)?;
    address_space
        .publish_instruction(gpa(LINUX_GUEST_KERNEL_IPA), image.len())
        .map_err(PayloadLoadError::Memory)?;
    address_space
        .publish_data(gpa(GUEST_DTB_IPA), device_tree.len())
        .map_err(PayloadLoadError::Memory)?;
    Ok(())
}

pub(crate) fn prepare_linux_vcpu_context() -> Result<crate::hal::vm::VcpuContext, Error> {
    use crate::hal::vm::InitialRegisterAssignment as Register;
    const A0: usize = 10;
    const A1: usize = 11;

    crate::hal::vm::prepare_initial_context(
        LINUX_GUEST_KERNEL_IPA,
        &[Register::new(A0, 0), Register::new(A1, GUEST_DTB_IPA)],
    )
    .map_err(|_| Error::InvalidLayout)
}

pub(crate) fn describe_linux_guest_layout(
    initramfs_range: Option<PayloadRange>,
    stage2_root: u64,
    mut emit: impl FnMut(core::fmt::Arguments<'_>),
) {
    emit(format_args!(
        "HypeR: guest IPA layout: DTB {:#x}, Image {:#x}, initramfs {:#x}-{:#x}, stage-2 root {:#x}",
        GUEST_DTB_IPA,
        LINUX_GUEST_KERNEL_IPA,
        initramfs_range.map_or(0, |range| range.start().get()),
        initramfs_range.map_or(0, |range| range.end().get()),
        stage2_root
    ));
}

fn build_device_tree(
    memory_base: u64,
    memory_size: u64,
    initramfs: Option<PayloadRange>,
    command_line: &str,
    vcpu_count: u32,
) -> Result<alloc::vec::Vec<u8>, fdt::Error> {
    const CPU_INTC_PHANDLE: u32 = 0x10;

    let mut builder = Builder::new();
    builder.begin_node("")?;
    builder.property_u32("#address-cells", 2)?;
    builder.property_u32("#size-cells", 2)?;
    builder.property_string("compatible", "hyper,riscv-virtual-machine")?;
    builder.property_string("model", "HypeR RISC-V virtual machine")?;

    builder.begin_node("chosen")?;
    builder.property_string("bootargs", command_line)?;
    if let Some(range) = initramfs {
        builder.property_u64_cells("linux,initrd-start", range.start().get())?;
        builder.property_u64_cells("linux,initrd-end", range.end().get())?;
    }
    builder.end_node()?;

    fdt::begin_hex_node(&mut builder, "memory@", memory_base)?;
    builder.property_string("device_type", "memory")?;
    builder.property_u64_pair("reg", memory_base, memory_size)?;
    builder.end_node()?;

    builder.begin_node("cpus")?;
    builder.property_u32("#address-cells", 1)?;
    builder.property_u32("#size-cells", 0)?;
    builder.property_u32("timebase-frequency", 10_000_000)?;
    for index in 0..vcpu_count {
        fdt::begin_hex_node(&mut builder, "cpu@", u64::from(index))?;
        builder.property_string("device_type", "cpu")?;
        builder.property_string("compatible", "riscv")?;
        builder.property_string("status", "okay")?;
        builder.property_u32("reg", index)?;
        builder.property_string("riscv,isa", "rv64imafdc")?;
        builder.property_string("riscv,isa-base", "rv64i")?;
        builder.property_string_list(
            "riscv,isa-extensions",
            &["i", "m", "a", "f", "d", "c", "zicsr", "zifencei"],
        )?;
        builder.property_string("mmu-type", "riscv,sv39")?;
        builder.begin_node("interrupt-controller")?;
        builder.property_empty("interrupt-controller")?;
        builder.property_u32("#interrupt-cells", 1)?;
        builder.property_string("compatible", "riscv,cpu-intc")?;
        builder.property_u32("phandle", CPU_INTC_PHANDLE + index)?;
        builder.end_node()?;
        builder.end_node()?;
    }
    builder.end_node()?;

    builder.begin_node("sbi")?;
    builder.property_string("compatible", "riscv,sbi")?;
    builder.end_node()?;

    builder.end_node()?;
    builder.finish()
}

fn read_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    let value: [u8; 8] = bytes.get(offset..offset.checked_add(8)?)?.try_into().ok()?;
    Some(u64::from_le_bytes(value))
}

const fn gpa(address: u64) -> GuestPhysicalAddress {
    GuestPhysicalAddress::new(address)
}
