// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Capability-checked Native VMO and VMAR operations.

use alloc::vec::Vec;

use hyper::mm::PAGE_SIZE;

use super::{
    Access, MachineError, MemoryObjectError, Permissions, UserAddress, UserSlice, VmarObject,
    VmoObject,
};
use crate::kernel::authority::Rights;
use crate::kernel::capability::HandleValue;
use crate::kernel::object::ObjectKind;
use crate::kernel::process::{Process, ProcessError};
use crate::kernel::vfs::{FileObject, VfsError};

const MAXIMUM_VMO_SIZE: u64 = hyper::abi::native::HYPER_NATIVE_VMO_MAX_SIZE_BYTES;
const MAXIMUM_TRANSFER: usize = hyper::abi::native::HYPER_NATIVE_VMO_MAX_TRANSFER_BYTES as usize;

#[derive(Debug)]
pub(crate) enum ServiceError {
    InvalidInput,
    Machine(MachineError),
    MemoryObject(MemoryObjectError),
    Process(ProcessError),
    Scheduler(crate::kernel::task::scheduler::Error),
    Vfs(VfsError),
}

impl From<MachineError> for ServiceError {
    fn from(error: MachineError) -> Self {
        Self::Machine(error)
    }
}

impl From<MemoryObjectError> for ServiceError {
    fn from(error: MemoryObjectError) -> Self {
        Self::MemoryObject(error)
    }
}

impl From<ProcessError> for ServiceError {
    fn from(error: ProcessError) -> Self {
        Self::Process(error)
    }
}

impl From<VfsError> for ServiceError {
    fn from(error: VfsError) -> Self {
        Self::Vfs(error)
    }
}

pub(crate) fn create_vmo(process: &Process, size: u64) -> Result<HandleValue, ServiceError> {
    let size = page_aligned_size(size)?;
    let object = VmoObject::try_new_writable(size, &process.resource_domain())?;
    Ok(process.create_object(object, writable_vmo_rights())?)
}

pub(crate) fn create_file_executable_vmo(
    process: &Process,
    file: HandleValue,
) -> Result<HandleValue, ServiceError> {
    let file = process.resolve_handle::<FileObject>(file, Rights::READ.union(Rights::EXECUTE))?;
    let snapshot = file
        .object()
        .executable_snapshot(&process.resource_domain())?;
    let byte_count =
        u64::try_from(snapshot.bytes().len()).map_err(|_| ServiceError::InvalidInput)?;
    let size = page_aligned_size(byte_count)?;
    let writable = VmoObject::try_new_writable(size, &process.resource_domain())?;
    writable.write(0, snapshot.bytes())?;
    let pin = crate::kernel::task::scheduler::preempt_disable().map_err(ServiceError::Scheduler)?;
    let executable = writable.try_loader_executable_snapshot(&pin, &process.resource_domain());
    crate::kernel::task::scheduler::preempt_enable_without_reschedule(pin)
        .map_err(ServiceError::Scheduler)?;
    let executable = executable?;
    Ok(process.create_object(executable, executable_vmo_rights())?)
}

pub(crate) fn read_vmo(
    process: &Process,
    value: HandleValue,
    offset: u64,
    destination: Option<UserSlice>,
) -> Result<(), ServiceError> {
    let Some(destination) = destination else {
        return Ok(());
    };
    let length = transfer_length(destination)?;
    let vmo = process.resolve_handle::<VmoObject>(value, Rights::READ)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| ProcessError::Allocation)?;
    bytes.resize(length, 0);
    vmo.object().read(offset, &mut bytes)?;
    process.copy_to_user(destination, &bytes)?;
    Ok(())
}

pub(crate) fn write_vmo(
    process: &Process,
    value: HandleValue,
    offset: u64,
    source: Option<UserSlice>,
) -> Result<(), ServiceError> {
    let Some(source) = source else {
        return Ok(());
    };
    let length = transfer_length(source)?;
    let vmo = process.resolve_handle::<VmoObject>(value, Rights::WRITE)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| ProcessError::Allocation)?;
    bytes.resize(length, 0);
    process.copy_from_user(source, &mut bytes)?;
    vmo.object().write(offset, &bytes)?;
    Ok(())
}

