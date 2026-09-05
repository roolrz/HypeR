// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! `HypeR` Native syscall values and owned entry payloads.

pub use hyper_abi::*;

/// Architecture-neutral copy of one Native syscall request.
///
/// Architecture entry constructs this value after validating the active user
/// context, then ends every borrow of its private trap frame before publication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeInvocation {
    number: u64,
    arguments: [u64; HYPER_NATIVE_SYSCALL_ARGUMENT_REGISTERS],
    call_site: u64,
}

impl NativeInvocation {
    pub const fn new(
        number: u64,
        arguments: [u64; HYPER_NATIVE_SYSCALL_ARGUMENT_REGISTERS],
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

    pub const fn arguments(&self) -> &[u64; HYPER_NATIVE_SYSCALL_ARGUMENT_REGISTERS] {
        &self.arguments
    }

    pub const fn call_site(&self) -> u64 {
        self.call_site
    }
}

/// Native status and auxiliary result words ready for architecture encoding.
///
/// The default constructor clears auxiliary values on failure so it cannot
/// expose stale register contents or provisional capability values. A syscall
/// whose schema explicitly defines failure results uses [`Self::for_syscall`]
/// to retain only those declared words.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeResult {
    status: HyperNativeStatus,
    values: [u64; HYPER_NATIVE_SYSCALL_RESULT_REGISTERS],
}

impl NativeResult {
    pub const fn new(
        status: HyperNativeStatus,
        values: [u64; HYPER_NATIVE_SYSCALL_RESULT_REGISTERS],
    ) -> Self {
        Self {
            status,
            values: if status == 0 {
                values
            } else {
                [0; HYPER_NATIVE_SYSCALL_RESULT_REGISTERS]
            },
        }
    }

    /// Constructs a result using the schema-defined failure-result mask.
    ///
    /// This is the only path for returning auxiliary words with a nonzero
    /// status. Undeclared words are cleared even if the handler supplied them.
    pub const fn for_syscall(
        syscall_number: u64,
        status: HyperNativeStatus,
        values: [u64; HYPER_NATIVE_SYSCALL_RESULT_REGISTERS],
    ) -> Self {
        if status == HYPER_NATIVE_STATUS_OK {
            return Self { status, values };
        }
        let mask = hyper_native_failure_result_mask(syscall_number, status);
        let mut retained = [0; HYPER_NATIVE_SYSCALL_RESULT_REGISTERS];
        let mut index = 0usize;
        while index < HYPER_NATIVE_SYSCALL_RESULT_REGISTERS {
            if mask & (1u64 << index) != 0 {
                retained[index] = values[index];
            }
            index += 1;
        }
        Self {
            status,
            values: retained,
        }
    }

    pub const fn status(&self) -> HyperNativeStatus {
        self.status
    }

    pub const fn values(&self) -> &[u64; HYPER_NATIVE_SYSCALL_RESULT_REGISTERS] {
        &self.values
    }
}
