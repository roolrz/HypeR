// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;

#[derive(Debug, Parser)]
#[command(about = "Display physical memory usage")]
pub struct Free {
    /// Report exact bytes instead of rounded-down MiB values.
    #[arg(short = 'b', long)]
    pub bytes: bool,
}

#[cfg(test)]
#[path = "../tests/cli.rs"]
mod tests;
