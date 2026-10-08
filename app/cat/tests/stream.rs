// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn blank_squeezing_and_nonblank_numbering_span_tiny_buffers_and_files() -> io::Result<()> {
    let mut lines = Lines::default();
    let mut output = Vec::new();
    let options = Options {
        number: true,
        number_nonblank: true,
        squeeze_blank: true,
    };
    lines.copy(
        io::BufReader::with_capacity(1, &b"\n\na"[..]),
        &mut output,
        options,
    )?;
    lines.copy(
        io::BufReader::with_capacity(1, &b"b\n\n\n\xff"[..]),
        &mut output,
        options,
    )?;
    assert_eq!(output, b"\n     1\tab\n\n     2\t\xff");
    Ok(())
}

#[test]
fn numbering_spans_files_and_handles_binary_bytes() -> io::Result<()> {
    let mut lines = Lines::default();
    let mut output = Vec::new();
    lines.copy(
        &b"one"[..],
        &mut output,
        Options {
            number: true,
            ..Options::default()
        },
    )?;
    lines.copy(
        &b"two\n\n\xff\n"[..],
        &mut output,
        Options {
            number: true,
            ..Options::default()
        },
    )?;
    assert_eq!(output, b"     1\tonetwo\n     2\t\n     3\t\xff\n");
    Ok(())
}
