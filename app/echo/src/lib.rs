// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

pub mod cli;

use std::io::{self, Write};

pub fn write(args: &cli::Echo, output: &mut impl Write) -> io::Result<()> {
    for (index, word) in args.words.iter().enumerate() {
        if index != 0 {
            output.write_all(b" ")?;
        }
        if args.escapes {
            if !escaped(word.as_bytes(), output)? {
                return Ok(());
            }
        } else {
            output.write_all(word.as_bytes())?;
        }
    }
    if !args.no_newline {
        output.write_all(b"\n")?;
    }
    Ok(())
}

/// Returns false for `\c`, which suppresses all remaining output, including the newline.
fn escaped(bytes: &[u8], output: &mut impl Write) -> io::Result<bool> {
    let mut cursor = 0;
    while let Some(&byte) = bytes.get(cursor) {
        cursor += 1;
        if byte != b'\\' || cursor == bytes.len() {
            output.write_all(&[byte])?;
            continue;
        }
        let escape = bytes[cursor];
        cursor += 1;
        let decoded = match escape {
            b'c' => return Ok(false),
            b'a' => 7,
            b'b' => 8,
            b'e' => 27,
            b'f' => 12,
            b'n' => b'\n',
            b'r' => b'\r',
            b't' => b'\t',
            b'v' => 11,
            b'\\' => b'\\',
            b'0' | b'x' => {
                let radix = if escape == b'0' { 8 } else { 16 };
                let limit = if escape == b'0' { 3 } else { 2 };
                let start = cursor;
                let mut value = 0_u32;
                while cursor - start < limit {
                    let Some(digit) = bytes
                        .get(cursor)
                        .and_then(|byte| char::from(*byte).to_digit(radix))
                    else {
                        break;
                    };
                    value = value * radix + digit;
                    cursor += 1;
                }
                if cursor == start && escape == b'x' {
                    output.write_all(b"\\x")?;
                    continue;
                }
                value as u8
            }
            _ => {
                output.write_all(&[b'\\', escape])?;
                continue;
            }
        };
        output.write_all(&[decoded])?;
    }
    Ok(true)
}

#[cfg(test)]
#[path = "../tests/output.rs"]
mod tests;
