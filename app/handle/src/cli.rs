// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;

#[derive(Debug, Parser)]
#[command(about = "Inspect Native kernel objects or a process's handles")]
#[command(group(clap::ArgGroup::new("selection").required(true).args(["objects", "process"])))]
pub struct Handle {
    #[arg(long)]
    pub objects: bool,
    pub process: Option<std::num::NonZeroU64>,
    /// Show only this object kind (as printed in KIND).
    #[arg(long)]
    pub kind: Option<String>,
}

#[cfg(test)]
#[path = "../tests/cli.rs"]
mod tests;
