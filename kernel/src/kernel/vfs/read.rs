// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! File-data-cache integration for immutable VFS reads.

use alloc::vec::Vec;
use core::num::NonZeroU64;

use hyper::mm::PAGE_SIZE;

use crate::kernel::io_cache::{CacheAccess, CacheKey, FilePageIndex, NodeIdentity};

use super::objects::{Error, FileObject};
use super::read_contract::{ReadPlan, ReadProgress};

const PAGE_BYTES: usize = PAGE_SIZE as usize;

/// Completely initialized immutable contents of one cached file page.
pub(super) struct FilePage {
    bytes: Vec<u8>,
    valid: usize,
}

pub(super) fn cached(
    file: &FileObject,
    mut offset: u64,
    destination: &mut [u8],
) -> Result<usize, Error> {
    let mut completed = 0usize;
    let file_len = file.len();
    while completed < destination.len() {
        let remaining = destination.get_mut(completed..).ok_or(Error::InvalidPath)?;
        let Some(plan) =
            ReadPlan::next(offset, file_len, remaining.len()).map_err(map_contract_error)?
        else {
            break;
        };
        let output = remaining
            .get_mut(..plan.output_len())
            .ok_or(Error::InvalidPath)?;
        let actual = read_page(file, plan, output)?;
        let progress = plan.validate_read(actual).map_err(map_contract_error)?;
        completed = completed.checked_add(actual).ok_or(Error::InvalidPath)?;
        offset = offset
            .checked_add(u64::try_from(actual).map_err(|_| Error::InvalidPath)?)
            .ok_or(Error::InvalidPath)?;
        if progress == ReadProgress::Partial {
            break;
        }
    }
    Ok(completed)
}

fn read_page(file: &FileObject, plan: ReadPlan, destination: &mut [u8]) -> Result<usize, Error> {
    let filesystem = file.location().mount().filesystem();
    let node = file.location().node().get();
    let node = NonZeroU64::new(node).ok_or(Error::NotRegularFile)?;
    let key = CacheKey::new(
        filesystem.cache_generation(),
        NodeIdentity::new(node),
        FilePageIndex::new(plan.page_index()),
    );
    match file.cache().access(key).map_err(Error::Cache)? {
        CacheAccess::Hit(page) => copy_cached(page.value(), plan.within_page(), destination),
        CacheAccess::Load(reservation) => {
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(PAGE_BYTES)
                .map_err(|_| Error::Allocation)?;
            bytes.resize(PAGE_BYTES, 0);
            let valid = file.read_uncached(plan.page_offset(), &mut bytes)?;
            plan.validate_fill(valid, bytes.len())
                .map_err(map_contract_error)?;
            let page = reservation
                .publish(FilePage { bytes, valid })
                .map_err(|failure| Error::Cache(failure.cause()))?;
            copy_cached(page.value(), plan.within_page(), destination)
        }
        CacheAccess::LoadInProgress | CacheAccess::CapacityBusy => {
            // RamFs is synchronously resident, so a contending nonblocking
            // reader can bypass cache publication without waiting on a loader.
            // A future blocking backend will use a scheduler-aware completion
            // contract rather than spinning or holding this syscall open.
            file.read_uncached(plan.offset(), destination)
        }
    }
}

const fn map_contract_error(error: super::read_contract::Error) -> Error {
    match error {
        super::read_contract::Error::ArithmeticOverflow => Error::InvalidPath,
        super::read_contract::Error::InvalidBackendResult => {
            Error::Backend(super::instance::Error::InvalidBackendResult)
        }
    }
}

fn copy_cached(
    page: &FilePage,
    within_page: usize,
    destination: &mut [u8],
) -> Result<usize, Error> {
    let Some(valid) = page.bytes.get(..page.valid) else {
        return Err(Error::Cache(crate::kernel::io_cache::CacheError::Invariant));
    };
    let Some(source) = valid.get(within_page..) else {
        return Ok(0);
    };
    let count = source.len().min(destination.len());
    let source = source.get(..count).ok_or(Error::InvalidPath)?;
    let output = destination.get_mut(..count).ok_or(Error::InvalidPath)?;
    output.copy_from_slice(source);
    Ok(count)
}
