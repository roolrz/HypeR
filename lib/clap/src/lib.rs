// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Shared clap runtime for Native commands built with the same Rust toolchain.
//!
//! Applications retain their own argument declarations and derive expansions;
//! the parser, validation and help implementation live in this delivery crate.

extern crate hyper_rust_std as _;

#[doc(no_inline)]
pub use clap::*;
