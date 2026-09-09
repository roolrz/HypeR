// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;

#[derive(Debug, Parser)]
#[command(about = "Print arguments", disable_help_flag = true)]
pub struct Echo {
    // echo treats option-looking arguments as text, including --help and -n.
    #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
    pub words: Vec<String>,
}

#[cfg(test)]
#[path = "../tests/cli.rs"]
mod tests;
