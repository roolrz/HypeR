// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Bounded shell command-line normalization and tokenization.

pub const MAX_LINE_BYTES: usize = 512;
pub const MAX_ARGUMENTS: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseError {
    LineTooLong,
    TooManyArguments,
    UnterminatedQuote,
    TrailingEscape,
}

#[derive(Clone, Copy)]
struct Span {
    start: u16,
    end: u16,
}

const EMPTY_SPAN: Span = Span { start: 0, end: 0 };

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Quote {
    None,
    Single,
    Double,
}

/// One allocation-free parsed command line.
pub struct CommandLine {
    bytes: [u8; MAX_LINE_BYTES],
    length: usize,
    spans: [Span; MAX_ARGUMENTS],
    count: usize,
}

impl CommandLine {
    pub fn parse(input: &[u8]) -> Result<Self, ParseError> {
        if input.len() > MAX_LINE_BYTES {
            return Err(ParseError::LineTooLong);
        }
        let mut command = Self {
            bytes: [0; MAX_LINE_BYTES],
            length: 0,
            spans: [EMPTY_SPAN; MAX_ARGUMENTS],
            count: 0,
        };
        let mut quote = Quote::None;
        let mut escaped = false;
        let mut word_start = 0;
        let mut word_active = false;

        for byte in input.iter().copied() {
            if escaped {
                command.push_byte(byte)?;
                word_active = true;
                escaped = false;
                continue;
            }
            match (quote, byte) {
                (Quote::None | Quote::Double, b'\\') => {
                    if !word_active {
                        word_start = command.length;
                        word_active = true;
                    }
                    escaped = true;
                }
                (Quote::None, b'\'') => {
                    if !word_active {
                        word_start = command.length;
                        word_active = true;
                    }
                    quote = Quote::Single;
                }
                (Quote::Single, b'\'') => quote = Quote::None,
                (Quote::None, b'"') => {
                    if !word_active {
                        word_start = command.length;
                        word_active = true;
                    }
                    quote = Quote::Double;
                }
                (Quote::Double, b'"') => quote = Quote::None,
                (Quote::None, byte) if byte.is_ascii_whitespace() => {
                    if word_active {
                        command.finish_word(word_start)?;
                        word_active = false;
                    }
                }
                (_, byte) => {
                    if !word_active {
                        word_start = command.length;
                        word_active = true;
                    }
                    command.push_byte(byte)?;
                }
            }
        }
        if escaped {
            return Err(ParseError::TrailingEscape);
        }
        if quote != Quote::None {
            return Err(ParseError::UnterminatedQuote);
        }
        if word_active {
            command.finish_word(word_start)?;
        }
        Ok(command)
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.count
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn argument(&self, index: usize) -> Option<&str> {
        let span = *self.spans.get(index)?;
        let bytes = self
            .bytes
            .get(usize::from(span.start)..usize::from(span.end))?;
        core::str::from_utf8(bytes).ok()
    }

    fn push_byte(&mut self, byte: u8) -> Result<(), ParseError> {
        let slot = self
            .bytes
            .get_mut(self.length)
            .ok_or(ParseError::LineTooLong)?;
        *slot = byte;
        self.length = self.length.checked_add(1).ok_or(ParseError::LineTooLong)?;
        Ok(())
    }

    fn finish_word(&mut self, start: usize) -> Result<(), ParseError> {
        let slot = self
            .spans
            .get_mut(self.count)
            .ok_or(ParseError::TooManyArguments)?;
        *slot = Span {
            start: u16::try_from(start).map_err(|_| ParseError::LineTooLong)?,
            end: u16::try_from(self.length).map_err(|_| ParseError::LineTooLong)?,
        };
        self.count = self
            .count
            .checked_add(1)
            .ok_or(ParseError::TooManyArguments)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{CommandLine, ParseError};

    #[test]
    fn parses_quotes_escapes_and_empty_arguments() -> Result<(), ParseError> {
        let command = CommandLine::parse(br#"echo "two words" 'three' four\ five """#)?;
        assert!(!command.is_empty());
        assert_eq!(command.len(), 5);
        assert_eq!(command.argument(0), Some("echo"));
        assert_eq!(command.argument(1), Some("two words"));
        assert_eq!(command.argument(2), Some("three"));
        assert_eq!(command.argument(3), Some("four five"));
        assert_eq!(command.argument(4), Some(""));
        Ok(())
    }

    #[test]
    fn recognizes_an_empty_command_line() -> Result<(), ParseError> {
        let command = CommandLine::parse(b" \t ")?;
        assert!(command.is_empty());
        assert_eq!(command.len(), 0);
        Ok(())
    }

    #[test]
    fn rejects_incomplete_syntax() {
        assert!(matches!(
            CommandLine::parse(b"echo 'missing"),
            Err(ParseError::UnterminatedQuote)
        ));
        assert!(matches!(
            CommandLine::parse(b"echo trailing\\"),
            Err(ParseError::TrailingEscape)
        ));
    }
}
