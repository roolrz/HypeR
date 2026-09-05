// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Compiler-checked definitions for the HypeR Native ABI.
//!
//! This crate is dependency-free and usable from `no_std` kernels and native
//! applications. It describes machine-visible values only; syscall dispatch,
//! object policy, and language-level runtime interfaces belong to consumers.

#![no_std]

mod generated;

pub use generated::*;
