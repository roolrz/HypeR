// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native ipc service calls under borrowed Process authority.

use super::DeferredProcessServices;
use crate::kernel::abi::native::IpcServices;
use crate::kernel::capability::HandleValue;
use crate::kernel::ipc::{
    ByteChannelReadOutcome, ByteChannelServiceError, CapabilityChannelServiceError,
    CapabilityReceiveOutcome,
};
use crate::kernel::mm::user_space::UserSlice;
use crate::kernel::process::UserThreadPhase;

impl IpcServices for DeferredProcessServices<'_> {
    fn create_byte_channel(&self) -> Result<[HandleValue; 2], ByteChannelServiceError> {
        crate::kernel::ipc::byte_channel_create(self.process)
    }

    fn create_capability_channel(&self) -> Result<[HandleValue; 2], CapabilityChannelServiceError> {
        crate::kernel::ipc::capability_channel_create(self.process)
    }

    fn write_byte_channel(
        &self,
        endpoint: HandleValue,
        bytes: Option<UserSlice>,
    ) -> Result<(), ByteChannelServiceError> {
        crate::kernel::ipc::byte_channel_write(self.process, endpoint, bytes)
    }

    fn read_byte_channel(
        &self,
        endpoint: HandleValue,
        bytes: Option<UserSlice>,
    ) -> Result<ByteChannelReadOutcome, ByteChannelServiceError> {
        crate::kernel::ipc::byte_channel_read(self.process, endpoint, bytes)
    }

    fn try_send_capability_channel(
        &self,
        endpoint: HandleValue,
        bytes: Option<UserSlice>,
        dispositions: Option<UserSlice>,
    ) -> Result<(), CapabilityChannelServiceError> {
        crate::kernel::ipc::capability_channel_try_send(self.process, endpoint, bytes, dispositions)
    }

    fn receive_capability_channel(
        &self,
        endpoint: HandleValue,
        deadline: u64,
        bytes: Option<UserSlice>,
        slots: Option<UserSlice>,
    ) -> Result<CapabilityReceiveOutcome, CapabilityChannelServiceError> {
        crate::kernel::ipc::capability_channel_receive(
            self.process,
            endpoint,
            deadline,
            bytes,
            slots,
            || self.thread.snapshot().phase == UserThreadPhase::StopRequested,
        )
    }
}
