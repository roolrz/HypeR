//! Initial single-vCPU Linux boot path for the QEMU virtual platform.

mod fdt;

use core::convert::Infallible;
use core::ptr::{copy_nonoverlapping, write_bytes};

use alloc::vec::Vec;
use hyper::hal::cache::CacheMaintenance;
use hyper::hal::interrupt::InterruptId;
use hyper::mm::{BuddyError, MAX_ORDER, PAGE_SIZE, PhysicalAddress};

use super::{VmBundle, VmInterruptController};
use crate::kernel::mm::page_block::PageBlock;
use crate::kernel::task::thread::{VcpuExecution, VirtualMachineId};

const GUEST_RAM_IPA: u64 = 0x4000_0000;
const GUEST_DTB_IPA: u64 = GUEST_RAM_IPA + 0x0001_0000;
const GUEST_KERNEL_IPA: u64 = GUEST_RAM_IPA + 0x0020_0000;
const GUEST_UART_IPA: u64 = 0x0900_0000;
const GUEST_UART_PHYSICAL: u64 = 0x0900_0000;
const GUEST_UART_SIZE: u64 = 0x1000;
const GUEST_TIMER_INTERRUPT: u32 = 27;
const AARCH64_IMAGE_MAGIC_OFFSET: usize = 56;
const AARCH64_IMAGE_HEADER_SIZE: usize = 64;

#[derive(Debug)]
pub enum Error {
    AddressOverflow,
    Allocation(BuddyError),
    Cache(hyper::hal::cache::CacheError),
    Context(crate::arch::VgicError),
    DeviceTree(fdt::Error),
    Interrupts(super::interrupt::Error),
    InvalidKernel,
    InvalidLayout,
    UnsupportedArchitecture,
    UnsupportedGuestType,
    UnsupportedMemorySize,
    UnsupportedVcpuCount,
    Stage2(crate::arch::Stage2Error),
    Vcpu(super::VcpuInterruptError),
}

impl From<BuddyError> for Error {
    fn from(error: BuddyError) -> Self {
        Self::Allocation(error)
    }
}

impl From<hyper::hal::cache::CacheError> for Error {
    fn from(error: hyper::hal::cache::CacheError) -> Self {
        Self::Cache(error)
    }
}

impl From<fdt::Error> for Error {
    fn from(error: fdt::Error) -> Self {
        Self::DeviceTree(error)
    }
}

impl From<crate::arch::Stage2Error> for Error {
    fn from(error: crate::arch::Stage2Error) -> Self {
        Self::Stage2(error)
    }
}

impl From<super::interrupt::Error> for Error {
    fn from(error: super::interrupt::Error) -> Self {
        Self::Interrupts(error)
    }
}

impl From<super::VcpuInterruptError> for Error {
    fn from(error: super::VcpuInterruptError) -> Self {
        Self::Vcpu(error)
    }
}

