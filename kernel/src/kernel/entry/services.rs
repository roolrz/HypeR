// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Concrete Native service adapters over borrowed Process and Thread authority.
//!
//! The parent owns the shared syscall context and basic handle/user-copy calls;
//! each child implements a capability domain without extending the borrow.

mod affinity;
mod console;
mod device;
mod guest_io;
mod inspect;
mod ipc;
mod memory;
mod object;
mod process_builder;
mod task;
mod vfs;
mod vm;

use crate::kernel::abi::native::{HandleServices, ImmediateServices, UserMemoryServices};
use crate::kernel::capability::{HandleInfo, HandleValue, Rights};
use crate::kernel::mm::user_space::UserSlice;
use crate::kernel::process::{Process, ProcessError, UserThread};

/// Borrowed syscall authority; contains no machine-run or return-token state.
pub(super) struct DeferredProcessServices<'process> {
    process: &'process Process,
    thread: &'process UserThread,
}

impl<'process> DeferredProcessServices<'process> {
    pub(super) fn new(process: &'process Process, thread: &'process UserThread) -> Self {
        Self { process, thread }
    }
}

impl UserMemoryServices for DeferredProcessServices<'_> {
    fn copy_to_user(&self, destination: UserSlice, source: &[u8]) -> Result<(), ProcessError> {
        self.process.copy_to_user(destination, source)
    }

    fn copy_from_user(
        &self,
        source: UserSlice,
        destination: &mut [u8],
    ) -> Result<(), ProcessError> {
        self.process.copy_from_user(source, destination)
    }
}

impl ImmediateServices for DeferredProcessServices<'_> {
    fn close_handle(&self, value: HandleValue) -> Result<(), ProcessError> {
        self.process.close_handle(value)
    }
}

impl HandleServices for DeferredProcessServices<'_> {
    fn handle_info(
        &self,
        value: HandleValue,
        required_rights: Rights,
    ) -> Result<HandleInfo, ProcessError> {
        self.process.handle_info(value, required_rights)
    }

    fn duplicate_handle(
        &self,
        value: HandleValue,
        rights: Rights,
    ) -> Result<HandleValue, ProcessError> {
        self.process.duplicate_handle(value, rights)
    }
    fn replace_handle(
        &self,
        value: HandleValue,
        rights: Rights,
    ) -> Result<HandleValue, ProcessError> {
        self.process.replace_handle(value, rights)
    }
}
