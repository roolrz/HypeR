// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;

#[derive(Debug, Parser)]
#[command(about = "List Native processes and threads")]
pub struct Ps {
    #[arg(short = 'T', long)]
    pub threads: bool,
    #[command(flatten)]
    pub filter: hyper_tool_args::ProcessFilter,
    /// Print rows only, suitable for pipelines.
    #[arg(long)]
    pub no_headers: bool,
}

#[cfg(test)]
#[path = "../tests/cli.rs"]
mod tests;
