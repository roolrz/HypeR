// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::{Parser, ValueEnum};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum Sort {
    #[default]
    Name,
    Size,
}

#[derive(Debug, Parser)]
#[command(about = "List files with permission modes and readable sizes")]
pub struct Ls {
    /// Include hidden entries.
    #[arg(short = 'a', long)]
    pub all: bool,
    /// Print one name per line without metadata.
    #[arg(short = '1', long)]
    pub names_only: bool,
    /// Show exact byte counts instead of IEC units.
    #[arg(long)]
    pub bytes: bool,
    /// Sort by name or descending size.
    #[arg(long, value_enum, default_value = "name")]
    pub sort: Sort,
    /// Reverse the selected ordering.
    #[arg(short = 'r', long)]
    pub reverse: bool,
    /// Files or directories to list. Defaults to the working directory.
    pub paths: Vec<PathBuf>,
}