pub(crate) fn allocate_vmar(
    process: &Process,
    parent: HandleValue,
    address: u64,
    size: u64,
) -> Result<HandleValue, ServiceError> {
    let range = aligned_range(address, size)?;
    let parent = process.resolve_handle::<VmarObject>(parent, Rights::MAP)?;
    let reservation = process.reserve_handles::<1>()?;
    let child = match VmarObject::try_child(parent.object(), range, &process.resource_domain()) {
        Ok(child) => child,
        Err(error) => {
            process.abort_handles(reservation);
            return Err(error.into());
        }
    };
    let child_token = child.token();
    match process.publish_reserved_object(reservation, child, vmar_rights()) {
        Ok(value) => Ok(value),
        Err(error) => {
            let rollback = parent
                .object()
                .address_space()
                .logical()
                .destroy_vmar(child_token);
            if let Err(rollback_error) = rollback {
                crate::kernel::crash::fatal(format_args!(
                    "HypeR: failed to roll back unpublished VMAR after handle publication error: \
                     {rollback_error:?}"
                ));
            }
            Err(error.into())
        }
    }
}

pub(crate) fn map_vmo(
    process: &Process,
    vmar: HandleValue,
    vmo: HandleValue,
    object_offset: u64,
    address: u64,
    size: u64,
    permissions: Permissions,
) -> Result<(), ServiceError> {
    if !object_offset.is_multiple_of(PAGE_SIZE) {
        return Err(ServiceError::InvalidInput);
    }
    let range = aligned_range(address, size)?;
    if !permissions.is_valid() {
        return Err(ServiceError::InvalidInput);
    }
    let vmar = process.resolve_handle::<VmarObject>(vmar, Rights::MAP)?;
    let mut required = Rights::MAP;
    if permissions.contains(Access::Read) {
        required = required.union(Rights::READ);
    }
    if permissions.contains(Access::Write) {
        required = required.union(Rights::WRITE);
    }
    if permissions.contains(Access::Execute) {
        required = required.union(Rights::EXECUTE);
    }
    let vmo = process.resolve_handle::<VmoObject>(vmo, required)?;
    let maximum = if vmo.object().writable().is_some() {
        Permissions::read_write()
    } else {
        Permissions::read_execute()
    };
    if !permissions.is_subset_of(maximum) {
        return Err(ServiceError::InvalidInput);
    }
    let logical = vmar.object().address_space().logical();
    let prepared = if let Some(storage) = vmo.object().writable_clone() {
        storage
            .populate(object_offset, range.length())
            .map_err(|failure| MemoryObjectError::Vmo(failure.cause))?;
        logical
            .prepare_map_writable(
                vmar.object().token(),
                range,
                storage,
                object_offset,
                permissions,
                maximum,
            )
            .map_err(MachineError::Logical)?
    } else if let Some(storage) = vmo.object().executable_clone() {
        logical
            .prepare_map_executable(
                vmar.object().token(),
                range,
                storage,
                object_offset,
                permissions,
                maximum,
            )
            .map_err(MachineError::Logical)?
    } else {
        return Err(ServiceError::InvalidInput);
    };
    vmar.object()
        .address_space()
        .prepare_change(prepared)?
        .commit()?;
    Ok(())
}

pub(crate) fn protect(
    process: &Process,
    vmar: HandleValue,
    address: u64,
    size: u64,
    permissions: Permissions,
) -> Result<(), ServiceError> {
    if !permissions.is_valid() {
        return Err(ServiceError::InvalidInput);
    }
    let range = aligned_range(address, size)?;
    let vmar = process.resolve_handle::<VmarObject>(vmar, Rights::MAP)?;
    let prepared = vmar
        .object()
        .address_space()
        .logical()
        .prepare_protect(vmar.object().token(), range, permissions)
        .map_err(MachineError::Logical)?;
    vmar.object()
        .address_space()
        .prepare_change(prepared)?
        .commit()?;
    Ok(())
}

