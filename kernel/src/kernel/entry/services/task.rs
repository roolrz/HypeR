// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native task service calls under borrowed Process authority.

use super::DeferredProcessServices;
use super::affinity::AffinityInputError;
use crate::kernel::abi::native::{ObjectServiceError, TaskServices};
use crate::kernel::capability::{HandleValue, Rights};
use crate::kernel::mm::user_space::{UserAddress, UserSlice};
use crate::kernel::process::{
    ProcessError, ProcessObject, ProcessSnapshot, TerminalReason, UserThreadPhase,
};

impl TaskServices for DeferredProcessServices<'_> {
    fn create_thread(
        &self,
        entry: u64,
        stack: u64,
        tls: u64,
        argument: u64,
        affinity_words: Option<UserSlice>,
        affinity_word_count: usize,
    ) -> Result<HandleValue, ObjectServiceError> {
        let start = crate::kernel::process::UserThreadStart::try_new(
            UserAddress::new(entry),
            UserAddress::new(stack),
            UserAddress::new(tls),
        )
        .map_err(|_| ObjectServiceError::InvalidInput)?
        .with_argument(argument);
        let affinity = if affinity_words.is_none() && affinity_word_count == 0 {
            let caller = self
                .thread
                .scheduler_id()
                .ok_or(ObjectServiceError::InvalidInput)?;
            crate::kernel::task::scheduler::thread_placement(caller)
                .map_err(ProcessError::from)?
                .1
        } else {
            let affinity = self
                .copy_affinity(affinity_words, affinity_word_count)
                .map_err(|error| match error {
                    AffinityInputError::Invalid => ObjectServiceError::InvalidInput,
                    AffinityInputError::Memory(error) => ObjectServiceError::Process(error),
                })?;
            crate::kernel::task::scheduler::validate_affinity(affinity)
                .map_err(|_| ObjectServiceError::InvalidInput)?;
            affinity
        };
        let process = self.process;
        let thread = process.create_user_thread("native-worker", start, affinity)?;
        let rights = Rights::DUPLICATE
            .union(Rights::WAIT)
            .union(Rights::INSPECT)
            .union(Rights::START)
            .union(Rights::REQUEST_STOP);
        match process.publish_thread_handle(&thread, rights) {
            Ok(handle) => Ok(handle),
            Err(error) => {
                if let Some(id) = thread.scheduler_id() {
                    crate::kernel::task::scheduler::request_user_thread_stop(
                        id,
                        TerminalReason::Requested,
                    )
                    .map_err(ProcessError::from)?;
                }
                Err(error.into())
            }
        }
    }
    fn start_thread(&self, value: HandleValue) -> Result<(), ProcessError> {
        let thread = self
            .process
            .resolve_user_thread_handle(value, Rights::START)?;
        thread.ready()?;
        Ok(())
    }
    fn stop_thread(&self, value: HandleValue) -> Result<(), ProcessError> {
        let thread = self
            .process
            .resolve_user_thread_handle(value, Rights::REQUEST_STOP)?;
        if thread.snapshot().phase == UserThreadPhase::Detached {
            return Ok(());
        }
        if let Some(id) = thread.scheduler_id() {
            match crate::kernel::task::scheduler::request_user_thread_stop(
                id,
                TerminalReason::Requested,
            ) {
                Ok(()) => {}
                Err(crate::kernel::task::scheduler::Error::ThreadNotFound)
                    if thread.snapshot().phase == UserThreadPhase::Detached => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }
    fn atomic_wait(
        &self,
        address: u64,
        expected: u32,
        deadline: u64,
    ) -> Result<crate::kernel::task::WaitOutcome, ObjectServiceError> {
        crate::kernel::process::atomic_wait::wait(self.process, address, expected, deadline, || {
            self.thread.snapshot().phase == UserThreadPhase::StopRequested
        })
        .map_err(atomic_wait_error)
    }
    fn atomic_wake(&self, address: u64, count: u32) -> Result<u64, ObjectServiceError> {
        crate::kernel::process::atomic_wait::wake(self.process, address, count)
            .map_err(atomic_wait_error)
    }
    fn sleep_thread(
        &self,
        deadline: u64,
    ) -> Result<crate::kernel::task::WaitOutcome, ObjectServiceError> {
        crate::kernel::process::atomic_wait::sleep(self.process, deadline, || {
            self.thread.snapshot().phase == UserThreadPhase::StopRequested
        })
        .map_err(atomic_wait_error)
    }

    fn process_info(&self, process: HandleValue) -> Result<ProcessSnapshot, ProcessError> {
        Ok(self
            .process
            .resolve_handle::<ProcessObject>(process, Rights::INSPECT)?
            .object()
            .snapshot())
    }

    fn request_process_stop(&self, process: HandleValue) -> Result<(), ProcessError> {
        let process = self
            .process
            .resolve_handle::<ProcessObject>(process, Rights::REQUEST_STOP)?;
        process.object().request_stop(TerminalReason::Requested);
        Ok(())
    }
}

fn atomic_wait_error(error: crate::kernel::process::atomic_wait::Error) -> ObjectServiceError {
    use crate::kernel::process::atomic_wait::Error;
    match error {
        Error::Process(error) => ObjectServiceError::Process(error),
        Error::Wait(error) => ObjectServiceError::Wait(error),
        Error::InvalidInput => ObjectServiceError::InvalidInput,
    }
}
