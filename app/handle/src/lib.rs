// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

#[cfg(target_os = "hyper")]
extern crate hyper_tool_args_shared as hyper_tool_args;

pub mod cli;
mod details;
pub mod output;
pub mod query;
mod summary;