pub fn boot(guest: VmBundle<'_>) -> Result<Infallible, Error> {
    if guest.guest_type() != "linux" {
        return Err(Error::UnsupportedGuestType);
    }
    if guest.architecture() != "aarch64" {
        return Err(Error::UnsupportedArchitecture);
    }
    let image = guest.kernel();
    if image.len() < AARCH64_IMAGE_HEADER_SIZE
        || image.get(AARCH64_IMAGE_MAGIC_OFFSET..AARCH64_IMAGE_MAGIC_OFFSET + 4) != Some(b"ARMd")
    {
        return Err(Error::InvalidKernel);
    }
    if guest.vcpu_count() != 1 || guest.vcpu_count() > hyper::config::MAX_CPUS as u32 {
        return Err(Error::UnsupportedVcpuCount);
    }
    let guest_ram_size = guest.memory_size();
    let guest_ram_pages = guest_ram_size / PAGE_SIZE;
    if guest_ram_pages == 0 || !guest_ram_pages.is_power_of_two() {
        return Err(Error::InvalidLayout);
    }
    let guest_ram_order = guest_ram_pages.trailing_zeros() as usize;
    if guest_ram_order > MAX_ORDER {
        return Err(Error::UnsupportedMemorySize);
    }
    let guest_ram = PageBlock::allocate(guest_ram_order)?;
    let ram_physical = guest_ram.physical();
    let ram_virtual = linear_address(ram_physical)?;
    // SAFETY: The contiguous block is exclusively owned by this VM and the
    // permanent linear map covers its complete physical range.
    let guest_ram_size_usize = usize::try_from(guest_ram_size).map_err(|_| Error::InvalidLayout)?;
    unsafe { write_bytes(ram_virtual as *mut u8, 0, guest_ram_size_usize) };

    let initramfs = guest.initramfs();
    let image_end = GUEST_KERNEL_IPA
        .checked_add(image.len() as u64)
        .ok_or(Error::AddressOverflow)?;
    let initramfs_range = match initramfs {
        Some(bytes) => {
            let start = align_up(image_end, 2 * 1024 * 1024)?;
            let end = start
                .checked_add(bytes.len() as u64)
                .ok_or(Error::AddressOverflow)?;
            Some((start, end))
        }
        None => None,
    };
    let ram_end = GUEST_RAM_IPA
        .checked_add(guest_ram_size)
        .ok_or(Error::AddressOverflow)?;
    let payload_end = initramfs_range.map_or(image_end, |(_, end)| end);
    if payload_end > ram_end {
        return Err(Error::InvalidLayout);
    }
    let device_tree = fdt::build(
        GUEST_RAM_IPA,
        guest_ram_size,
        initramfs_range,
        guest.command_line(),
        guest.vcpu_count(),
    )?;
    if GUEST_DTB_IPA + device_tree.len() as u64 > GUEST_KERNEL_IPA {
        return Err(Error::InvalidLayout);
    }

    copy_into_guest(ram_virtual, guest_ram_size, GUEST_KERNEL_IPA, image)?;
    if let (Some(bytes), Some((start, _))) = (initramfs, initramfs_range) {
        copy_into_guest(ram_virtual, guest_ram_size, start, bytes)?;
    }
    copy_into_guest(ram_virtual, guest_ram_size, GUEST_DTB_IPA, &device_tree)?;
    let image_virtual = guest_host_virtual(ram_virtual, guest_ram_size, GUEST_KERNEL_IPA)?;
    let dtb_virtual = guest_host_virtual(ram_virtual, guest_ram_size, GUEST_DTB_IPA)?;
    // SAFETY: These exclusively owned ranges have just been initialized and
    // cannot be observed by the guest before stage-2 activation and ERET.
    unsafe {
        crate::arch::ArchitectureCache::publish_instruction_range(image_virtual, image.len())?;
        if let (Some(bytes), Some((start, _))) = (initramfs, initramfs_range) {
            let initramfs_virtual = guest_host_virtual(ram_virtual, guest_ram_size, start)?;
            crate::arch::ArchitectureCache::publish_data_range(initramfs_virtual, bytes.len())?;
        }
        crate::arch::ArchitectureCache::publish_data_range(dtb_virtual, device_tree.len())?;
    }

    let mut table_pages = Stage2PagePool::new();
    let mut allocate_table = || table_pages.allocate_zeroed();
    let mut stage2 = crate::arch::Stage2AddressSpace::new(1, &mut allocate_table)?;
    stage2.map_normal(
        GUEST_RAM_IPA,
        ram_physical.get(),
        guest_ram_size,
        &mut allocate_table,
    )?;
    stage2.map_device(
        GUEST_UART_IPA,
        GUEST_UART_PHYSICAL,
        GUEST_UART_SIZE,
        &mut allocate_table,
    )?;

    let interrupts =
        VmInterruptController::new(guest.vcpu_count(), InterruptId::new(GUEST_TIMER_INTERRUPT))?;
    let mut context = crate::arch::VcpuContext::new(GUEST_KERNEL_IPA);
    context.general[0] = GUEST_DTB_IPA;
    context.general[1] = 0;
    context.general[2] = 0;
    context.general[3] = 0;
    context.set_virtual_count(
        crate::kernel::time::monotonic_ticks(),
        crate::kernel::time::monotonic_ticks(),
    );
    let _ = context.initialize_vgic().map_err(Error::Context)?;
    let mut execution = VcpuExecution {
        virtual_machine: VirtualMachineId(1),
        vcpu_id: 0,
        context,
    };

    crate::println!(
        "HypeR: entering Linux guest: Image {} bytes, initramfs {} bytes, RAM {} MiB",
        image.len(),
        initramfs.map_or(0, |bytes| bytes.len()),
        guest_ram_size / (1024 * 1024)
    );
    crate::println!(
        "HypeR: guest IPA layout: DTB {:#x}, Image {:#x}, initramfs {:#x}-{:#x}, stage-2 root {:#x}",
        GUEST_DTB_IPA,
        GUEST_KERNEL_IPA,
        initramfs_range.map_or(0, |(start, _)| start),
        initramfs_range.map_or(0, |(_, end)| end),
        stage2.root_address()
    );

    // SAFETY: All guest-visible RAM and device mappings are complete. These
    // stack objects remain pinned because guest entry never returns.
    unsafe {
        execution.activate_virtual_hardware(&interrupts)?;
        stage2.activate();
        crate::arch::enable_local_irq();
        execution.context.enter()
    }
}

