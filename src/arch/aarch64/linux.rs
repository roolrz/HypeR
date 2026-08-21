// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! `AArch64` Linux Image boot convention for virtual machines.

use crate::arch::guest::{Error, PayloadLoadError, PayloadMemory, PayloadRange};
use hyper::vm::fdt::{self, Builder};
use hyper::vm::{bundle::VmBundle, exit::GuestPhysicalAddress};

pub(crate) const LINUX_GUEST_RAM_IPA: u64 = 0x4000_0000;
pub(crate) const LINUX_GUEST_KERNEL_IPA: u64 = LINUX_GUEST_RAM_IPA + 0x0020_0000;
pub(crate) const LINUX_GUEST_TIMER_INTERRUPT: u32 = 27;

const GUEST_DTB_IPA: u64 = LINUX_GUEST_RAM_IPA + 0x0001_0000;
const IMAGE_SIZE_OFFSET: usize = 16;
const IMAGE_MAGIC_OFFSET: usize = 56;
const IMAGE_HEADER_SIZE: usize = 64;
const GIC_PHANDLE: u32 = 1;
const UART_CLOCK_PHANDLE: u32 = 2;
const APB_CLOCK_PHANDLE: u32 = 3;

pub(crate) const fn linux_guest_architecture() -> &'static str {
    "aarch64"
}

pub(crate) const fn validate_linux_host() -> Result<(), Error> {
    Ok(())
}

pub(crate) fn describe_linux_host(_emit: impl FnMut(core::fmt::Arguments<'_>)) {}

pub(crate) fn validate_linux_kernel(image: &[u8]) -> Result<(), Error> {
    if image.len() < IMAGE_HEADER_SIZE
        || image.get(IMAGE_MAGIC_OFFSET..IMAGE_MAGIC_OFFSET + 4) != Some(b"ARMd")
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

pub(crate) fn prepare_linux_vcpu_context() -> super::VcpuContext {
    let mut context = super::VcpuContext::new(LINUX_GUEST_KERNEL_IPA);
    context.general[0] = GUEST_DTB_IPA;
    context.general[1] = 0;
    context.general[2] = 0;
    context.general[3] = 0;
    context
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
    let mut builder = Builder::new();
    builder.begin_node("")?;
    builder.property_u32("#address-cells", 2)?;
    builder.property_u32("#size-cells", 2)?;
    builder.property_u32("interrupt-parent", GIC_PHANDLE)?;
    builder.property_string("compatible", "hyper,virtual-machine")?;
    builder.property_string("model", "HypeR AArch64 virtual machine")?;

    builder.begin_node("chosen")?;
    builder.property_string("bootargs", command_line)?;
    builder.property_string("stdout-path", "/pl011@9000000")?;
    if let Some(range) = initramfs {
        builder.property_u64_cells("linux,initrd-start", range.start().get())?;
        builder.property_u64_cells("linux,initrd-end", range.end().get())?;
    }
    builder.end_node()?;

    builder.begin_node("aliases")?;
    builder.property_string("serial0", "/pl011@9000000")?;
    builder.end_node()?;

    builder.begin_node("memory@40000000")?;
    builder.property_string("device_type", "memory")?;
    builder.property_u64_pair("reg", memory_base, memory_size)?;
    builder.end_node()?;

    add_cpu_nodes(&mut builder, vcpu_count)?;
    add_interrupt_nodes(&mut builder)?;
    add_console_node(&mut builder)?;

    builder.end_node()?;
    builder.finish()
}

fn add_cpu_nodes(builder: &mut Builder, vcpu_count: u32) -> Result<(), fdt::Error> {
    builder.begin_node("cpus")?;
    builder.property_u32("#address-cells", 2)?;
    builder.property_u32("#size-cells", 0)?;
    for index in 0..vcpu_count {
        fdt::begin_hex_node(builder, "cpu@", u64::from(index))?;
        builder.property_string("device_type", "cpu")?;
        builder.property_string("compatible", "arm,armv8")?;
        builder.property_string("enable-method", "psci")?;
        builder.property_u64_cells("reg", u64::from(index))?;
        builder.end_node()?;
    }
    builder.end_node()?;

    builder.begin_node("psci")?;
    builder.property_string_list("compatible", &["arm,psci-1.0", "arm,psci-0.2"])?;
    builder.property_string("method", "hvc")?;
    builder.end_node()
}

fn add_interrupt_nodes(builder: &mut Builder) -> Result<(), fdt::Error> {
    builder.begin_node("intc@8000000")?;
    builder.property_empty("interrupt-controller")?;
    builder.property_u32("#interrupt-cells", 3)?;
    builder.property_string("compatible", "arm,gic-v3")?;
    builder.property_u32("phandle", GIC_PHANDLE)?;
    builder.property_cells(
        "reg",
        &hyper::vm::aarch64::device::gicv3::REFERENCE_REG_CELLS,
    )?;
    builder.end_node()?;

    builder.begin_node("timer")?;
    builder.property_string("compatible", "arm,armv8-timer")?;
    builder.property_empty("always-on")?;
    builder.property_cells("interrupts", &[1, 13, 4, 1, 14, 4, 1, 11, 4, 1, 10, 4])?;
    builder.end_node()
}

fn add_console_node(builder: &mut Builder) -> Result<(), fdt::Error> {
    fixed_clock(builder, "clock-uart", UART_CLOCK_PHANDLE, 24_000_000)?;
    fixed_clock(builder, "clock-apb", APB_CLOCK_PHANDLE, 24_000_000)?;

    builder.begin_node("pl011@9000000")?;
    builder.property_string_list("compatible", &["arm,pl011", "arm,primecell"])?;
    builder.property_u64_pair("reg", super::GUEST_CONSOLE_BASE, super::GUEST_CONSOLE_SIZE)?;
    builder.property_cells("interrupts", &[0, super::GUEST_CONSOLE_INTERRUPT - 32, 4])?;
    builder.property_cells("clocks", &[UART_CLOCK_PHANDLE, APB_CLOCK_PHANDLE])?;
    builder.property_string_list("clock-names", &["uartclk", "apb_pclk"])?;
    builder.end_node()
}

fn fixed_clock(
    builder: &mut Builder,
    name: &str,
    phandle: u32,
    frequency: u32,
) -> Result<(), fdt::Error> {
    builder.begin_node(name)?;
    builder.property_string("compatible", "fixed-clock")?;
    builder.property_u32("#clock-cells", 0)?;
    builder.property_u32("clock-frequency", frequency)?;
    builder.property_u32("phandle", phandle)?;
    builder.end_node()
}

fn read_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    let value: [u8; 8] = bytes.get(offset..offset.checked_add(8)?)?.try_into().ok()?;
    Some(u64::from_le_bytes(value))
}

const fn gpa(address: u64) -> GuestPhysicalAddress {
    GuestPhysicalAddress::new(address)
}
