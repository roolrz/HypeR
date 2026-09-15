// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native Process, `TaskGroup`, and `UserThread` lifecycle ownership.

pub(crate) mod atomic_wait;
mod builder;
mod builder_input;
mod builder_policy;
mod directory;
pub(crate) mod hierarchy;
mod image;
mod lifecycle;
mod loader;
mod objects;
mod owner;
mod task_group;
mod user_thread;

pub(crate) use builder::{
    ProcessBuilder, ProcessBuilderError, StartupCapability, abort_process_builder,
    create_process_builder, start_process_builder,
};
pub(crate) use directory::{ProcessDiagnosticRef, ProcessScanCursor, scan};
pub(crate) use image::{
    AbiFamily, ExecutionRoute, ImageError, MachineAbi, ProcessImage, UserThreadStart,
};
pub(crate) use lifecycle::{ProcessPhase, TerminalReason, UserThreadPhase};
pub(crate) use loader::{Error as LoaderError, INITIAL_STACK_TOP, load_native};
pub(crate) use objects::{ProcessObject, TaskFactory, TaskGroupObject, TaskObjectError};
pub(crate) use owner::{
    ChildProcessStartError, PreparedDirectProcessHandleTransfer, PreparedProcess, Process,
    ProcessError, ProcessHandleBatchReservation, ProcessId, ProcessSnapshot, ProcessStopReport,
    promote_delayed_retirements, reap_one_process, retirement_work,
};
pub(crate) use task_group::{TaskGroup, TaskGroupError, TaskGroupId};
pub(in crate::kernel) use user_thread::UserExecutionOwnership;
pub(crate) use user_thread::{RunAdmissionError, StoppedUserRun, UserExecution, UserThread};

#[cfg(not(feature = "kernel-self-test"))]
pub(crate) use owner::ProcessHandleReservation;

#[cfg_attr(
    feature = "kernel-self-test",
    allow(
        unused_imports,
        reason = "Native diagnostics tests require user execution support"
    )
)]
pub(crate) use owner::ProcessNameSnapshot;
