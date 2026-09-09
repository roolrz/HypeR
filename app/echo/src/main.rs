// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! External echo using standard argument parsing and output.

use clap::Parser;
use std::io::{self, Write};

fn main() -> io::Result<()> {
    let args = hyper_echo::cli::Echo::parse();
    writeln!(io::stdout().lock(), "{}", args.words.join(" "))
}
