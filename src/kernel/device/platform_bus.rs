// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Platform-device enumeration and driver binding orchestration.

use alloc::vec::Vec;

use hyper::{
    drivers::platform::{
        DeviceScanner, DriverManager, DriverServices, MmioMappingError, MmioResource,
        PermanentMmioMapping, PlatformDevice, PlatformDriver, ProbeError, ProbeReport, ScanError,
    },
    platform::fdt,
    sync::InterruptSpinLock,
};

type KernelSpinLock<T> = InterruptSpinLock<T, crate::hal::irq::LocalMask>;

struct KernelDriverServices<'a> {
    boot: &'a super::super::boot::Initialization,
}

impl DriverServices for KernelDriverServices<'_> {
    fn map_mmio(&self, resource: MmioResource) -> Result<PermanentMmioMapping, MmioMappingError> {
        if !self.boot.maps_mmio(resource) {
            return Err(MmioMappingError::NotMapped);
        }
        let virtual_start = crate::kernel::mm::memory::mmio_address(resource.start())
            .ok_or(MmioMappingError::AddressOverflow)?;
        // SAFETY: Final stage-1 construction maps every DTB-discovered MMIO
        // range with device attributes and retains those mappings permanently.
        unsafe {
            PermanentMmioMapping::new(
                resource,
                hyper::mm::VirtualAddress::new(virtual_start as u64),
            )
        }
    }
}

struct PlatformBusState {
    _devices: Vec<PlatformDevice>,
    _manager: DriverManager,
}

static PLATFORM_BUS: KernelSpinLock<Option<PlatformBusState>> = KernelSpinLock::new(None);
static BUILTIN_DRIVERS: &[&dyn PlatformDriver] = &[
    &hyper::drivers::serial::PL011_PLATFORM_DRIVER,
    &hyper::drivers::serial::NS16550_PLATFORM_DRIVER,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    AlreadyInitialized,
    DriverRegistration(ProbeError),
    Fdt(fdt::Error),
    Scan(ScanError),
}

pub(super) struct InitializationReport {
    pub(super) drivers: ProbeReport,
    pub(super) console: Result<Option<super::serial::Capabilities>, super::serial::Error>,
}

pub(super) fn initialize(
    boot: &super::super::boot::Initialization,
) -> Result<InitializationReport, Error> {
    let mut scanner = DeviceScanner::new(boot.essential().claims());
    // SAFETY: The DTB reservation remains in the permanent RAM linear map.
    match unsafe { fdt::discover_with(boot.linear_dtb(), &mut scanner) } {
        Ok(_) => {}
        Err(fdt::WalkError::Fdt(error)) => return Err(Error::Fdt(error)),
        Err(fdt::WalkError::Visitor(error)) => return Err(Error::Scan(error)),
    }
    let mut devices = scanner.finish().map_err(Error::Scan)?;
    let console = super::serial::initialize(boot, &devices);
    let reserved_console_base = boot.early_console().map(|console| console.base);
    devices.retain(|device| {
        reserved_console_base.is_none_or(|reserved| {
            device.registers().first().map(|range| range.start()) != Some(reserved)
        })
    });
    let mut manager = DriverManager::new();
    for &driver in BUILTIN_DRIVERS {
        manager
            .register(driver)
            .map_err(Error::DriverRegistration)?;
    }
    let services = KernelDriverServices { boot };
    let drivers = manager.probe_devices(&devices, &services);
    PLATFORM_BUS.with(|slot| {
        if slot.is_some() {
            return Err(Error::AlreadyInitialized);
        }
        *slot = Some(PlatformBusState {
            _devices: devices,
            _manager: manager,
        });
        Ok(InitializationReport { drivers, console })
    })
}
