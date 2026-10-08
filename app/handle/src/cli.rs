// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::{ArgGroup, Parser};
use hyper_os::handle::{ObjectKind, Rights};
use std::num::NonZeroU64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProcessSelector {
    Koid(NonZeroU64),
    Name(String),
}

impl std::str::FromStr for ProcessSelector {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.starts_with(|c: char| c.is_ascii_digit()) {
            parse_id(value).map(Self::Koid)
        } else if value.is_empty() || value.starts_with('-') {
            Err("expected a process KOID or exact process name".into())
        } else {
            Ok(Self::Name(value.into()))
        }
    }
}

#[derive(Debug, Parser)]
#[command(about = "Inspect Native kernel objects and process capabilities")]
#[command(
    after_help = "IDs accept decimal or 0x hexadecimal; output uses full-width hexadecimal.\nWith no selection, list visible objects. Process names match exactly.\nSingle-object and single-handle queries include privileged type details.\nExamples:\n  handle shell\n  handle --object 0x100000012 -v\n  handle --all --object 0x100000012\n  handle io-runtime --kind physical-device\n  handle --all --right write --no-headers"
)]
#[command(group(ArgGroup::new("selection").args([
    "objects", "process", "process_option", "all", "list_kinds", "list_rights"
])))]
#[command(group(ArgGroup::new("handles").args(["process", "process_option", "all"])))]
#[command(group(ArgGroup::new("one_process").args(["process", "process_option"])))]
pub struct Handle {
    /// List visible kernel objects (the default).
    #[arg(long)]
    pub objects: bool,
    /// Inspect handles of a process by full KOID or exact name.
    pub process: Option<ProcessSelector>,
    /// Same as the positional PROCESS argument.
    #[arg(short = 'p', long = "process", value_name = "PROCESS")]
    pub process_option: Option<ProcessSelector>,
    /// Scan handles across visible processes; combine with --object to find holders.
    #[arg(short = 'a', long)]
    pub all: bool,
    /// Only show this object KOID, or handles referring to it.
    #[arg(short = 'o', long, value_name = "KOID", value_parser = parse_id, conflicts_with_all = ["list_kinds", "list_rights"])]
    pub object: Option<NonZeroU64>,
    /// Only show this process-local handle (requires PROCESS).
    #[arg(long, value_name = "HANDLE", value_parser = parse_id, requires = "one_process")]
    pub handle: Option<NonZeroU64>,
    /// Filter by kind name or numeric kind ID; repeat to match any listed kind.
    #[arg(short = 'k', long, value_name = "KIND", value_parser = parse_kind, conflicts_with_all = ["list_kinds", "list_rights"])]
    pub kind: Vec<u32>,
    /// Require a granted right; repeat to require all listed rights.
    #[arg(short = 'r', long, value_name = "RIGHT", value_parser = parse_right, requires = "handles")]
    pub right: Vec<Rights>,
    /// Include purposes, raw rights/flags, and object reference counts by owner class.
    #[arg(short = 'v', long, conflicts_with_all = ["no_headers", "list_kinds", "list_rights"])]
    pub verbose: bool,
    /// Print table rows only, omitting headers, banners, and type-specific details.
    #[arg(long)]
    pub no_headers: bool,
    /// List all SDK object kinds and their purposes; no inspector is needed.
    #[arg(long)]
    pub list_kinds: bool,
    /// List all SDK right names; no inspector is needed.
    #[arg(long)]
    pub list_rights: bool,
}

impl Handle {
    pub fn process_selector(&self) -> Option<&ProcessSelector> {
        self.process.as_ref().or(self.process_option.as_ref())
    }

    pub fn matches_object(&self, koid: u64, kind: ObjectKind) -> bool {
        self.object.is_none_or(|id| id.get() == koid)
            && (self.kind.is_empty() || self.kind.contains(&kind.as_raw()))
    }
}

fn parse_id(value: &str) -> Result<NonZeroU64, String> {
    let number = if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        if hex.is_empty() || !hex.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err("expected a nonzero 64-bit ID in decimal or 0x hexadecimal".into());
        }
        u64::from_str_radix(hex, 16)
    } else {
        if value.is_empty() || !value.bytes().all(|c| c.is_ascii_digit()) {
            return Err("expected a nonzero 64-bit ID in decimal or 0x hexadecimal".into());
        }
        value.parse()
    };
    number
        .ok()
        .and_then(NonZeroU64::new)
        .ok_or_else(|| "expected a nonzero 64-bit ID in decimal or 0x hexadecimal".into())
}

fn parse_kind(value: &str) -> Result<u32, String> {
    if let Some(kind) = ObjectKind::from_name(value) {
        return Ok(kind.as_raw());
    }
    parse_id(value)
        .ok()
        .and_then(|id| u32::try_from(id.get()).ok())
        .ok_or_else(|| {
            format!("unknown kind '{value}'; see --list-kinds (numeric kind IDs are also accepted)")
        })
}

fn parse_right(value: &str) -> Result<Rights, String> {
    Rights::from_name(value).ok_or_else(|| format!("unknown right '{value}'; see --list-rights"))
}

#[cfg(test)]
#[path = "../tests/cli.rs"]
mod tests;