struct Stage2PagePool {
    pages: Vec<PageBlock>,
}

impl Stage2PagePool {
    const fn new() -> Self {
        Self { pages: Vec::new() }
    }

    fn allocate_zeroed(&mut self) -> Option<PhysicalAddress> {
        if self.pages.try_reserve(1).is_err() {
            return None;
        }
        let page = PageBlock::allocate(0).ok()?;
        let physical = page.physical();
        let virtual_address = crate::kernel::mm::memory::linear_address(physical.get())?;
        // SAFETY: This newly allocated page is exclusively owned by the pool
        // and permanently mapped writable through the linear map.
        unsafe { write_bytes(virtual_address as *mut u8, 0, PAGE_SIZE as usize) };
        self.pages.push(page);
        Some(physical)
    }
}

fn copy_into_guest(
    ram_virtual: usize,
    ram_size: u64,
    ipa: u64,
    source: &[u8],
) -> Result<(), Error> {
    let offset = ipa.checked_sub(GUEST_RAM_IPA).ok_or(Error::InvalidLayout)?;
    let length = u64::try_from(source.len()).map_err(|_| Error::InvalidLayout)?;
    let end = offset.checked_add(length).ok_or(Error::AddressOverflow)?;
    if end > ram_size {
        return Err(Error::InvalidLayout);
    }
    let destination = guest_host_virtual(ram_virtual, ram_size, ipa)?;
    // SAFETY: Layout validation ensures the destination lies inside the
    // exclusive guest RAM allocation and does not overlap the static source.
    unsafe { copy_nonoverlapping(source.as_ptr(), destination as *mut u8, source.len()) };
    Ok(())
}

fn guest_host_virtual(ram_virtual: usize, ram_size: u64, ipa: u64) -> Result<usize, Error> {
    let offset = ipa.checked_sub(GUEST_RAM_IPA).ok_or(Error::InvalidLayout)?;
    if offset >= ram_size {
        return Err(Error::InvalidLayout);
    }
    ram_virtual
        .checked_add(offset as usize)
        .ok_or(Error::AddressOverflow)
}

fn linear_address(physical: PhysicalAddress) -> Result<usize, Error> {
    crate::kernel::mm::memory::linear_address(physical.get()).ok_or(Error::InvalidLayout)
}

fn align_up(value: u64, alignment: u64) -> Result<u64, Error> {
    value
        .checked_add(alignment - 1)
        .map(|rounded| rounded & !(alignment - 1))
        .ok_or(Error::AddressOverflow)
}
