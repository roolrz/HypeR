// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;

#[derive(Debug, Parser)]
#[command(about = "Monitor Native CPU, memory and processes")]
pub struct Top {}

#[cfg(test)]
#[path = "../tests/cli.rs"]
mod tests;
