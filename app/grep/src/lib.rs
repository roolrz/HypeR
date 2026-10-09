// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
use regex::bytes::{Regex, RegexBuilder};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(disable_help_flag = true)]
#[command(about = "Select lines matching a regular expression (or -F literal text)")]
pub struct Grep {
    #[arg(long, action = clap::ArgAction::Help)]
    pub help: Option<bool>,
    #[arg(short = 'i', long)]
    pub ignore_case: bool,
    #[arg(short = 'v', long)]
    pub invert_match: bool,
    #[arg(short = 'n', long)]
    pub line_number: bool,
    #[arg(short = 'c', long)]
    pub count: bool,
    #[arg(short = 'l', long)]
    pub files_with_matches: bool,
    #[arg(short = 'q', long)]
    pub quiet: bool,
    #[arg(short = 'F', long)]
    pub fixed_strings: bool,
    /// Stop after NUM selected lines per input; zero reads no lines.
    #[arg(short = 'm', long)]
    pub max_count: Option<u64>,
    /// Require the pattern to match the whole line.
    #[arg(short = 'x', long)]
    pub line_regexp: bool,
    /// Flush each selected line, useful in long-running pipelines.
    #[arg(long)]
    pub line_buffered: bool,
    #[arg(short = 'H', long)]
    pub with_filename: bool,
    #[arg(short = 'h', long)]
    pub no_filename: bool,
    #[arg(short = 'e', long = "regexp", action = clap::ArgAction::Append)]
    pub patterns: Vec<String>,
    /// Pattern followed by files; with -e, all operands are files. '-' reads stdin.
    pub operands: Vec<String>,
}
impl Grep {
    pub fn matcher(&self) -> Result<(Regex, Vec<PathBuf>), String> {
        let (patterns, files) = if self.patterns.is_empty() {
            let (pattern, files) = self.operands.split_first().ok_or("a pattern is required")?;
            (vec![pattern.as_str()], files)
        } else {
            (
                self.patterns.iter().map(String::as_str).collect(),
                self.operands.as_slice(),
            )
        };
        let patterns: Vec<_> = patterns
            .into_iter()
            .flat_map(|p| p.split('\n'))
            .map(|p| {
                if self.fixed_strings {
                    regex::escape(p)
                } else {
                    p.to_owned()
                }
            })
            .collect();
        let regex = RegexBuilder::new(
            &patterns
                .iter()
                .map(|p| {
                    if self.line_regexp {
                        format!("\\A(?:{p})\\z")
                    } else {
                        format!("(?:{p})")
                    }
                })
                .collect::<Vec<_>>()
                .join("|"),
        )
        .case_insensitive(self.ignore_case)
        .unicode(false)
        .build()
        .map_err(|error| error.to_string())?;
        let files = if files.is_empty() {
            vec![PathBuf::from("-")]
        } else {
            files.iter().map(PathBuf::from).collect()
        };
        Ok((regex, files))
    }
    pub fn scan(
        &self,
        regex: &Regex,
        mut input: impl BufRead,
        output: &mut impl Write,
        name: &str,
        show_name: bool,
    ) -> io::Result<bool> {
        let mut line = Vec::new();
        let mut number = 0_u64;
        let mut selected = 0_u64;
        loop {
            if self.max_count.is_some_and(|limit| selected >= limit) {
                break;
            }
            line.clear();
            if input.read_until(b'\n', &mut line)? == 0 {
                break;
            }
            number += 1;
            let content = line.strip_suffix(b"\n").unwrap_or(&line);
            if regex.is_match(content) == self.invert_match {
                continue;
            }
            selected += 1;
            if self.quiet {
                return Ok(true);
            }
            if self.files_with_matches {
                writeln!(output, "{name}")?;
                return Ok(true);
            }
            if !self.count {
                if show_name {
                    write!(output, "{name}:")?;
                }
                if self.line_number {
                    write!(output, "{number}:")?;
                }
                output.write_all(content)?;
                output.write_all(b"\n")?;
                if self.line_buffered {
                    output.flush()?;
                }
            }
        }
        if self.count && !self.files_with_matches && !self.quiet {
            if show_name {
                write!(output, "{name}:")?;
            }
            writeln!(output, "{selected}")?;
        }
        Ok(selected != 0)
    }
}

#[cfg(test)]
#[path = "../tests/grep.rs"]
mod tests;