pub(crate) fn unmap(
    process: &Process,
    vmar: HandleValue,
    address: u64,
    size: u64,
) -> Result<(), ServiceError> {
    let range = aligned_range(address, size)?;
    let vmar = process.resolve_handle::<VmarObject>(vmar, Rights::MAP)?;
    let prepared = vmar
        .object()
        .address_space()
        .logical()
        .prepare_unmap(vmar.object().token(), range)
        .map_err(MachineError::Logical)?;
    vmar.object()
        .address_space()
        .prepare_change(prepared)?
        .commit()?;
    Ok(())
}

pub(crate) fn destroy(process: &Process, value: HandleValue) -> Result<(), ServiceError> {
    let vmar = process.resolve_handle::<VmarObject>(value, Rights::MAP)?;
    if vmar.object().token() == vmar.object().address_space().logical().root_vmar() {
        return Err(ServiceError::InvalidInput);
    }
    let consumption =
        process.prepare_handle_consumption(value, Rights::MAP, ObjectKind::VMAR, vmar.koid())?;
    match vmar
        .object()
        .address_space()
        .logical()
        .destroy_vmar(vmar.object().token())
    {
        Ok(()) => {
            consumption.commit().release();
            Ok(())
        }
        Err(error) => {
            consumption.rollback();
            Err(MachineError::Logical(error).into())
        }
    }
}

pub(crate) fn permissions(raw: u64) -> Option<Permissions> {
    let known = hyper::abi::native::HYPER_NATIVE_VMAR_PERMISSION_READ
        | hyper::abi::native::HYPER_NATIVE_VMAR_PERMISSION_WRITE
        | hyper::abi::native::HYPER_NATIVE_VMAR_PERMISSION_EXECUTE;
    if raw & !known != 0 {
        return None;
    }
    let mut permissions = Permissions::NONE;
    if raw & hyper::abi::native::HYPER_NATIVE_VMAR_PERMISSION_READ != 0 {
        permissions = permissions.union(Permissions::READ);
    }
    if raw & hyper::abi::native::HYPER_NATIVE_VMAR_PERMISSION_WRITE != 0 {
        permissions = permissions.union(Permissions::WRITE);
    }
    if raw & hyper::abi::native::HYPER_NATIVE_VMAR_PERMISSION_EXECUTE != 0 {
        permissions = permissions.union(Permissions::EXECUTE);
    }
    permissions.is_valid().then_some(permissions)
}

fn page_aligned_size(size: u64) -> Result<u64, ServiceError> {
    if size == 0 || size > MAXIMUM_VMO_SIZE {
        return Err(ServiceError::InvalidInput);
    }
    size.checked_add(PAGE_SIZE - 1)
        .map(|value| value & !(PAGE_SIZE - 1))
        .ok_or(ServiceError::InvalidInput)
}

fn aligned_range(address: u64, size: u64) -> Result<UserSlice, ServiceError> {
    if size == 0 || !address.is_multiple_of(PAGE_SIZE) || !size.is_multiple_of(PAGE_SIZE) {
        return Err(ServiceError::InvalidInput);
    }
    UserSlice::new(UserAddress::new(address), size).map_err(|_| ServiceError::InvalidInput)
}

fn transfer_length(slice: UserSlice) -> Result<usize, ServiceError> {
    let length = usize::try_from(slice.length()).map_err(|_| ServiceError::InvalidInput)?;
    if length > MAXIMUM_TRANSFER {
        return Err(ServiceError::InvalidInput);
    }
    Ok(length)
}

const fn writable_vmo_rights() -> Rights {
    Rights::DUPLICATE
        .union(Rights::TRANSFER)
        .union(Rights::INSPECT)
        .union(Rights::READ)
        .union(Rights::WRITE)
        .union(Rights::MAP)
}

const fn executable_vmo_rights() -> Rights {
    Rights::DUPLICATE
        .union(Rights::TRANSFER)
        .union(Rights::INSPECT)
        .union(Rights::READ)
        .union(Rights::EXECUTE)
        .union(Rights::MAP)
}

const fn vmar_rights() -> Rights {
    Rights::DUPLICATE
        .union(Rights::TRANSFER)
        .union(Rights::INSPECT)
        .union(Rights::MAP)
}
