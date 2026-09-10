// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn numbering_spans_files_and_handles_binary_bytes() -> io::Result<()> {
    let mut lines = Lines::default();
    let mut output = Vec::new();
    lines.copy(&b"one"[..], &mut output, true)?;
    lines.copy(&b"two\n\n\xff\n"[..], &mut output, true)?;
    assert_eq!(output, b"     1\tonetwo\n     2\t\n     3\t\xff\n");
    Ok(())
}
