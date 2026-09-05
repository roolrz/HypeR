// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Safe, capability-oriented operating-system bindings for `HypeR` Native apps.
//!
//! This crate is independent of any particular application. It is also the
//! semantic substrate intended for a future Rust standard-library port; it
//! does not mirror unstable `std::sys` implementation details.

#![no_std]

mod abi;
pub mod console;
mod error;
pub mod startup;
mod status;

pub use abi::require_core_abi;
pub use error::{Error, Result};
pub use status::Status;
