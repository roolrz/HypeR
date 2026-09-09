// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Linux guest boot policy shared by all supported instruction sets.

pub(super) mod abi;
mod selected;

use hyper::mm::{BuddyError, PAGE_SIZE};

use crate::kernel::task::thread::ThreadId;
use crate::kernel::vm::VmBundle;
use crate::kernel::vm::memory::GuestAddressSpace;
use crate::kernel::vm::registry::VmBuilder;

pub(crate) enum Error {
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
    Resource(crate::kernel::accounting::ResourceError),
    Scheduler(crate::kernel::task::scheduler::Error),
    UnsupportedArchitecture,
    UnsupportedGuestType,
    UnsupportedVcpuCount,
    VirtualizationUnavailable,
    Stage2(crate::hal::vm::Stage2Error),
    Vcpu(crate::kernel::vm::VcpuInterruptError),
}

impl core::fmt::Debug for Error {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::AddressOverflow => formatter.write_str("AddressOverflow"),
            Self::Allocation(error) => formatter.debug_tuple("Allocation").field(error).finish(),
            Self::Cache(error) => formatter.debug_tuple("Cache").field(error).finish(),
            Self::DeviceTree(error) => formatter.debug_tuple("DeviceTree").field(error).finish(),
            Self::Interrupts(error) => formatter.debug_tuple("Interrupts").field(error).finish(),
            Self::Devices(error) => formatter.debug_tuple("Devices").field(error).finish(),
            Self::InvalidKernel => formatter.write_str("InvalidKernel"),
            Self::InvalidLayout => formatter.write_str("InvalidLayout"),
            Self::Memory(error) => formatter.debug_tuple("Memory").field(error).finish(),
            Self::Registry(error) => formatter.debug_tuple("Registry").field(error).finish(),
            Self::Resource(error) => formatter.debug_tuple("Resource").field(error).finish(),
            Self::Scheduler(error) => formatter.debug_tuple("Scheduler").field(error).finish(),
            Self::UnsupportedArchitecture => formatter.write_str("UnsupportedArchitecture"),
            Self::UnsupportedGuestType => formatter.write_str("UnsupportedGuestType"),
            Self::UnsupportedVcpuCount => formatter.write_str("UnsupportedVcpuCount"),
            Self::VirtualizationUnavailable => formatter.write_str("VirtualizationUnavailable"),
            Self::Stage2(error) => formatter.debug_tuple("Stage2").field(error).finish(),
            Self::Vcpu(error) => formatter.debug_tuple("Vcpu").field(error).finish(),
        }
    }
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

