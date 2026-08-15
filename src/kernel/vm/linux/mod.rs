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
    let guest_ram_order = validate_guest(&guest)?;
    let image = guest.kernel();
    let initramfs = guest.initramfs();
    let guest_ram = GuestRam::allocate(guest.memory_size(), guest_ram_order)?;
    let initramfs_range = layout_payload(image, initramfs, guest_ram.size)?;

    load_payload(&guest, &guest_ram, initramfs_range)?;
    let (stage2, _table_pages) = build_stage2_address_space(&guest_ram)?;
    let (interrupts, mut execution) = prepare_boot_vcpu(guest.vcpu_count())?;

    report_guest_layout(&guest, initramfs_range, stage2.root_address());
    enter_guest(&mut execution, &interrupts, &stage2)
}

fn validate_guest(guest: &VmBundle<'_>) -> Result<usize, Error> {
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
    Ok(guest_ram_order)
}

fn layout_payload(
    image: &[u8],
    initramfs: Option<&[u8]>,
    guest_ram_size: u64,
) -> Result<Option<(u64, u64)>, Error> {
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
    Ok(initramfs_range)
}

fn load_payload(
    guest: &VmBundle<'_>,
    guest_ram: &GuestRam,
    initramfs_range: Option<(u64, u64)>,
) -> Result<(), Error> {
    let image = guest.kernel();
    let initramfs = guest.initramfs();
    let device_tree = fdt::build(
        GUEST_RAM_IPA,
        guest_ram.size,
        initramfs_range,
        guest.command_line(),
        guest.vcpu_count(),
    )?;
    if GUEST_DTB_IPA + device_tree.len() as u64 > GUEST_KERNEL_IPA {
        return Err(Error::InvalidLayout);
    }

    copy_into_guest(
        guest_ram.virtual_address,
        guest_ram.size,
        GUEST_KERNEL_IPA,
        image,
    )?;
    if let (Some(bytes), Some((start, _))) = (initramfs, initramfs_range) {
        copy_into_guest(guest_ram.virtual_address, guest_ram.size, start, bytes)?;
    }
    copy_into_guest(
        guest_ram.virtual_address,
        guest_ram.size,
        GUEST_DTB_IPA,
        &device_tree,
    )?;
    let image_virtual =
        guest_host_virtual(guest_ram.virtual_address, guest_ram.size, GUEST_KERNEL_IPA)?;
    let dtb_virtual = guest_host_virtual(guest_ram.virtual_address, guest_ram.size, GUEST_DTB_IPA)?;
    // SAFETY: These exclusively owned ranges have just been initialized and
    // cannot be observed by the guest before stage-2 activation and ERET.
    unsafe {
        crate::arch::ArchitectureCache::publish_instruction_range(image_virtual, image.len())?;
        if let (Some(bytes), Some((start, _))) = (initramfs, initramfs_range) {
            let initramfs_virtual =
                guest_host_virtual(guest_ram.virtual_address, guest_ram.size, start)?;
            crate::arch::ArchitectureCache::publish_data_range(initramfs_virtual, bytes.len())?;
        }
        crate::arch::ArchitectureCache::publish_data_range(dtb_virtual, device_tree.len())?;
    }
    Ok(())
}

fn build_stage2_address_space(
    guest_ram: &GuestRam,
) -> Result<(crate::arch::Stage2AddressSpace, Stage2PagePool), Error> {
    let mut table_pages = Stage2PagePool::new();
    let mut allocate_table = || table_pages.allocate_zeroed();
    let mut stage2 = crate::arch::Stage2AddressSpace::new(1, &mut allocate_table)?;
    stage2.map_normal(
        GUEST_RAM_IPA,
        guest_ram.physical.get(),
        guest_ram.size,
        &mut allocate_table,
    )?;
    Ok((stage2, table_pages))
}

fn prepare_boot_vcpu(vcpu_count: u32) -> Result<(VmInterruptController, VcpuExecution), Error> {
    let interrupts =
        VmInterruptController::new(vcpu_count, InterruptId::new(GUEST_TIMER_INTERRUPT))?;
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
    let execution = VcpuExecution {
        virtual_machine: VirtualMachineId(1),
        vcpu_id: 0,
        context,
    };
    Ok((interrupts, execution))
}

fn report_guest_layout(
    guest: &VmBundle<'_>,
    initramfs_range: Option<(u64, u64)>,
    stage2_root: u64,
) {
    crate::println!(
        "HypeR: entering Linux guest: Image {} bytes, initramfs {} bytes, RAM {} MiB",
        guest.kernel().len(),
        guest.initramfs().map_or(0, |bytes| bytes.len()),
        guest.memory_size() / (1024 * 1024)
    );
    crate::println!(
        "HypeR: guest IPA layout: DTB {:#x}, Image {:#x}, initramfs {:#x}-{:#x}, stage-2 root {:#x}",
        GUEST_DTB_IPA,
        GUEST_KERNEL_IPA,
        initramfs_range.map_or(0, |(start, _)| start),
        initramfs_range.map_or(0, |(_, end)| end),
        stage2_root
    );
}

fn enter_guest(
    execution: &mut VcpuExecution,
    interrupts: &VmInterruptController,
    stage2: &crate::arch::Stage2AddressSpace,
) -> Result<Infallible, Error> {
    // SAFETY: All guest-visible RAM and device mappings are complete. These
    // stack objects remain pinned because guest entry never returns.
    unsafe {
        execution.activate_virtual_hardware(interrupts)?;
        stage2.activate();
        crate::arch::enable_local_irq();
        execution.context.enter()
    }
}

struct GuestRam {
    _pages: PageBlock,
    physical: PhysicalAddress,
    virtual_address: usize,
    size: u64,
}

impl GuestRam {
    fn allocate(size: u64, order: usize) -> Result<Self, Error> {
        let pages = PageBlock::allocate(order)?;
        let physical = pages.physical();
        let virtual_address = linear_address(physical)?;
        let size_usize = usize::try_from(size).map_err(|_| Error::InvalidLayout)?;
        // SAFETY: The contiguous block is exclusively owned by this VM and the
        // permanent linear map covers its complete physical range.
        unsafe { write_bytes(virtual_address as *mut u8, 0, size_usize) };
        Ok(Self {
            _pages: pages,
            physical,
            virtual_address,
            size,
        })
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
