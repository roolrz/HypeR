// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;

#[derive(Debug, Parser)]
#[command(about = "List Native processes and threads")]
pub struct Ps {
    #[arg(short = 'T', long)]
    pub threads: bool,
    /// Select one process by KOID.
    #[arg(short = 'p', long)]
    pub process: Option<std::num::NonZeroU64>,
    /// Show process names containing this text.
    #[arg(long)]
    pub name: Option<String>,
}

#[cfg(test)]
#[path = "../tests/cli.rs"]
mod tests;