impl From<abi::PayloadLoadError<crate::kernel::vm::memory::Error>> for Error {
    fn from(error: abi::PayloadLoadError<crate::kernel::vm::memory::Error>) -> Self {
        match error {
            abi::PayloadLoadError::Abi(error) => error,
            abi::PayloadLoadError::Memory(error) => Self::Memory(error),
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

impl From<crate::kernel::vm::registry::VcpuPreparationError> for Error {
    fn from(error: crate::kernel::vm::registry::VcpuPreparationError) -> Self {
        match error {
            crate::kernel::vm::registry::VcpuPreparationError::Registry(error) => {
                Self::Registry(error)
            }
            crate::kernel::vm::registry::VcpuPreparationError::Scheduler(error) => {
                Self::Scheduler(error)
            }
        }
    }
}

impl From<crate::kernel::accounting::ResourceError> for Error {
    fn from(error: crate::kernel::accounting::ResourceError) -> Self {
        Self::Resource(error)
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
    let abi = selected::linux_abi();
    let image = guest.kernel();
    let initramfs = guest.initramfs();
    let resource_domain = crate::kernel::accounting::ResourceDomain::try_new_root(
        crate::kernel::accounting::ResourceLimits::UNLIMITED,
    )?;
    let lifecycle_resources = crate::kernel::vm::registry::VmLifecycleResources::try_reserve(
        &resource_domain,
        guest.vcpu_count(),
    )?;
    let mut reservation = crate::kernel::vm::registry::reserve()?;
    let virtual_machine = reservation.id();
    let mut address_space = GuestAddressSpace::new(
        reservation.take_hardware_vmid()?,
        abi.ram_base().get(),
        guest.memory_size(),
        &resource_domain,
    )?;
    let initramfs_range = layout_payload(image, initramfs, guest.memory_size())?;

    let initramfs_matches_range = match (initramfs, initramfs_range) {
        (None, None) => true,
        (Some(bytes), Some(range)) => u64::try_from(bytes.len()).ok() == Some(range.length()),
        _ => false,
    };
    if !initramfs_matches_range {
        return Err(Error::InvalidLayout);
    }

    selected::load_linux_payload(&guest, &mut address_space, initramfs_range)?;
    // Instruction publication precedes this handoff. Each CPU observes the
    // address space's instruction epoch and performs its local synchronization
    // before the vCPU enters there, including after migration.
    address_space.finish_boot_loading();
    let stage2_root = address_space.root_address();
    let guest_memory = address_space.statistics();
    let (interrupt_plan, context) = prepare_boot_vcpu(guest.vcpu_count())?;
    let interrupt_controller_charge = lifecycle_resources.reserve_interrupt_controller(
        crate::hal::vm::prepared_interrupt_controller_allocation_size(&interrupt_plan),
    )?;
    let interrupts = crate::hal::vm::create_prepared_interrupt_controller(interrupt_plan)?;
    let devices = crate::kernel::vm::device::prepare(Some(
        crate::kernel::vm::device::VirtualSerialBinding::for_host_test(),
    ))?;

    report_guest_layout(&guest, initramfs_range, stage2_root, guest_memory);
    crate::kernel::mm::report_statistics("guest prepared");
    // Both unpublished capabilities roll themselves back until registry
    // publication and scheduler ownership are committed below.
    let builder = VmBuilder::new(
        reservation,
        lifecycle_resources,
        crate::kernel::vm::installed::VirtualMachineConfiguration {
            guest_physical_base: abi.ram_base().get(),
            memory_size: guest.memory_size(),
            vcpu_count: guest.vcpu_count(),
            architecture: crate::hal::vm::guest_architecture_abi(),
            platform_profile: hyper::abi::native::HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE
                as u32,
        },
        address_space,
        interrupts,
        interrupt_controller_charge,
        devices,
    )?;
    let prepared = builder.prepare_boot_vcpu(0, context)?;
    let installed = prepared.install()?;
    let (installed_id, thread, control) = installed.into_started_boot_parts();
    debug_assert_eq!(installed_id, virtual_machine);
    let _ = crate::kernel::vm::device::try_publish_console_route(installed_id, 0, thread);
    crate::kernel::vm::lifecycle::retain_default(control);
    Ok(thread)
}

fn validate_guest(guest: &VmBundle<'_>) -> Result<(), Error> {
    if !crate::hal::vm::guest_execution_available() {
        return Err(Error::VirtualizationUnavailable);
    }
    selected::describe_linux_host(|description| {
        crate::pr_info!("{description}");
    });
    if guest.guest_type() != "linux" {
        return Err(Error::UnsupportedGuestType);
    }
    if guest.architecture() != selected::linux_abi().architecture() {
        return Err(Error::UnsupportedArchitecture);
    }
    selected::validate_linux_kernel(guest.kernel())?;
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
) -> Result<Option<abi::PayloadRange>, Error> {
    let abi = selected::linux_abi();
    let payload_size = selected::linux_kernel_occupied_size(image)?;
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
                abi::PayloadRange::new(
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
) -> Result<
    (
        crate::hal::vm::PreparedInterruptController,
        crate::hal::vm::VcpuContext,
    ),
    Error,
> {
    let interrupts = crate::hal::vm::prepare_interrupt_controller(
        vcpu_count,
        selected::linux_abi().timer_interrupt(),
    )?;
    let mut context = selected::prepare_linux_vcpu_context()?;
    crate::hal::vm::set_virtual_count(
        &mut context,
        crate::kernel::time::monotonic_ticks(),
        crate::kernel::time::monotonic_ticks(),
    );
    Ok((interrupts, context))
}

fn report_guest_layout(
    guest: &VmBundle<'_>,
    initramfs_range: Option<abi::PayloadRange>,
    stage2_root: u64,
    memory: crate::kernel::vm::memory::GuestMemoryStats,
) {
    crate::pr_info!(
        "HypeR: entering Linux guest: Image {} bytes, initramfs {} bytes, RAM {} MiB",
        guest.kernel().len(),
        guest.initramfs().map_or(0, |bytes| bytes.len()),
        guest.memory_size() / (1024 * 1024)
    );
    selected::describe_linux_guest_layout(initramfs_range, stage2_root, |description| {
        crate::pr_info!("{description}");
    });
    crate::pr_info!(
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

#[cfg(feature = "kernel-self-test")]
pub(crate) const fn test_abi() -> (u64, hyper::vm::interrupt::VirtualInterruptId) {
    let abi = selected::linux_abi();
    (abi.ram_base().get(), abi.timer_interrupt())
}

#[cfg(feature = "kernel-self-test")]
pub(crate) fn test_boot_context() -> Option<crate::hal::vm::VcpuContext> {
    selected::prepare_linux_vcpu_context().ok()
}
