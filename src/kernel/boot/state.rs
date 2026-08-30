// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use hyper::platform::ConsoleInfo;
use hyper::platform::PhysicalRange;
use hyper::platform::PlatformInfo;
use hyper::sync::PublishedOnce;

use crate::kernel::mm::PreparedMemory;

pub struct BootState {
    pub platform: PlatformInfo,
    pub essential: crate::hal::platform::EssentialInfo,
    pub early_console: Option<ConsoleInfo>,
    pub memory: PreparedMemory,
    pub dtb_address: u64,
    pub image_physical_start: u64,
    pub initial_ramdisk: PhysicalRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    AlreadyInstalled,
    NotInstalled,
}

static BOOT_STATE: PublishedOnce<BootState> = PublishedOnce::new();

pub fn install(state: BootState) -> Result<(), Error> {
    BOOT_STATE
        .publish(state)
        .map_err(|_| Error::AlreadyInstalled)
}

pub fn with<R>(operation: impl FnOnce(&BootState) -> R) -> Result<R, Error> {
    let state = BOOT_STATE.get().ok_or(Error::NotInstalled)?;
    Ok(operation(state))
}
