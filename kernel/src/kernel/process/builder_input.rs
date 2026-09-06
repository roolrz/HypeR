// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Bounded scalar-input policy shared by `ProcessBuilder` entry paths.

pub(crate) const MAX_ARGUMENTS: usize = 64;
pub(crate) const MAX_ENVIRONMENT: usize = 64;
const _: () = assert!(hyper::abi::native::HYPER_NATIVE_PROCESS_NAME_MAX_BYTES <= usize::MAX as u64);
pub(crate) const MAX_NAME_BYTES: usize =
    hyper::abi::native::HYPER_NATIVE_PROCESS_NAME_MAX_BYTES as usize;
pub(crate) const MAX_STRING_BYTES: usize = 4096;
pub(crate) const MAX_TOTAL_STRING_BYTES: usize = 16 * 1024;
pub(crate) const MAX_STARTUP_HANDLES: usize = 256;
pub(crate) const ABI_AFFINITY_WORDS: usize = 4;

/// Returns whether one argv element satisfies the wire-format contract.
///
/// Empty elements are valid. Only the argv vector itself must be nonempty.
pub(crate) fn valid_argument(value: &str) -> bool {
    valid_string(value)
}

/// Returns whether one environment entry is a bounded `NAME=VALUE` string.
///
/// `NAME` is nonempty and may not contain `=`. `VALUE` may be empty and may
/// contain further `=` bytes.
pub(crate) fn valid_environment(value: &str) -> bool {
    if !valid_string(value) {
        return false;
    }
    value
        .as_bytes()
        .iter()
        .position(|byte| *byte == b'=')
        .is_some_and(|separator| separator != 0)
}

pub(crate) fn valid_name(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_NAME_BYTES && !value.as_bytes().contains(&0)
}

fn valid_string(value: &str) -> bool {
    value.len() <= MAX_STRING_BYTES && !value.as_bytes().contains(&0)
}
