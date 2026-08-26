// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Linux guest boot policy shared by all supported instruction sets.

use hyper::mm::{BuddyError, PAGE_SIZE};

use crate::kernel::task::thread::ThreadId;
use crate::kernel::vm::memory::GuestAddressSpace;
use crate::kernel::vm::registry::VmBuilder;
use crate::kernel::vm::{VmBundle, VmInterruptController};

#[derive(Debug)]
pub enum Error {
    AddressOverflow,
    Allocation(BuddyError),
    Cache(hyper::hal::cache::CacheError),
    DeviceTree(hyper::vm::fdt::Error),
    Interrupts(crate::hal::vm::InterruptError),
    Devices(crate::kernel::vm::device::Error),
    InvalidKernel,
    InvalidLayout,
    Memory(crate::kernel::vm::memory::Error),
    Registry(crate::kernel::vm::registry::Error),
    Scheduler(crate::kernel::task::scheduler::Error),
    UnsupportedArchitecture,
    UnsupportedGuestType,
    UnsupportedMemorySize,
    UnsupportedVcpuCount,
    VirtualizationUnavailable,
    Stage2(crate::hal::vm::Stage2Error),
    Vcpu(crate::kernel::vm::VcpuInterruptError),
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

impl From<hyper::vm::fdt::Error> for Error {
    fn from(error: hyper::vm::fdt::Error) -> Self {
        Self::DeviceTree(error)
    }
}

impl From<crate::hal::guest::Error> for Error {
    fn from(error: crate::hal::guest::Error) -> Self {
        match error {
            crate::hal::guest::Error::AddressOverflow => Self::AddressOverflow,
            crate::hal::guest::Error::DeviceTree(error) => Self::DeviceTree(error),
            crate::hal::guest::Error::InvalidKernel => Self::InvalidKernel,
            crate::hal::guest::Error::InvalidLayout => Self::InvalidLayout,
            crate::hal::guest::Error::VirtualizationUnavailable => Self::VirtualizationUnavailable,
        }
    }
}

impl From<crate::hal::guest::PayloadLoadError<crate::kernel::vm::memory::Error>> for Error {
    fn from(error: crate::hal::guest::PayloadLoadError<crate::kernel::vm::memory::Error>) -> Self {
        match error {
            crate::hal::guest::PayloadLoadError::Abi(error) => error.into(),
            crate::hal::guest::PayloadLoadError::Memory(error) => Self::Memory(error),
        }
    }
}

impl From<crate::hal::vm::Stage2Error> for Error {
    fn from(error: crate::hal::vm::Stage2Error) -> Self {
        Self::Stage2(error)
    }
}

impl From<crate::kernel::vm::memory::Error> for Error {
    fn from(error: crate::kernel::vm::memory::Error) -> Self {
        Self::Memory(error)
    }
}

impl From<crate::kernel::vm::registry::Error> for Error {
    fn from(error: crate::kernel::vm::registry::Error) -> Self {
        Self::Registry(error)
    }
}

impl From<crate::kernel::task::scheduler::Error> for Error {
    fn from(error: crate::kernel::task::scheduler::Error) -> Self {
        Self::Scheduler(error)
    }
}

impl From<crate::hal::vm::InterruptError> for Error {
    fn from(error: crate::hal::vm::InterruptError) -> Self {
        Self::Interrupts(error)
    }
}

impl From<crate::kernel::vm::device::Error> for Error {
    fn from(error: crate::kernel::vm::device::Error) -> Self {
        Self::Devices(error)
    }
}

impl From<crate::kernel::vm::VcpuInterruptError> for Error {
    fn from(error: crate::kernel::vm::VcpuInterruptError) -> Self {
        Self::Vcpu(error)
    }
}

pub fn boot(guest: VmBundle<'_>) -> Result<ThreadId, Error> {
    validate_guest(&guest)?;
    let abi = crate::hal::guest::linux_abi();
    let image = guest.kernel();
    let initramfs = guest.initramfs();
    let reservation = crate::kernel::vm::registry::reserve()?;
    let virtual_machine = reservation.id();
    let mut address_space = GuestAddressSpace::new(
        reservation.hardware_vmid(),
        abi.ram_base().get(),
        guest.memory_size(),
    )?;
    let initramfs_range = layout_payload(image, initramfs, guest.memory_size())?;

    crate::hal::guest::load_linux_payload(&guest, &mut address_space, initramfs_range)?;
    // Instruction publication precedes this handoff. Each CPU observes the
    // address space's instruction epoch and performs its local synchronization
    // before the vCPU enters there, including after migration.
    address_space.finish_boot_loading();
    let stage2_root = address_space.root_address();
    let guest_memory = address_space.statistics();
    let (interrupts, context) = prepare_boot_vcpu(guest.vcpu_count())?;
    let devices = crate::kernel::vm::device::prepare()?;

    report_guest_layout(&guest, initramfs_range, stage2_root, guest_memory);
    crate::kernel::mm::report_statistics("guest prepared");
    // Both unpublished capabilities roll themselves back until registry
    // publication and scheduler ownership are committed below.
    let builder = VmBuilder::new(reservation, address_space, interrupts, devices)?;
    let prepared = builder.prepare_boot_vcpu(0, context)?;
    let installed = prepared.install()?;
    debug_assert_eq!(installed.id(), virtual_machine);
    let thread = installed.boot_vcpu();
    crate::kernel::task::scheduler::thread_ready(thread)?;
    Ok(thread)
}

fn validate_guest(guest: &VmBundle<'_>) -> Result<(), Error> {
    crate::hal::guest::validate_linux_host()?;
    crate::hal::guest::describe_linux_host(|description| {
        crate::println!("{description}");
    });
    if guest.guest_type() != "linux" {
        return Err(Error::UnsupportedGuestType);
    }
    if guest.architecture() != crate::hal::guest::linux_abi().architecture() {
        return Err(Error::UnsupportedArchitecture);
    }
    crate::hal::guest::validate_linux_kernel(guest.kernel())?;
    // This is also the current VM execution invariant: address-space
    // activation handles migration residency, while one exclusive execution
    // claim prevents concurrent sibling vCPUs until synchronous VM-wide
    // shootdown is implemented.
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
) -> Result<Option<crate::hal::guest::PayloadRange>, Error> {
    let abi = crate::hal::guest::linux_abi();
    let payload_size = crate::hal::guest::linux_kernel_occupied_size(image)?;
    let image_end = abi
        .kernel_load()
        .get()
        .checked_add(payload_size)
        .ok_or(Error::AddressOverflow)?;
    let initramfs_range = match initramfs {
        Some(bytes) => {
            let start = align_up(image_end, 2 * 1024 * 1024)?;
            let length = u64::try_from(bytes.len()).map_err(|_| Error::AddressOverflow)?;
            let end = start.checked_add(length).ok_or(Error::AddressOverflow)?;
            Some(
                crate::hal::guest::PayloadRange::new(
                    hyper::vm::exit::GuestPhysicalAddress::new(start),
                    hyper::vm::exit::GuestPhysicalAddress::new(end),
                )
                .ok_or(Error::InvalidLayout)?,
            )
        }
        None => None,
    };
    let ram_end = abi
        .ram_base()
        .get()
        .checked_add(guest_ram_size)
        .ok_or(Error::AddressOverflow)?;
    let payload_end = initramfs_range.map_or(image_end, |range| range.end().get());
    if payload_end > ram_end {
        return Err(Error::InvalidLayout);
    }
    Ok(initramfs_range)
}

fn prepare_boot_vcpu(
    vcpu_count: u32,
) -> Result<(VmInterruptController, crate::hal::vm::VcpuContext), Error> {
    let interrupts =
        VmInterruptController::new(vcpu_count, crate::hal::guest::linux_abi().timer_interrupt())?;
    let mut context = crate::hal::guest::prepare_linux_vcpu_context();
    crate::hal::vm::set_virtual_count(
        &mut context,
        crate::kernel::time::monotonic_ticks(),
        crate::kernel::time::monotonic_ticks(),
    );
    Ok((interrupts, context))
}

fn report_guest_layout(
    guest: &VmBundle<'_>,
    initramfs_range: Option<crate::hal::guest::PayloadRange>,
    stage2_root: u64,
    memory: crate::kernel::vm::memory::GuestMemoryStats,
) {
    crate::println!(
        "HypeR: entering Linux guest: Image {} bytes, initramfs {} bytes, RAM {} MiB",
        guest.kernel().len(),
        guest.initramfs().map_or(0, |bytes| bytes.len()),
        guest.memory_size() / (1024 * 1024)
    );
    crate::hal::guest::describe_linux_layout(initramfs_range, stage2_root, |description| {
        crate::println!("{description}");
    });
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
