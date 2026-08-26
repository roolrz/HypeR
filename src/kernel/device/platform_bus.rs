// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Platform-device enumeration and driver binding orchestration.

use alloc::vec::Vec;
use core::mem::ManuallyDrop;

use hyper::{
    drivers::platform::{
        DeviceScanner, DriverManager, DriverServices, MmioMappingError, MmioResource,
        PermanentDriverManager, PermanentMmioMapping, PlatformDevice, PlatformDriver, ProbeError,
        ProbeReport, ScanError,
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
    _manager: PermanentDriverManager,
}

enum PlatformBusLifecycle {
    Empty,
    Preparing,
    Ready { _state: PlatformBusState },
}

struct InitializationReservation {
    active: bool,
}

static PLATFORM_BUS: KernelSpinLock<PlatformBusLifecycle> =
    KernelSpinLock::new(PlatformBusLifecycle::Empty);
static BUILTIN_DRIVERS: &[&dyn PlatformDriver] = &[
    &hyper::drivers::serial::PL011_PLATFORM_DRIVER,
    &hyper::drivers::serial::NS16550_PLATFORM_DRIVER,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    AlreadyInitialized,
    InitializationInProgress,
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
    let reservation = InitializationReservation::acquire()?;
    let mut scanner = DeviceScanner::new(boot.essential().claims());
    // SAFETY: The DTB reservation remains in the permanent RAM linear map.
    match unsafe { fdt::discover_with(boot.linear_dtb(), &mut scanner) } {
        Ok(_) => {}
        Err(fdt::WalkError::Fdt(error)) => return Err(Error::Fdt(error)),
        Err(fdt::WalkError::Visitor(error)) => return Err(Error::Scan(error)),
    }
    let mut devices = scanner.finish().map_err(Error::Scan)?;
    // Complete every fallible framework allocation before activating the
    // runtime console. From that publication onward initialization has no
    // recoverable error path which could discard its IRQ ownership.
    let mut manager = DriverManager::new();
    for &driver in BUILTIN_DRIVERS {
        manager
            .register(driver)
            .map_err(Error::DriverRegistration)?;
    }
    let console = super::serial::initialize(boot, &devices);
    let reserved_console_base = boot.early_console().map(|console| console.base);
    devices.retain(|device| {
        reserved_console_base.is_none_or(|reserved| {
            device.registers().first().map(|range| range.start()) != Some(reserved)
        })
    });
    let services = KernelDriverServices { boot };
    let drivers = manager.probe_devices(&devices, &services);
    reservation.commit(PlatformBusState {
        _devices: devices,
        _manager: manager.retain_permanently(),
    });
    Ok(InitializationReport { drivers, console })
}

impl InitializationReservation {
    fn acquire() -> Result<Self, Error> {
        PLATFORM_BUS.with(|lifecycle| match lifecycle {
            PlatformBusLifecycle::Empty => {
                *lifecycle = PlatformBusLifecycle::Preparing;
                Ok(Self { active: true })
            }
            PlatformBusLifecycle::Preparing => Err(Error::InitializationInProgress),
            PlatformBusLifecycle::Ready { .. } => Err(Error::AlreadyInitialized),
        })
    }

    fn commit(mut self, state: PlatformBusState) {
        let rejected = PLATFORM_BUS.with(|lifecycle| {
            if matches!(lifecycle, PlatformBusLifecycle::Preparing) {
                *lifecycle = PlatformBusLifecycle::Ready { _state: state };
                None
            } else {
                Some(state)
            }
        });
        if let Some(state) = rejected {
            // Keep all prepared devices and their permanent driver owner live
            // after releasing the platform-bus lock. Dropping that owner here
            // would enter its fail-stop path while the same lock was held.
            let _state = ManuallyDrop::new(state);
            crate::hal::cpu::halt()
        }
        self.active = false;
    }
}

impl Drop for InitializationReservation {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let rolled_back = PLATFORM_BUS.with(|lifecycle| {
            if matches!(lifecycle, PlatformBusLifecycle::Preparing) {
                *lifecycle = PlatformBusLifecycle::Empty;
                true
            } else {
                false
            }
        });
        if !rolled_back {
            crate::hal::cpu::halt()
        }
    }
}
