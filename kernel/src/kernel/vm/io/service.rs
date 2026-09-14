// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native capability preparation and publication for guest I/O mechanisms.

use super::{Error, Mailbox, Notification};
use crate::kernel::capability::{HandleFlags, HandleValue, PreparedHandle, Rights};
use crate::kernel::object::{ObjectPublication, UserExportableObject};
use crate::kernel::process::{Process, ProcessError};
use crate::kernel::vm::objects::{Error as ObjectError, VirtualMachineObject};

fn publish<T: UserExportableObject>(
    process: &Process,
    payload: T,
    install: impl FnOnce() -> Result<(), Error>,
) -> Result<HandleValue, Error> {
    let output = process.reserve_handles::<1>()?;
    let prepare = || {
        let publication = ObjectPublication::try_new(payload).map_err(ObjectError::from)?;
        let prepared = PreparedHandle::try_from_new_object(
            publication,
            T::SUPPORTED_RIGHTS,
            HandleFlags::NONE,
        )
        .map_err(ProcessError::from)?;
        install()?;
        Ok::<_, Error>(prepared)
    };
    let prepared = match prepare() {
        Ok(prepared) => prepared,
        Err(error) => {
            process.abort_handles(output);
            return Err(error);
        }
    };
    // Once topology is installed, a concurrent process teardown closes the
    // prepared handle and therefore the bridge. Its immutable MMIO reservation
    // remains VM-owned until VM retirement, never a dangling userspace owner.
    process
        .publish_handles(output, [prepared])
        .map(|values| values[0])
        .map_err(|failure| failure.error.into())
}

pub(crate) fn create_mailbox(
    process: &Process,
    machine: HandleValue,
    base: u64,
    irq: u32,
) -> Result<HandleValue, Error> {
    let machine = process.resolve_handle::<VirtualMachineObject>(machine, Rights::WRITE)?;
    let owner = machine.object().owner();
    let mailbox = Mailbox::prepare(&owner, base, irq, &process.resource_domain())?;
    let installer = mailbox.clone();
    publish(process, mailbox, || installer.install(&owner))
}

pub(crate) fn send_mailbox(
    process: &Process,
    mailbox: HandleValue,
    bytes: &[u8],
) -> Result<(), Error> {
    let mailbox = process.resolve_handle::<Mailbox>(mailbox, Rights::WRITE)?;
    mailbox.object().send(bytes)
}

pub(crate) fn receive_mailbox(
    process: &Process,
    mailbox: HandleValue,
    mut copy: impl FnMut(&[u8]) -> Result<(), Error>,
) -> Result<usize, Error> {
    let mailbox = process.resolve_handle::<Mailbox>(mailbox, Rights::READ)?;
    let claim = mailbox.object().claim()?;
    let length = claim.bytes().len();
    copy(claim.bytes())?;
    claim.commit();
    Ok(length)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn create_notification(
    process: &Process,
    front: HandleValue,
    back: HandleValue,
    front_base: u64,
    back_base: u64,
    front_irq: u32,
    back_irq: u32,
) -> Result<HandleValue, Error> {
    let front = process.resolve_handle::<VirtualMachineObject>(front, Rights::WRITE)?;
    let back = process.resolve_handle::<VirtualMachineObject>(back, Rights::WRITE)?;
    let front = front.object().owner();
    let back = back.object().owner();
    let notification = Notification::prepare(
        &front,
        &back,
        front_base,
        back_base,
        front_irq,
        back_irq,
        &process.resource_domain(),
    )?;
    let installer = notification.clone();
    publish(process, notification, || installer.install(&front, &back))
}

pub(crate) fn control_notification(
    process: &Process,
    notification: HandleValue,
    operation: u32,
) -> Result<u32, Error> {
    let notification = process.resolve_handle::<Notification>(notification, Rights::WRITE)?;
    notification.object().control(operation)
}
