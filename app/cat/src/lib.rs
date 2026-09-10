// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

pub mod cli;

use std::io::{self, BufRead, Write};

/// Keep line numbering continuous across file and buffer boundaries.
#[derive(Default)]
pub struct Lines {
    count: u64,
    in_line: bool,
}

impl Lines {
    pub fn copy(
        &mut self,
        mut input: impl BufRead,
        output: &mut impl Write,
        numbered: bool,
    ) -> io::Result<()> {
        loop {
            let bytes = input.fill_buf()?;
            if bytes.is_empty() {
                return Ok(());
            }
            if !numbered {
                output.write_all(bytes)?;
            } else {
                for part in bytes.split_inclusive(|byte| *byte == b'\n') {
                    if !self.in_line {
                        self.count = self.count.saturating_add(1);
                        write!(output, "{:>6}\t", self.count)?;
                    }
                    output.write_all(part)?;
                    self.in_line = part.last() != Some(&b'\n');
                }
            }
            let length = bytes.len();
            input.consume(length);
        }
    }
}

#[cfg(test)]
#[path = "../tests/stream.rs"]
mod tests;
