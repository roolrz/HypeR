// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! `HypeR` Native syscall validation and dispatch.

/// Register arguments owned by one decoded Native syscall invocation.
pub(super) type Arguments = [u64; hyper::abi::native::HYPER_NATIVE_SYSCALL_ARGUMENT_REGISTERS];

mod services;

pub(in crate::kernel) use services::{
    ConsoleServiceError, ConsoleServices, DeferredAction, HandleServices, HierarchyServices,
    InspectServices, IpcServices, MemoryServices, ObjectServiceError, ObjectServices,
    ProcessBuilderServiceError, ProcessBuilderServices, SystemInspectServices, TaskServices,
    UserMemoryServices, VfsServices, VmServices,
};

mod dispatch;

pub(in crate::kernel) use dispatch::{dispatch_deferred, dispatch_immediate, is_immediate};
mod handlers;
#[cfg(feature = "kernel-self-test")]
mod self_test;
mod status;
mod wire;

#[cfg(feature = "kernel-self-test")]
pub(crate) use self_test::{SelfTestError, run_self_test};
