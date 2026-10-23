// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native ipc service calls under borrowed Process authority.

use crate::kernel::capability::InvocationReceiver;

use super::ProcessServices;
use crate::kernel::abi::native::IpcServices;
use crate::kernel::capability::HandleValue;
use crate::kernel::ipc::{
    ByteChannelReadOutcome, ByteChannelServiceError, CapabilityChannelServiceError,
    CapabilityReceiveOutcome,
};
use crate::kernel::mm::user_space::UserSlice;
use crate::kernel::process::UserThreadPhase;

impl IpcServices for ProcessServices<'_> {
    fn create_byte_channel(&self) -> Result<[HandleValue; 2], ByteChannelServiceError> {
        crate::kernel::ipc::byte_channel_create(self.process)
    }

    fn create_capability_channel(&self) -> Result<[HandleValue; 2], CapabilityChannelServiceError> {
        crate::kernel::ipc::capability_channel_create(self.process)
    }

    fn write_byte_channel(
        &self,
        endpoint: &InvocationReceiver,
        bytes: Option<UserSlice>,
    ) -> Result<(), ByteChannelServiceError> {
        let result = crate::kernel::ipc::byte_channel_write(self.process, endpoint, bytes);
        crate::tracepoint!(
            Ipc,
            "byte_send",
            thread = self.thread.scheduler_id().map_or(0, |id| id.get()),
            channel = endpoint.info().koid.get(),
            bytes = bytes.map_or(0, |b| b.length()),
            success = u64::from(result.is_ok())
        );
        result
    }

    fn read_byte_channel(
        &self,
        endpoint: &InvocationReceiver,
        bytes: Option<UserSlice>,
    ) -> Result<ByteChannelReadOutcome, ByteChannelServiceError> {
        let result = crate::kernel::ipc::byte_channel_read(self.process, endpoint, bytes);
        crate::tracepoint!(
            Ipc,
            "byte_receive",
            thread = self.thread.scheduler_id().map_or(0, |id| id.get()),
            channel = endpoint.info().koid.get(),
            bytes = match &result {
                Ok(ByteChannelReadOutcome::Received { bytes }) => *bytes,
                _ => 0,
            },
            success = u64::from(matches!(
                &result,
                Ok(ByteChannelReadOutcome::Received { .. })
            ))
        );
        result
    }

    fn try_send_capability_channel(
        &self,
        endpoint: &InvocationReceiver,
        bytes: Option<UserSlice>,
        dispositions: Option<UserSlice>,
    ) -> Result<(), CapabilityChannelServiceError> {
        let result = crate::kernel::ipc::capability_channel_try_send(
            self.process,
            endpoint,
            bytes,
            dispositions,
        );
        crate::tracepoint!(
            Ipc,
            "capability_send",
            thread = self.thread.scheduler_id().map_or(0, |id| id.get()),
            channel = endpoint.info().koid.get(),
            bytes = bytes.map_or(0, |b| b.length()),
            success = u64::from(result.is_ok())
        );
        result
    }

    fn receive_capability_channel(
        &self,
        endpoint: &InvocationReceiver,
        deadline: u64,
        bytes: Option<UserSlice>,
        slots: Option<UserSlice>,
    ) -> Result<CapabilityReceiveOutcome, CapabilityChannelServiceError> {
        let result = crate::kernel::ipc::capability_channel_receive(
            self.process,
            endpoint,
            deadline,
            bytes,
            slots,
            || self.thread.snapshot().phase == UserThreadPhase::StopRequested,
        );
        crate::tracepoint!(
            Ipc,
            "capability_receive",
            thread = self.thread.scheduler_id().map_or(0, |id| id.get()),
            channel = endpoint.info().koid.get(),
            bytes = match &result {
                Ok(CapabilityReceiveOutcome::Delivered(info)) => info.bytes as u64,
                _ => 0,
            },
            success = u64::from(matches!(
                &result,
                Ok(CapabilityReceiveOutcome::Delivered(_))
            ))
        );
        result
    }
}
