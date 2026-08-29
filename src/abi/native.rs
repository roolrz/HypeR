// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Experimental `HypeR` Native syscall values and owned entry payloads.

include!("../../abi/native/experimental.rs");

/// Architecture-neutral copy of one Native syscall request.
///
/// Architecture entry constructs this value after validating the active user
/// context, then ends every borrow of its private trap frame before publication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeInvocation {
    number: u64,
    arguments: [u64; HYPER_EXPERIMENTAL_SYSCALL_ARGUMENT_REGISTERS],
    call_site: u64,
}

impl NativeInvocation {
    pub const fn new(
        number: u64,
        arguments: [u64; HYPER_EXPERIMENTAL_SYSCALL_ARGUMENT_REGISTERS],
        call_site: u64,
    ) -> Self {
        Self {
            number,
            arguments,
            call_site,
        }
    }

    pub const fn number(&self) -> u64 {
        self.number
    }

    pub const fn arguments(&self) -> &[u64; HYPER_EXPERIMENTAL_SYSCALL_ARGUMENT_REGISTERS] {
        &self.arguments
    }

    pub const fn call_site(&self) -> u64 {
        self.call_site
    }
}

/// Native status and auxiliary result words ready for architecture encoding.
///
/// A nonzero status clears both auxiliary values so failure never exposes
/// stale register contents or provisional capability values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeResult {
    status: HyperExperimentalStatus,
    values: [u64; HYPER_EXPERIMENTAL_SYSCALL_RESULT_REGISTERS],
}

impl NativeResult {
    pub const fn new(
        status: HyperExperimentalStatus,
        values: [u64; HYPER_EXPERIMENTAL_SYSCALL_RESULT_REGISTERS],
    ) -> Self {
        Self {
            status,
            values: if status == 0 {
                values
            } else {
                [0; HYPER_EXPERIMENTAL_SYSCALL_RESULT_REGISTERS]
            },
        }
    }

    pub const fn status(&self) -> HyperExperimentalStatus {
        self.status
    }

    pub const fn values(&self) -> &[u64; HYPER_EXPERIMENTAL_SYSCALL_RESULT_REGISTERS] {
        &self.values
    }
}
