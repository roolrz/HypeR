// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Read-only postmortem access under an explicitly delegated capability.

use super::ProcessServices;
use crate::kernel::abi::native::{ProcessBuilderServiceError, RetentionServices};
use crate::kernel::accounting::{ResourceAmount, ResourceKind};
use crate::kernel::authority::Rights;
use crate::kernel::capability::{HandleSnapshot, HandleValue};
use crate::kernel::mm::user_space::{CrashMapping, UserSlice};
use crate::kernel::process::retention::{
    CrashView, Error, Info, ProcessRetirement, ThreadEvidence,
};
use crate::kernel::process::{ProcessBuilder, ProcessError};

impl RetentionServices for ProcessServices<'_> {
    fn retain_process_builder(
        &self,
        builder: HandleValue,
        bytes: u64,
        timeout: u64,
    ) -> Result<HandleValue, ProcessBuilderServiceError> {
        let builder = self.process.resolve_handle::<ProcessBuilder>(
            builder,
            Rights::WRITE.union(Rights::RETAIN_PROCESS),
        )?;
        // Object allocation and handle capacity are prepared before changing
        // policy. Publishing the prepared handle can now fail only if the
        // caller has stopped; withdrawing its authority revokes retention.
        let reservation = self.process.reserve_handles::<1>()?;
        let control = match builder
            .object()
            .retain_on_exit(self.process, bytes, timeout)
        {
            Ok(control) => control,
            Err(error) => {
                self.process.abort_handles(reservation);
                return Err(error.into());
            }
        };
        self.process
            .publish_handles(reservation, [control])
            .map(|values| values[0])
            .map_err(|failure| failure.error.into())
    }
    fn retention_info(&self, handle: HandleValue, view: bool) -> Result<Info, Error> {
        if view {
            self.process
                .resolve_handle::<CrashView>(handle, Rights::INSPECT)?
                .object()
                .info()
        } else {
            Ok(self
                .process
                .resolve_handle::<ProcessRetirement>(handle, Rights::INSPECT)?
                .object()
                .info())
        }
    }
    fn retention_open_view(&self, handle: HandleValue) -> Result<HandleValue, Error> {
        let control = self
            .process
            .resolve_handle::<ProcessRetirement>(handle, Rights::INSPECT_DETAILS)?;
        if control.object().info().process_koid == self.process.koid().get() {
            return Err(Error::Process(ProcessError::Handle(
                crate::kernel::capability::HandleError::AccessDenied,
            )));
        }
        let view = control
            .object()
            .open_view(&self.process.resource_domain())?;
        self.process
            .create_object(view, CrashView::RIGHTS)
            .map_err(Into::into)
    }
    fn retention_release(&self, handle: HandleValue) -> Result<(), Error> {
        self.process
            .resolve_handle::<ProcessRetirement>(handle, Rights::RETIRE)?
            .object()
            .release();
        Ok(())
    }
    fn crash_thread(
        &self,
        handle: HandleValue,
        index: usize,
    ) -> Result<(ThreadEvidence, usize), Error> {
        self.process
            .resolve_handle::<CrashView>(handle, Rights::READ)?
            .object()
            .thread(index)
    }
    fn crash_mapping(
        &self,
        handle: HandleValue,
        index: usize,
    ) -> Result<(CrashMapping, usize), Error> {
        self.process
            .resolve_handle::<CrashView>(handle, Rights::READ)?
            .object()
            .mapping(index)
    }
    fn crash_handle(
        &self,
        handle: HandleValue,
        index: usize,
    ) -> Result<(HandleSnapshot, usize), Error> {
        self.process
            .resolve_handle::<CrashView>(handle, Rights::READ)?
            .object()
            .handle(index)
    }
    fn crash_read(
        &self,
        handle: HandleValue,
        address: u64,
        output: UserSlice,
    ) -> Result<usize, Error> {
        let size = usize::try_from(output.length()).map_err(|_| Error::Invalid)?;
        if size == 0 || size > 65536 {
            return Err(Error::Invalid);
        }
        let view = self
            .process
            .resolve_handle::<CrashView>(handle, Rights::READ)?;
        let _charge = self
            .process
            .resource_domain()
            .reserve(ResourceAmount::ZERO.with(ResourceKind::KernelMemoryBytes, size as u64))
            .map_err(ProcessError::from)?
            .commit();
        let mut buffer = alloc::vec::Vec::new();
        buffer
            .try_reserve_exact(size)
            .map_err(|_| Error::Process(ProcessError::Allocation))?;
        buffer.resize(size, 0);
        let count = view.object().read(address, &mut buffer)?;
        let target = UserSlice::new(output.base(), count as u64).map_err(|_| Error::Invalid)?;
        self.process.copy_to_user(target, &buffer[..count])?;
        Ok(count)
    }
}
