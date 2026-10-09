// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::{Parser, ValueEnum};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Sort {
    Cpu,
    Name,
    Koid,
}

#[derive(Debug, Parser)]
#[command(about = "Monitor Native CPU, memory and processes")]
pub struct Top {
    /// Emit plain snapshots without terminal escape sequences or keyboard input.
    #[arg(short = 'b', long)]
    pub batch: bool,
    /// Exit after this many snapshots.
    #[arg(short = 'n', long)]
    pub iterations: Option<std::num::NonZeroU32>,
    /// Refresh period in seconds (0.1 to 60).
    #[arg(short = 'd', long, default_value = "1", value_parser = hyper_tool_args::parse_interval)]
    pub delay: f64,
    #[command(flatten)]
    pub filter: hyper_tool_args::ProcessFilter,
    /// Order process rows; ties are resolved by KOID.
    #[arg(long, value_enum, default_value = "cpu")]
    pub sort: Sort,
    /// Show at most this many matching process rows (summary remains system-wide).
    #[arg(short = 'l', long)]
    pub limit: Option<std::num::NonZeroUsize>,
}

#[cfg(test)]
#[path = "../tests/cli.rs"]
mod tests;
