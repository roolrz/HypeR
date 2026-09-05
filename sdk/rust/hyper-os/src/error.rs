// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use crate::Status;

/// Failure reported by the safe Native OS binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// The kernel rejected an otherwise well-formed Native operation.
    Status(Status),
    /// The installed kernel does not implement the SDK's required ABI.
    UnsupportedAbi { revision: u64, features: u64 },
    /// A trusted runtime or kernel response violated the Native ABI contract.
    InvalidResponse,
    /// The runtime did not provide a valid parsed startup record.
    InvalidStartup,
}

pub type Result<T> = core::result::Result<T, Error>;
