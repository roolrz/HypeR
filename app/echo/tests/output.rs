// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;
use clap::Parser;

#[test]
fn escapes_newline_suppression_and_early_stop() -> Result<(), Box<dyn std::error::Error>> {
    for (words, expected) in [
        (
            vec!["echo", "-n", "hello", "world"],
            b"hello world".as_slice(),
        ),
        (vec!["echo", "-e", r"a\nb\t\x41\0102"], b"a\nb\tAB\n"),
        (vec!["echo", "-e", r"one\ctwo", "three"], b"one"),
        (vec!["echo", "-e", "-E", r"a\nb"], b"a\\nb\n"),
        (vec!["echo", "-e", r"\q\xZ"], b"\\q\\xZ\n"),
        (vec!["echo", "--help", "-n"], b"--help -n\n"),
        (vec!["echo", "--", "-n"], b"-n\n"),
        (vec!["echo", "-neX"], b"-neX\n"),
        (vec!["echo", "-e", "text", "-n"], b"text -n\n"),
    ] {
        let mut output = Vec::new();
        write(&cli::Echo::try_parse_from(words)?, &mut output)?;
        assert_eq!(output, expected);
    }
    Ok(())
}
