// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Common filesystem worker lifecycle and ordered range/metadata dispatch.
//! Drivers implement media semantics; capability ownership, request validation,
//! shared-memory ownership, mount publication and parent failure live here.
use hyper_filesystem::protocol::{self, Entry, Error, Operation, Request, Response};
use hyper_os::{
    handle::ByteChannelObject,
    startup::Startup,
    wait::{self, ObjectSignals, WaitItem},
};

pub trait Volume {
    fn stat(&mut self, path: &str) -> Result<Entry, Error>;
    /// Enumerate ordinary children; dot entries are namespace traversal, not children.
    fn entry(&mut self, path: &str, index: usize) -> Result<Option<Entry>, Error>;
    fn read(&mut self, path: &str, offset: u64, output: &mut [u8]) -> Result<usize, Error>;
    fn write(&mut self, path: &str, offset: u64, input: &[u8]) -> Result<usize, Error>;
    fn create(&mut self, path: &str, directory: bool) -> Result<(), Error>;
    fn remove(&mut self, path: &str) -> Result<(), Error>;
    fn rename(&mut self, path: &str, target: &str) -> Result<(), Error>;
    fn resize(&mut self, path: &str, length: u64) -> Result<(), Error>;
    fn sync(&mut self) -> Result<(), Error>;
    fn set_times(
        &mut self,
        path: &str,
        accessed: Option<hyper_filesystem::Timestamp>,
        modified: Option<hyper_filesystem::Timestamp>,
    ) -> Result<(), Error>;
}
/// Returns on parent death or transport disconnect. Outstanding metadata/data
/// have one owner; the server cannot start another transaction before reply.
pub fn serve(
    startup: &mut Startup<'_>,
    mount_path: &str,
    mut volume: impl Volume,
) -> Result<(), String> {
    let ready = startup
        .take(hyper_service::filesystem::READY)
        .map_err(show)?;
    let owner = startup
        .take(hyper_service::filesystem::OWNER)
        .map_err(show)?;
    let root = startup
        .borrow(hyper_os::startup::ROOT_DIRECTORY)
        .map_err(show)?;
    let (client, server) = hyper_os::channel::create_pair().map_err(show)?;
    let mut buffer = hyper_os::filesystem::SharedBuffer::create(
        startup.borrow(hyper_os::startup::ROOT_VMAR).map_err(show)?,
        protocol::DATA_BYTES,
    )
    .map_err(show)?;
    // Validate the driver's root before making any namespace visible.
    if volume.stat("").map_err(show)?.kind != protocol::DIRECTORY {
        return Err("filesystem root is not a directory".into());
    }
    hyper_os::filesystem::mount(client.as_handle_ref(), buffer.memory(), root, mount_path)
        .map_err(show)?;
    ready
        .as_byte_channel()
        .send(hyper_service::filesystem::READY_MESSAGE)
        .map_err(show)?;
    drop(ready);
    println!("HypeR filesystem: mount ready");
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(protocol::REQUEST_BYTES)
        .map_err(show)?;
    bytes.resize(protocol::REQUEST_BYTES, 0);
    let mut reply = [0; protocol::RESPONSE_BYTES];
    let mut sequence = 0;
    loop {
        let observed = wait::wait_many(
            &[
                WaitItem::new(
                    owner.as_handle_ref(),
                    ObjectSignals::<ByteChannelObject>::PEER_CLOSED,
                ),
                WaitItem::new(
                    server.as_handle_ref(),
                    ObjectSignals::<ByteChannelObject>::READABLE
                        .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED),
                ),
            ],
            hyper_os::DEADLINE_INFINITE,
        )
        .map_err(|error| format!("wait for filesystem request: {error:?}"))?;
        if observed.index == 0
            || ObjectSignals::<ByteChannelObject>::PEER_CLOSED.is_present_in(observed.observed)
        {
            return Ok(());
        }
        // Readiness is a hint, not a reservation of the next message. Rearm
        // both wait sources if receive observes a transient empty queue.
        let length = match server.as_byte_channel().try_receive(&mut bytes) {
            Ok(length) => length,
            Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK)) => continue,
            Err(error) => return Err(format!("receive filesystem request: {error:?}")),
        };
        let request = Request::decode(&bytes[..length]).ok_or("malformed filesystem request")?;
        if request.sequence <= sequence {
            return Err("filesystem sequence mismatch".into());
        }
        sequence = request.sequence;
        // SAFETY: Receiving this request transfers the fixed shared range to
        // this sole server. Kernel waits for reply; on timeout it permanently
        // retires the generation and never accesses/reuses the range again.
        let result = dispatch(
            &mut volume,
            request,
            unsafe { buffer.bytes_mut() },
            &mut reply,
        )?;
        server
            .as_byte_channel()
            .send(&reply)
            .map_err(|error| format!("send filesystem reply: {error:?}"))?;
        if result.is_some_and(Error::terminal) {
            return Err("filesystem backend failed".into());
        }
    }
}
fn dispatch(
    volume: &mut impl Volume,
    request: Request<'_>,
    data: &mut [u8],
    reply: &mut [u8; protocol::RESPONSE_BYTES],
) -> Result<Option<Error>, String> {
    let mut entry = Entry::empty();
    let result = (|| match request.operation {
        Operation::Stat => {
            entry = volume.stat(request.path)?;
            Ok(0)
        }
        Operation::Entry => match volume.entry(
            request.path,
            usize::try_from(request.offset).map_err(|_| Error::InvalidInput)?,
        )? {
            Some(value) => {
                entry = value;
                Ok(1)
            }
            None => Ok(0),
        },
        Operation::Read => {
            let length = usize::try_from(request.length).map_err(|_| Error::InvalidInput)?;
            volume
                .read(
                    request.path,
                    request.offset,
                    data.get_mut(..length).ok_or(Error::InvalidInput)?,
                )
                .map(|n| n as u64)
        }
        Operation::Write => {
            let length = usize::try_from(request.length).map_err(|_| Error::InvalidInput)?;
            volume
                .write(
                    request.path,
                    request.offset,
                    data.get(..length).ok_or(Error::InvalidInput)?,
                )
                .map(|n| n as u64)
        }
        Operation::Create if request.length <= 1 => {
            volume.create(request.path, request.length != 0).map(|_| 0)
        }
        Operation::Create => Err(Error::InvalidInput),
        Operation::Remove => volume.remove(request.path).map(|_| 0),
        Operation::Rename => volume.rename(request.path, request.target).map(|_| 0),
        Operation::Resize => volume.resize(request.path, request.length).map(|_| 0),
        Operation::Sync => volume.sync().map(|_| 0),
        Operation::SetMetadata => volume
            .set_times(request.path, request.accessed, request.modified)
            .map(|_| 0),
    })();
    let failure = result.err();
    Response {
        sequence: request.sequence,
        operation: request.operation,
        result,
        entry,
    }
    .encode(reply)
    .ok_or_else(|| format!("invalid filesystem {:?} response", request.operation))?;
    Ok(failure)
}
fn show(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}
