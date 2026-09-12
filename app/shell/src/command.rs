// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Bounded shell words using the standard POSIX shlex syntax.

pub const MAX_LINE_BYTES: usize = 512;
pub const MAX_ARGUMENTS: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseError {
    LineTooLong,
    TooManyArguments,
    InvalidSyntax,
}

pub struct CommandLine(Vec<String>);

impl CommandLine {
    pub fn parse(input: &[u8]) -> Result<Self, ParseError> {
        if input.len() > MAX_LINE_BYTES {
            return Err(ParseError::LineTooLong);
        }
        let text = std::str::from_utf8(input).map_err(|_| ParseError::InvalidSyntax)?;
        if text.contains('\0') {
            return Err(ParseError::InvalidSyntax);
        }
        let words = shlex::split(text).ok_or(ParseError::InvalidSyntax)?;
        if words.len() > MAX_ARGUMENTS {
            return Err(ParseError::TooManyArguments);
        }
        Ok(Self(words))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn argument(&self, index: usize) -> Option<&str> {
        self.0.get(index).map(String::as_str)
    }
    pub fn arguments(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(String::as_str)
    }
}

/// One concurrently running pipeline, with redirects applied after pipe wiring.
pub struct Pipeline(pub Vec<Stage>);
pub struct Stage {
    pub command: CommandLine,
    pub redirects: Vec<Redirect>,
}
#[derive(Debug, PartialEq, Eq)]
pub struct Redirect {
    pub stream: u8,
    pub append: bool,
    pub path: String,
}

impl Pipeline {
    pub fn parse(input: &[u8]) -> Result<Self, ParseError> {
        if input.len() > MAX_LINE_BYTES {
            return Err(ParseError::LineTooLong);
        }
        let text = std::str::from_utf8(input).map_err(|_| ParseError::InvalidSyntax)?;
        if text.contains('\0') {
            return Err(ParseError::InvalidSyntax);
        }
        // Keep quoting intact until shlex decodes each word span. Operators are
        // recognized only outside quotes and escapes, including without spaces.
        let bytes = text.as_bytes();
        let mut tokens: Vec<(String, bool)> = Vec::new();
        let mut start = 0;
        let mut i = 0;
        let mut quote = 0;
        let mut word_start = 0;
        while i < bytes.len() {
            let byte = bytes[i];
            if byte == b'\\' && quote != b'\'' {
                i += 2;
                continue;
            }
            if quote != 0 {
                if byte == quote {
                    quote = 0;
                }
                i += 1;
                continue;
            }
            if byte == b'\'' || byte == b'"' {
                quote = byte;
                i += 1;
                continue;
            }
            if byte == b'#' && i == word_start {
                break;
            }
            if matches!(byte, b'|' | b'<' | b'>' | b';' | b'&') {
                if matches!(byte, b';' | b'&') {
                    return Err(ParseError::InvalidSyntax);
                }
                let mut end = i;
                let stderr =
                    byte == b'>' && i > start && bytes[i - 1] == b'2' && i - 1 == word_start;
                if stderr {
                    end -= 1;
                }
                for word in shlex::split(&text[start..end]).ok_or(ParseError::InvalidSyntax)? {
                    tokens.push((word, false));
                }
                let op_start = end;
                i += 1;
                if byte == b'>' && bytes.get(i) == Some(&b'>') {
                    i += 1;
                }
                tokens.push((text[op_start..i].to_owned(), true));
                start = i;
                word_start = i;
            } else {
                if byte.is_ascii_whitespace() {
                    word_start = i + 1;
                }
                i += 1;
            }
        }
        if i > bytes.len() || quote != 0 {
            return Err(ParseError::InvalidSyntax);
        }
        for word in shlex::split(&text[start..i]).ok_or(ParseError::InvalidSyntax)? {
            tokens.push((word, false));
        }
        let mut stages = Vec::new();
        let mut words = Vec::new();
        let mut redirects = Vec::new();
        let mut tokens = tokens.into_iter();
        while let Some((token, operator)) = tokens.next() {
            if !operator {
                words.push(token);
                if words.len() > MAX_ARGUMENTS {
                    return Err(ParseError::TooManyArguments);
                }
            } else if token == "|" {
                if words.is_empty() || stages.len() >= 7 {
                    return Err(ParseError::InvalidSyntax);
                }
                stages.push(Stage {
                    command: CommandLine(std::mem::take(&mut words)),
                    redirects: std::mem::take(&mut redirects),
                });
            } else {
                let (path, operator) = tokens.next().ok_or(ParseError::InvalidSyntax)?;
                if operator || path.is_empty() {
                    return Err(ParseError::InvalidSyntax);
                }
                redirects.push(Redirect {
                    stream: if token == "<" {
                        0
                    } else if token.starts_with('2') {
                        2
                    } else {
                        1
                    },
                    append: token.ends_with(">>"),
                    path,
                });
            }
        }
        if words.is_empty() {
            if !stages.is_empty() || !redirects.is_empty() {
                return Err(ParseError::InvalidSyntax);
            }
        } else {
            stages.push(Stage {
                command: CommandLine(words),
                redirects,
            });
        }
        Ok(Self(stages))
    }
}

#[cfg(test)]
#[path = "../tests/command.rs"]
mod tests;
