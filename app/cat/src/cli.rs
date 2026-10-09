// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(about = "Concatenate files to standard output; '-' reads standard input")]
pub struct Cat {
    /// Number all output lines.
    #[arg(short = 'n', long)]
    pub number: bool,
    /// Number nonempty lines; overrides -n.
    #[arg(short = 'b', long)]
    pub number_nonblank: bool,
    /// Suppress repeated empty lines.
    #[arg(short = 's', long)]
    pub squeeze_blank: bool,
    /// Files to read, in order. Defaults to standard input.
    pub files: Vec<PathBuf>,
}
