//! Allocation-free discovery of Linux-style DTB `/chosen` properties.

use core::str;

use super::fdt::{NodeId, NodeResources, NodeVisitor};

pub const MAX_COMMAND_LINE_SIZE: usize = 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    DuplicateBootargs,
    DuplicateKaslrSeed,
    InvalidEncoding,
    InvalidKaslrSeed,
    InvalidTree,
    TooLong,
}

/// A validated copy of the DTB `/chosen/bootargs` string.
#[derive(Clone, Copy)]
pub struct CommandLine {
    bytes: [u8; MAX_COMMAND_LINE_SIZE],
    length: usize,
}

impl CommandLine {
    const EMPTY: Self = Self {
        bytes: [0; MAX_COMMAND_LINE_SIZE],
        length: 0,
    };

    fn from_property(value: &[u8]) -> Result<Self, Error> {
        let bytes = value.strip_suffix(&[0]).ok_or(Error::InvalidEncoding)?;
        if bytes.contains(&0) {
            return Err(Error::InvalidEncoding);
        }
        let text = str::from_utf8(bytes).map_err(|_| Error::InvalidEncoding)?;
        if text.len() > MAX_COMMAND_LINE_SIZE {
            return Err(Error::TooLong);
        }
        let mut command_line = Self::EMPTY;
        command_line.bytes[..text.len()].copy_from_slice(text.as_bytes());
        command_line.length = text.len();
        Ok(command_line)
    }

    pub fn as_str(&self) -> &str {
        // SAFETY: Construction validates UTF-8 and only copies those bytes.
        unsafe { str::from_utf8_unchecked(&self.bytes[..self.length]) }
    }

    /// Returns the value of the first `name=value` argument.
    pub fn value(&self, name: &str) -> Option<&str> {
        self.as_str().split_ascii_whitespace().find_map(|argument| {
            let (argument_name, value) = argument.split_once('=')?;
            (argument_name == name).then_some(value)
        })
    }

    /// Reports whether a flag or valued argument with this name is present.
    pub fn contains(&self, name: &str) -> bool {
        self.as_str().split_ascii_whitespace().any(|argument| {
            argument == name
                || argument
                    .split_once('=')
                    .is_some_and(|(argument_name, _)| argument_name == name)
        })
    }
}

#[derive(Clone, Copy)]
pub struct Properties {
    command_line: Option<CommandLine>,
    kaslr_seed: Option<u64>,
}

impl Properties {
    pub const fn command_line(&self) -> Option<&CommandLine> {
        self.command_line.as_ref()
    }

    pub const fn kaslr_seed(&self) -> Option<u64> {
        self.kaslr_seed
    }
}

/// Extracts boot configuration during the allocation-free initial FDT walk.
pub struct Discovery {
    depth: usize,
    chosen_depth: Option<usize>,
    command_line: Option<CommandLine>,
    kaslr_seed: Option<u64>,
    error: Option<Error>,
}

impl Discovery {
    pub const fn new() -> Self {
        Self {
            depth: 0,
            chosen_depth: None,
            command_line: None,
            kaslr_seed: None,
            error: None,
        }
    }

    pub fn finish(self) -> Result<Properties, Error> {
        match self.error {
            Some(error) => Err(error),
            None if self.depth != 0 => Err(Error::InvalidTree),
            None => Ok(Properties {
                command_line: self.command_line,
                kaslr_seed: self.kaslr_seed,
            }),
        }
    }

    fn record_property(&mut self, name: &str, value: &[u8]) -> Result<(), Error> {
        match name {
            "bootargs" => {
                if self.command_line.is_some() {
                    return Err(Error::DuplicateBootargs);
                }
                self.command_line = Some(CommandLine::from_property(value)?);
            }
            "kaslr-seed" => {
                if self.kaslr_seed.is_some() {
                    return Err(Error::DuplicateKaslrSeed);
                }
                let raw: [u8; 8] = value.try_into().map_err(|_| Error::InvalidKaslrSeed)?;
                self.kaslr_seed = Some(u64::from_be_bytes(raw));
            }
            _ => {}
        }
        Ok(())
    }
}

impl Default for Discovery {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeVisitor for Discovery {
    fn begin_node(&mut self, _id: NodeId, name: &str) {
        let Some(depth) = self.depth.checked_add(1) else {
            self.error = Some(Error::InvalidTree);
            return;
        };
        self.depth = depth;
        if depth == 2 && name == "chosen" {
            self.chosen_depth = Some(depth);
        }
    }

    fn property(&mut self, _id: NodeId, name: &str, value: &[u8]) {
        if self.error.is_some() || self.chosen_depth != Some(self.depth) {
            return;
        }
        if let Err(error) = self.record_property(name, value) {
            self.error = Some(error);
        }
    }

    fn end_node(&mut self, _node: NodeResources<'_>) {
        if self.depth == 0 {
            self.error = Some(Error::InvalidTree);
            return;
        }
        if self.chosen_depth == Some(self.depth) {
            self.chosen_depth = None;
        }
        self.depth -= 1;
    }
}
