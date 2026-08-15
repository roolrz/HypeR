//! Initial single-vCPU Linux boot path for the QEMU virtual platform.

mod fdt;

use hyper::hal::interrupt::InterruptId;
use hyper::mm::{BuddyError, PAGE_SIZE};

use super::{VmBundle, VmInterruptController};
use crate::kernel::task::thread::{ThreadId, VirtualMachineId};
use crate::kernel::vm::memory::GuestAddressSpace;

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
    DeviceTree(fdt::Error),
    Interrupts(super::interrupt::Error),
    InvalidKernel,
    InvalidLayout,
    Memory(crate::kernel::vm::memory::Error),
    Runtime(super::runtime::Error),
    Scheduler(crate::kernel::task::scheduler::Error),
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

impl From<crate::kernel::vm::memory::Error> for Error {
    fn from(error: crate::kernel::vm::memory::Error) -> Self {
        Self::Memory(error)
    }
}

impl From<super::runtime::Error> for Error {
    fn from(error: super::runtime::Error) -> Self {
        Self::Runtime(error)
    }
}

impl From<crate::kernel::task::scheduler::Error> for Error {
    fn from(error: crate::kernel::task::scheduler::Error) -> Self {
        Self::Scheduler(error)
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

pub fn boot(guest: VmBundle<'_>) -> Result<ThreadId, Error> {
    validate_guest(&guest)?;
    let image = guest.kernel();
    let initramfs = guest.initramfs();
    let virtual_machine = VirtualMachineId(1);
    let mut address_space =
        GuestAddressSpace::new(virtual_machine, GUEST_RAM_IPA, guest.memory_size())?;
    let initramfs_range = layout_payload(image, initramfs, guest.memory_size())?;

    load_payload(&guest, &mut address_space, initramfs_range)?;
    address_space.finish_boot_loading();
    let stage2_root = address_space.root_address();
    crate::kernel::vm::memory::install(address_space)?;
    let guest_memory = crate::kernel::vm::memory::statistics(virtual_machine)
        .ok_or(crate::kernel::vm::memory::Error::NotInstalled)?;
    let (interrupts, context) = prepare_boot_vcpu(guest.vcpu_count())?;

    report_guest_layout(&guest, initramfs_range, stage2_root, guest_memory);
    crate::kernel::mm::report_statistics("guest prepared");
    super::runtime::install(virtual_machine, interrupts)?;
    let thread = super::vcpu::create_thread(virtual_machine, 0, context)?;
    crate::kernel::task::scheduler::thread_ready(thread)?;
    Ok(thread)
}

fn validate_guest(guest: &VmBundle<'_>) -> Result<(), Error> {
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
    if guest_ram_size == 0 || guest_ram_size & (PAGE_SIZE - 1) != 0 {
        return Err(Error::InvalidLayout);
    }
    Ok(())
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
    address_space: &mut GuestAddressSpace,
    initramfs_range: Option<(u64, u64)>,
) -> Result<(), Error> {
    let image = guest.kernel();
    let initramfs = guest.initramfs();
    let device_tree = fdt::build(
        GUEST_RAM_IPA,
        guest.memory_size(),
        initramfs_range,
        guest.command_line(),
        guest.vcpu_count(),
    )?;
    if GUEST_DTB_IPA + device_tree.len() as u64 > GUEST_KERNEL_IPA {
        return Err(Error::InvalidLayout);
    }

    address_space.write(GUEST_KERNEL_IPA, image)?;
    if let (Some(bytes), Some((start, _))) = (initramfs, initramfs_range) {
        address_space.write(start, bytes)?;
    }
    address_space.write(GUEST_DTB_IPA, &device_tree)?;
    address_space.publish_instruction(GUEST_KERNEL_IPA, image.len())?;
    if let (Some(bytes), Some((start, _))) = (initramfs, initramfs_range) {
        address_space.publish_data(start, bytes.len())?;
    }
    address_space.publish_data(GUEST_DTB_IPA, device_tree.len())?;
    Ok(())
}

fn prepare_boot_vcpu(
    vcpu_count: u32,
) -> Result<(VmInterruptController, crate::arch::VcpuContext), Error> {
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
    Ok((interrupts, context))
}

fn report_guest_layout(
    guest: &VmBundle<'_>,
    initramfs_range: Option<(u64, u64)>,
    stage2_root: u64,
    memory: crate::kernel::vm::memory::GuestMemoryStats,
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
    crate::println!(
        "HypeR: guest demand paging: {}/{} pages committed for boot",
        memory.boot_committed_pages,
        memory.addressable_pages
    );
}

fn align_up(value: u64, alignment: u64) -> Result<u64, Error> {
    value
        .checked_add(alignment - 1)
        .map(|rounded| rounded & !(alignment - 1))
        .ok_or(Error::AddressOverflow)
}
