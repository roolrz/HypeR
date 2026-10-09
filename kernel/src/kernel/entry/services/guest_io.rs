// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native guest I/O service calls under borrowed Process authority.

use super::DeferredProcessServices;
use crate::kernel::capability::HandleValue;
use crate::kernel::mm::user_space::UserSlice;

impl crate::kernel::abi::native::GuestIoServices for DeferredProcessServices<'_> {
    fn create_guest_mapping(
        &self,
        backend: HandleValue,
        memory: HandleValue,
        frontend: u64,
    ) -> Result<(HandleValue, u64), crate::kernel::vm::service::Error> {
        crate::kernel::vm::create_guest_mapping(self.process, backend, memory, frontend)
    }
    fn release_guest_mapping(
        &self,
        mapping: HandleValue,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::release_guest_mapping(self.process, mapping)
    }
    fn create_native_block(
        &self,
        memory: HandleValue,
        backend: HandleValue,
        guest_base: u64,
        notification_base: u64,
        notification_irq: u32,
    ) -> Result<HandleValue, crate::kernel::vm::service::Error> {
        crate::kernel::block::service::create(
            self.process,
            memory,
            backend,
            guest_base,
            notification_base,
            notification_irq,
        )
    }
    fn activate_native_block(
        &self,
        block: HandleValue,
        readonly: bool,
    ) -> Result<(u64, bool), crate::kernel::block::service::ActivationError> {
        crate::kernel::block::service::activate(self.process, block, readonly)
    }
    fn transfer_native_block(
        &self,
        block: HandleValue,
        operation: u32,
        first: u64,
        buffer: Option<UserSlice>,
    ) -> Result<(), crate::kernel::block::service::TransferError> {
        crate::kernel::block::service::transfer(self.process, block, operation, first, buffer)
    }
    fn mount_filesystem(
        &self,
        channel: HandleValue,
        buffer: HandleValue,
        directory: HandleValue,
        path: UserSlice,
    ) -> Result<(), crate::kernel::vfs::VfsServiceError> {
        crate::kernel::vfs::service::mount_remote(self.process, channel, buffer, directory, path)
    }

    fn create_guest_mailbox(
        &self,
        machine: HandleValue,
        base: u64,
        irq: u32,
    ) -> Result<HandleValue, crate::kernel::vm::service::Error> {
        crate::kernel::vm::io::service::create_mailbox(self.process, machine, base, irq)
    }
    fn send_guest_mailbox(
        &self,
        mailbox: HandleValue,
        bytes: &[u8],
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::io::service::send_mailbox(self.process, mailbox, bytes)
    }
    fn receive_guest_mailbox(
        &self,
        mailbox: HandleValue,
        copy: &mut dyn FnMut(&[u8]) -> Result<(), crate::kernel::vm::service::Error>,
    ) -> Result<usize, crate::kernel::vm::service::Error> {
        crate::kernel::vm::io::service::receive_mailbox(self.process, mailbox, copy)
    }
    fn create_guest_notification(
        &self,
        frontend: HandleValue,
        backend: HandleValue,
        frontend_base: u64,
        backend_base: u64,
        frontend_irq: u32,
        backend_irq: u32,
    ) -> Result<HandleValue, crate::kernel::vm::service::Error> {
        crate::kernel::vm::io::service::create_notification(
            self.process,
            frontend,
            backend,
            frontend_base,
            backend_base,
            frontend_irq,
            backend_irq,
        )
    }
    fn control_guest_notification(
        &self,
        notification: HandleValue,
        operation: u32,
    ) -> Result<u32, crate::kernel::vm::service::Error> {
        crate::kernel::vm::io::service::control_notification(self.process, notification, operation)
    }
}
