// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use core::cell::UnsafeCell;
use core::mem::MaybeUninit;

use hyper::platform::ConsoleInfo;
use hyper::platform::PhysicalRange;
use hyper::platform::PlatformInfo;
use hyper::sync::atomic::{AtomicU8, Ordering};

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

const EMPTY: u8 = 0;
const INSTALLING: u8 = 1;
const READY: u8 = 2;

struct BootStateStorage {
    state: UnsafeCell<MaybeUninit<BootState>>,
    status: AtomicU8,
}

// SAFETY: Exactly one successful installer writes `state` before publishing
// READY with Release. Every reader requires an Acquire observation of READY and
// receives shared access only; the retained BootState is never mutated or freed.
unsafe impl Sync for BootStateStorage where BootState: Sync {}

impl BootStateStorage {
    const fn new() -> Self {
        Self {
            state: UnsafeCell::new(MaybeUninit::uninit()),
            status: AtomicU8::new(EMPTY),
        }
    }
}

static BOOT_STATE: BootStateStorage = BootStateStorage::new();

pub fn install(state: BootState) -> Result<(), Error> {
    if BOOT_STATE
        .status
        .compare_exchange(EMPTY, INSTALLING, Ordering::Relaxed, Ordering::Relaxed)
        .is_err()
    {
        return Err(Error::AlreadyInstalled);
    }

    // SAFETY: The successful EMPTY -> INSTALLING transition grants this call
    // unique initialization access. No reader can access the value until READY.
    unsafe { (*BOOT_STATE.state.get()).write(state) };
    BOOT_STATE.status.store(READY, Ordering::Release);
    Ok(())
}

pub fn with<R>(operation: impl FnOnce(&BootState) -> R) -> Result<R, Error> {
    if BOOT_STATE.status.load(Ordering::Acquire) != READY {
        return Err(Error::NotInstalled);
    }
    // SAFETY: Acquire observed the installer's Release publication. The value
    // is fully initialized, immutable, permanently retained, and never dropped.
    let state = unsafe { (&*BOOT_STATE.state.get()).assume_init_ref() };
    Ok(operation(state))
}
