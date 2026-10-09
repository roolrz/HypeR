// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::{Parser, ValueEnum};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum Sort {
    #[default]
    Name,
    Size,
    Time,
}

#[derive(Debug, Parser)]
#[command(about = "List files with permission modes and readable sizes")]
pub struct Ls {
    /// List directory operands themselves instead of their contents.
    #[arg(short = 'd', long)]
    pub directory: bool,
    /// Include hidden entries.
    #[arg(short = 'a', long)]
    pub all: bool,
    /// Print one name per line without metadata.
    #[arg(short = '1', long)]
    pub names_only: bool,
    /// Show exact byte counts instead of IEC units.
    #[arg(long)]
    pub bytes: bool,
    /// Sort by name, descending size, or newest modification time.
    #[arg(long, value_enum, default_value = "name")]
    pub sort: Sort,
    /// Sort newest modification time first.
    #[arg(short = 't', conflicts_with = "sort")]
    pub newest: bool,
    /// Sort largest size first.
    #[arg(short = 'S', conflicts_with_all = ["sort", "newest"])]
    pub largest: bool,
    /// Reverse the selected ordering.
    #[arg(short = 'r', long)]
    pub reverse: bool,
    /// Files or directories to list. Defaults to the working directory.
    pub paths: Vec<PathBuf>,
}
