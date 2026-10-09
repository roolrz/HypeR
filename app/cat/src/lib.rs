// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

pub mod cli;

use std::io::{self, BufRead, Write};

/// Keep line numbering continuous across file and buffer boundaries.
#[derive(Default)]
pub struct Lines {
    count: u64,
    in_line: bool,
    previous_blank: bool,
}

#[derive(Clone, Copy, Default)]
pub struct Options {
    pub number: bool,
    pub number_nonblank: bool,
    pub squeeze_blank: bool,
}

impl Lines {
    pub fn copy(
        &mut self,
        mut input: impl BufRead,
        output: &mut impl Write,
        options: Options,
    ) -> io::Result<()> {
        loop {
            let bytes = input.fill_buf()?;
            if bytes.is_empty() {
                return Ok(());
            }
            if !options.number && !options.number_nonblank && !options.squeeze_blank {
                output.write_all(bytes)?;
            } else {
                for part in bytes.split_inclusive(|byte| *byte == b'\n') {
                    let blank = !self.in_line && part == b"\n";
                    if blank && options.squeeze_blank && self.previous_blank {
                        continue;
                    }
                    if !self.in_line
                        && (options.number_nonblank && !blank
                            || options.number && !options.number_nonblank)
                    {
                        self.count = self.count.saturating_add(1);
                        write!(output, "{:>6}\t", self.count)?;
                    }
                    output.write_all(part)?;
                    self.in_line = part.last() != Some(&b'\n');
                    self.previous_blank = blank;
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
