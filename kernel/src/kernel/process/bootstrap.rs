// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Kernel-owned capabilities and opaque data delivered to userspace-loader.

use hyper::abi::native::{HyperNativeLoaderStartup, HyperNativeStartupHandle};

use crate::kernel::accounting::ResourceDomain;
use crate::kernel::authority::Rights;
use crate::kernel::capability::{HandleFlags, PreparedHandle};
use crate::kernel::ipc::{ByteChannel, ByteChannelError, PreparedByteMessage};
use crate::kernel::mm::user_space::{ExecutableProvenance, MemoryObjectError, VmoObject};
use crate::kernel::object::{ObjectPublication, diagnostics::PairedObject};
use crate::kernel::vfs::ExecutableSnapshot;

#[derive(Debug)]
pub(crate) enum Error {
    Channel(ByteChannelError),
    Memory(MemoryObjectError),
    Vfs(crate::kernel::vfs::VfsError),
    Process(super::ProcessError),
    Scheduler(
        #[expect(
            dead_code,
            reason = "Retains scheduler failure details for diagnostics"
        )]
        crate::kernel::task::scheduler::Error,
    ),
}

pub(crate) struct PreparedBootstrap {
    pub(crate) runtime_directory: PreparedHandle,
    pub(crate) executable: PreparedHandle,
    pub(crate) child_channel: PreparedHandle,
    pub(crate) parent_channel: PreparedHandle,
}

/// All storage is sponsored before publication. Dropping this owner closes
/// both unpublished endpoints and releases the immutable executable snapshot.
pub(crate) fn prepare(
    domain: &ResourceDomain,
    snapshot: &ExecutableSnapshot,
    header: HyperNativeLoaderStartup,
    records: impl Iterator<Item = HyperNativeStartupHandle>,
    data: &[u8],
) -> Result<PreparedBootstrap, Error> {
    let pin = crate::kernel::task::scheduler::preempt_disable().map_err(Error::Scheduler)?;
    let executable = snapshot
        .storage()
        .try_executable(&ExecutableProvenance::for_native_image_loader(), &pin);
    crate::kernel::task::scheduler::preempt_enable_without_reschedule(pin)
        .map_err(Error::Scheduler)?;
    let executable = VmoObject::from_executable(
        executable
            .map_err(MemoryObjectError::Vmo)
            .map_err(Error::Memory)?,
        domain,
    )
    .map_err(Error::Memory)?;
    let executable = prepare_handle(
        executable,
        Rights::READ.union(Rights::MAP).union(Rights::EXECUTE),
    )?;

    let mut message = PreparedByteMessage::try_new(
        domain,
        core::mem::size_of::<HyperNativeLoaderStartup>()
            + header.handle_count as usize * 16
            + data.len(),
    )
    .map_err(Error::Channel)?;
    let bytes = message.bytes_mut();
    // Explicit wire encoding avoids padding disclosure and unaligned access.
    for (index, value) in [
        header.size,
        header.handle_count,
        header.data_size,
        header.flags,
    ]
    .into_iter()
    .enumerate()
    {
        bytes[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    for (index, value) in [
        header.executable,
        header.executable_size,
        header.root_vmar,
        header.stack_vmar,
        header.stack_base,
        header.stack_size,
        header.loader_base,
        header.loader_size,
        header.runtime_directory,
    ]
    .into_iter()
    .enumerate()
    {
        bytes[16 + index * 8..24 + index * 8].copy_from_slice(&value.to_le_bytes());
    }
    let mut offset = 88;
    for record in records {
        bytes[offset..offset + 4].copy_from_slice(&record.purpose.to_le_bytes());
        bytes[offset + 4..offset + 8].copy_from_slice(&record.flags.to_le_bytes());
        bytes[offset + 8..offset + 16].copy_from_slice(&record.handle.to_le_bytes());
        offset += 16;
    }
    bytes[offset..].copy_from_slice(data);
    let (parent, child) = ByteChannel::try_pair(domain).map_err(Error::Channel)?;
    let parent = ObjectPublication::try_new(parent).map_err(|e| Error::Process(e.into()))?;
    let child = ObjectPublication::try_new(child).map_err(|e| Error::Process(e.into()))?;
    parent.object().bind_peer_identity(child.koid());
    child.object().bind_peer_identity(parent.koid());
    parent
        .object()
        .prepare_write(&message)
        .map_err(Error::Channel)?
        .publish(message);
    let parent_channel = PreparedHandle::try_from_new_object(
        parent,
        Rights::READ.union(Rights::WAIT),
        HandleFlags::NONE,
    )
    .map_err(|e| Error::Process(e.into()))?;
    let child_channel = PreparedHandle::try_from_new_object(
        child,
        Rights::READ.union(Rights::WRITE),
        HandleFlags::NONE,
    )
    .map_err(|e| Error::Process(e.into()))?;
    let runtime_directory = prepare_handle(
        super::loader::runtime_directory(domain).map_err(Error::Vfs)?,
        Rights::READ.union(Rights::EXECUTE),
    )?;
    Ok(PreparedBootstrap {
        executable,
        child_channel,
        parent_channel,
        runtime_directory,
    })
}

fn prepare_handle<T: crate::kernel::object::UserExportableObject>(
    object: T,
    rights: Rights,
) -> Result<PreparedHandle, Error> {
    let publication = ObjectPublication::try_new(object).map_err(|e| Error::Process(e.into()))?;
    PreparedHandle::try_from_new_object(publication, rights, HandleFlags::NONE)
        .map_err(|e| Error::Process(e.into()))
}
