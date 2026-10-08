// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;

#[derive(Debug, Parser)]
#[command(about = "Print arguments", disable_help_flag = true)]
pub struct Echo {
    /// Omit the trailing newline.
    #[arg(short = 'n')]
    pub no_newline: bool,
    /// Interpret backslash escapes.
    #[arg(short = 'e', overrides_with = "literal")]
    pub escapes: bool,
    /// Print backslashes literally (the default).
    #[arg(short = 'E', overrides_with = "escapes")]
    pub literal: bool,
    // Unknown options are text; the first operand ends option parsing.
    #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
    pub words: Vec<String>,
}

#[cfg(test)]
#[path = "../tests/cli.rs"]
mod tests;
