// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native Process, `TaskGroup`, and `UserThread` lifecycle ownership.

mod image;
mod lifecycle;
mod owner;
mod task_group;
mod user_thread;

pub(crate) use image::{
    AbiFamily, ExecutionRoute, ImageError, MachineAbi, ProcessImage, SupervisionSessionId,
    UserThreadStart,
};
pub(crate) use lifecycle::{ProcessPhase, TerminalReason, UserThreadPhase};
pub(crate) use owner::{
    AddressSpaceRetirement, HandleBatchPublishFailure, HandleTransferCommitFailure,
    PreparedProcess, PreparedProcessHandleTransfer, Process, ProcessCreateFailure, ProcessError,
    ProcessHandleBatchReservation, ProcessHandleReservation, ProcessId, ProcessRetirementStep,
    ProcessSnapshot, ProcessStopReport,
};
pub(crate) use task_group::{TaskGroup, TaskGroupError, TaskGroupId, TaskGroupStopReport};
pub(crate) use user_thread::{
    ActiveUserRun, PreparedUserRun, RunAdmissionError, StoppedUserRun, UserExecution, UserThread,
    UserThreadSnapshot,
};
