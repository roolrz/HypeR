// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! The single Rust standard-library owner for Native application shared libraries.
//!
//! Every Rust DSO links this crate so unrelated libraries can coexist in one
//! process without embedding competing copies of std and its dependencies.
//! It uses the installed SDK's std port and must ship with the same-build DSOs.

#[doc(no_inline)]
pub extern crate std;
