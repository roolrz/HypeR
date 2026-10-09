// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Deterministic media substitute, exercising the production IPC server.
use hyper_filesystem::{
    Timestamp,
    protocol::{self, Entry, Error},
};
use hyper_os::handle::{ByteChannelObject, OwnedHandle};
pub(super) struct FixtureVolume(pub(super) OwnedHandle<ByteChannelObject>);
impl hyper_fs_service::Volume for FixtureVolume {
    fn stat(&mut self, path: &str) -> Result<Entry, Error> {
        let mut entry = Entry::empty();
        match path {
            "" => {}
            "cached" | "blocked" | "delayed" | "max-size" => {
                entry.name[..path.len()].copy_from_slice(path.as_bytes());
                entry.name_len = path.len();
                entry.kind = protocol::FILE;
                entry.size = if path == "max-size" { u64::MAX } else { 8192 };
            }
            _ => return Err(Error::Missing),
        }
        Ok(entry)
    }
    fn read(&mut self, path: &str, offset: u64, output: &mut [u8]) -> Result<usize, Error> {
        self.stat(path)?;
        if path == "blocked" {
            self.0
                .as_byte_channel()
                .send(b"blocked")
                .map_err(|_| Error::Closed)?;
            // The supervisor kills the actual process in this bounded wait.
            std::thread::sleep(std::time::Duration::from_secs(60));
            return Err(Error::Io);
        }
        if path == "delayed" {
            self.0
                .as_byte_channel()
                .send(b"delayed")
                .map_err(|_| Error::Closed)?;
            super::receive(&self.0, b"release").map_err(|_| Error::Closed)?;
        } else {
            self.0
                .as_byte_channel()
                .send(b"read")
                .map_err(|_| Error::Closed)?;
        }
        let length = output.len().min(8192u64.saturating_sub(offset) as usize);
        output[..length].fill(0x5a);
        Ok(length)
    }
    fn entry(&mut self, _: &str, _: usize) -> Result<Option<Entry>, Error> {
        Err(Error::Unsupported)
    }
    fn write(&mut self, _: &str, _: u64, _: &[u8]) -> Result<usize, Error> {
        self.0
            .as_byte_channel()
            .send(b"write")
            .map_err(|_| Error::Closed)?;
        Err(Error::Unsupported)
    }
    fn create(&mut self, _: &str, _: bool) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
    fn remove(&mut self, _: &str) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
    fn rename(&mut self, _: &str, _: &str) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
    fn resize(&mut self, _: &str, _: u64) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
    fn sync(&mut self) -> Result<(), Error> {
        Ok(())
    }
    fn set_times(
        &mut self,
        _: &str,
        _: Option<Timestamp>,
        _: Option<Timestamp>,
    ) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
}
