// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Synchronous, sleepable access to an exclusively owned logical block volume.

pub const SECTOR_SIZE: usize = 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidRange,
    ReadOnly,
    Disconnected,
    Io,
    Unsupported,
    Exhausted,
    Corrupt,
}

/// A volume, not a physical disk or an implicitly selected partition.
///
/// Calls may sleep. Owners must serialize access with a sleepable lock, never
/// an IRQ mask or spin lock. Successful transfers complete the entire slice;
/// an error may have partially modified the medium. `flush` must wait for all
/// earlier writes to become durable, or return an error. Dropping a device
/// must not perform I/O. Geometry remains fixed until the device is retired.
/// This revision admits 512-byte logical sectors only.
pub trait BlockDevice {
    fn sector_count(&self) -> u64;
    /// Immutable admission policy for this exclusive device lifetime.
    fn is_read_only(&self) -> bool {
        false
    }
    fn read_sectors(&mut self, first: u64, output: &mut [u8]) -> Result<(), Error>;
    fn write_sectors(&mut self, first: u64, input: &[u8]) -> Result<(), Error>;
    fn flush(&mut self) -> Result<(), Error>;
}

/// Validate before issuing any transfer, including a zero-length request.
pub fn validate_range(sector_count: u64, first: u64, bytes: usize) -> Result<(), Error> {
    if !bytes.is_multiple_of(SECTOR_SIZE) {
        return Err(Error::InvalidRange);
    }
    let sectors = u64::try_from(bytes / SECTOR_SIZE).map_err(|_| Error::InvalidRange)?;
    if first
        .checked_add(sectors)
        .is_none_or(|end| end > sector_count)
    {
        return Err(Error::InvalidRange);
    }
    Ok(())
}
