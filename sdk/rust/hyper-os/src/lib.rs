// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Safe, capability-oriented operating-system bindings for `HypeR` Native apps.
//!
//! This crate is independent of any particular application. It is also the
//! semantic substrate intended for a future Rust standard-library port; it
//! does not mirror unstable `std::sys` implementation details.

#![no_std]

mod abi;
pub mod capability_channel;
pub mod channel;
pub mod console;
mod error;
pub mod fs;
pub mod handle;
pub mod inspect;
pub mod memory;
pub mod startup;
mod status;
pub mod task;
pub mod time;
pub mod virtual_serial;
pub mod vm;
pub mod wait;

pub use abi::require_core_abi;
pub use error::{Error, Result};
pub use handle::{HandleRef, OwnedHandle, Rights, RightsOffer};
pub use status::Status;

/// Absolute deadline which never expires.
pub const DEADLINE_INFINITE: u64 = hyper_abi::HYPER_NATIVE_DEADLINE_INFINITE;

/// Validates one extensible-info result and preserves the kernel record size.
///
/// Decoders must keep the returned size beside the zero-initialized record and
/// gate any field beyond the published minimum prefix on that size. Every
/// current record has `MIN_SIZE == size_of::<Record>()`, so its current fields
/// are all mandatory and need no per-field gates yet.
fn validate_info_result(result: hyper_sys::CallResult, minimum_size: usize) -> Result<usize> {
    Status::from_raw(result.status).into_result()?;
    let supported_size = usize::try_from(result.value0).map_err(|_| Error::InvalidResponse)?;
    if result.value1 != 0
        || supported_size < minimum_size
        || supported_size > hyper_abi::HYPER_NATIVE_EXTENSIBLE_RECORD_MAX_BYTES as usize
    {
        return Err(Error::InvalidResponse);
    }
    Ok(supported_size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_result_reports_the_kernel_record_size() {
        let supported = validate_info_result(
            hyper_sys::CallResult {
                status: hyper_abi::HYPER_NATIVE_STATUS_OK,
                value0: 64,
                value1: 0,
            },
            32,
        );
        assert_eq!(supported, Ok(64));
    }

    #[test]
    fn info_result_rejects_malformed_size_metadata() {
        let call = |value0, value1| {
            validate_info_result(
                hyper_sys::CallResult {
                    status: hyper_abi::HYPER_NATIVE_STATUS_OK,
                    value0,
                    value1,
                },
                32,
            )
        };
        assert_eq!(call(31, 0), Err(Error::InvalidResponse));
        assert_eq!(call(32, 1), Err(Error::InvalidResponse));
        assert_eq!(
            call(hyper_abi::HYPER_NATIVE_EXTENSIBLE_RECORD_MAX_BYTES + 1, 0,),
            Err(Error::InvalidResponse)
        );
    }
}
