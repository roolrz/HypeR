//! Allocation-free discovery of Linux-style DTB `/chosen` properties.

use core::str;

use super::{
    PhysicalRange,
    fdt::{NodeId, NodeResources, NodeVisitor},
};

pub const MAX_COMMAND_LINE_SIZE: usize = 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    DuplicateBootargs,
    DuplicateKaslrSeed,
    DuplicateInitrdEnd,
    DuplicateInitrdStart,
    InvalidEncoding,
    InvalidInitrd,
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

    pub fn parse(value: &str) -> Result<Self, Error> {
        if value.len() > MAX_COMMAND_LINE_SIZE {
            return Err(Error::TooLong);
        }
        let mut command_line = Self::EMPTY;
        command_line.bytes[..value.len()].copy_from_slice(value.as_bytes());
        command_line.length = value.len();
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
    command_line_error: Option<Error>,
    kaslr_seed: Option<u64>,
    kaslr_seed_error: Option<Error>,
    initial_ramdisk: Option<PhysicalRange>,
}

impl Properties {
    pub const fn command_line(&self) -> Option<&CommandLine> {
        self.command_line.as_ref()
    }

    pub const fn kaslr_seed(&self) -> Option<u64> {
        self.kaslr_seed
    }

    pub const fn command_line_error(&self) -> Option<Error> {
        self.command_line_error
    }

    pub const fn kaslr_seed_error(&self) -> Option<Error> {
        self.kaslr_seed_error
    }

    pub const fn initial_ramdisk(&self) -> Option<PhysicalRange> {
        self.initial_ramdisk
    }
}

/// Extracts boot configuration during the allocation-free initial FDT walk.
pub struct Discovery {
    depth: usize,
    chosen_depth: Option<usize>,
    command_line: Option<CommandLine>,
    command_line_error: Option<Error>,
    kaslr_seed: Option<u64>,
    kaslr_seed_error: Option<Error>,
    initrd_start: Option<u64>,
    initrd_end: Option<u64>,
    error: Option<Error>,
}

impl Discovery {
    pub const fn new() -> Self {
        Self {
            depth: 0,
            chosen_depth: None,
            command_line: None,
            command_line_error: None,
            kaslr_seed: None,
            kaslr_seed_error: None,
            initrd_start: None,
            initrd_end: None,
            error: None,
        }
    }

    pub fn finish(self) -> Result<Properties, Error> {
        match self.error {
            Some(error) => Err(error),
            None if self.depth != 0 => Err(Error::InvalidTree),
            None => {
                let initial_ramdisk = match (self.initrd_start, self.initrd_end) {
                    (None, None) => None,
                    (Some(start), Some(end)) => Some(
                        PhysicalRange::new(
                            start,
                            end.checked_sub(start).ok_or(Error::InvalidInitrd)?,
                        )
                        .ok_or(Error::InvalidInitrd)?,
                    ),
                    _ => return Err(Error::InvalidInitrd),
                };
                Ok(Properties {
                    command_line: self.command_line,
                    command_line_error: self.command_line_error,
                    kaslr_seed: self.kaslr_seed,
                    kaslr_seed_error: self.kaslr_seed_error,
                    initial_ramdisk,
                })
            }
        }
    }

    fn record_property(&mut self, name: &str, value: &[u8]) -> Result<(), Error> {
        match name {
            "bootargs" => {
                if self.command_line.is_some() || self.command_line_error.is_some() {
                    self.command_line = None;
                    self.command_line_error = Some(Error::DuplicateBootargs);
                } else {
                    match CommandLine::from_property(value) {
                        Ok(command_line) => self.command_line = Some(command_line),
                        Err(error) => self.command_line_error = Some(error),
                    }
                }
            }
            "kaslr-seed" => {
                if self.kaslr_seed.is_some() || self.kaslr_seed_error.is_some() {
                    self.kaslr_seed = None;
                    self.kaslr_seed_error = Some(Error::DuplicateKaslrSeed);
                } else {
                    match <[u8; 8]>::try_from(value) {
                        Ok(raw) => self.kaslr_seed = Some(u64::from_be_bytes(raw)),
                        Err(_) => self.kaslr_seed_error = Some(Error::InvalidKaslrSeed),
                    }
                }
            }
            "linux,initrd-start" => {
                if self.initrd_start.is_some() {
                    return Err(Error::DuplicateInitrdStart);
                }
                self.initrd_start = Some(decode_address(value)?);
            }
            "linux,initrd-end" => {
                if self.initrd_end.is_some() {
                    return Err(Error::DuplicateInitrdEnd);
                }
                self.initrd_end = Some(decode_address(value)?);
            }
            _ => {}
        }
        Ok(())
    }
}

fn decode_address(value: &[u8]) -> Result<u64, Error> {
    match value {
        [a, b, c, d] => Ok(u64::from(u32::from_be_bytes([*a, *b, *c, *d]))),
        [a, b, c, d, e, f, g, h] => Ok(u64::from_be_bytes([*a, *b, *c, *d, *e, *f, *g, *h])),
        _ => Err(Error::InvalidInitrd),
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
        if self.chosen_depth != Some(self.depth) {
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
