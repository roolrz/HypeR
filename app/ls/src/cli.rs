// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;

#[derive(Debug, Parser)]
#[command(about = "List a delegated directory")]
pub struct Ls {
    pub directory: Option<String>,
}

#[cfg(test)]
#[path = "../tests/cli.rs"]
mod tests;
