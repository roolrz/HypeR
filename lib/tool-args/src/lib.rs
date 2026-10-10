// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Argument parsing and process selection for Native system tools.
//!
//! Applications and this Rust shared library are built and deployed together.
//! This library carries no inspection authority.

// Select the shared parser implementation for Native delivery.
#[cfg(target_os = "hyper")]
extern crate hyper_clap_shared as _;

use clap::Args;
use std::num::NonZeroU64;

/// Parse a complete, generation-bearing identifier without accepting signs or truncation.
pub fn parse_id(value: &str) -> Result<NonZeroU64, String> {
    let (digits, radix) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .map_or((value, 10), |hex| (hex, 16));
    if digits.is_empty()
        || !digits.bytes().all(|byte| {
            if radix == 16 {
                byte.is_ascii_hexdigit()
            } else {
                byte.is_ascii_digit()
            }
        })
    {
        return Err("expected a nonzero 64-bit ID in decimal or 0x hexadecimal".into());
    }
    u64::from_str_radix(digits, radix)
        .ok()
        .and_then(NonZeroU64::new)
        .ok_or_else(|| "expected a nonzero 64-bit ID in decimal or 0x hexadecimal".into())
}

#[derive(Debug, Default, Args)]
pub struct ProcessFilter {
    /// Select process KOIDs (decimal or 0x hexadecimal); repeat or separate with commas.
    #[arg(short = 'p', long, value_delimiter = ',', value_parser = parse_id)]
    pub process: Vec<NonZeroU64>,
    /// Show process names containing this text; combines with --process.
    #[arg(long)]
    pub name: Option<String>,
}

impl ProcessFilter {
    pub fn matches(&self, koid: u64, name: &str) -> bool {
        (self.process.is_empty() || self.process.iter().any(|id| id.get() == koid))
            && self.name.as_ref().is_none_or(|part| name.contains(part))
    }

    pub fn is_empty(&self) -> bool {
        self.process.is_empty() && self.name.is_none()
    }
}

/// Bound sampling work and reject NaN/infinity before constructing a Duration.
pub fn parse_interval(value: &str) -> Result<f64, String> {
    let seconds: f64 = value.parse().map_err(|_| "expected seconds".to_owned())?;
    if !seconds.is_finite() || !(0.1..=60.0).contains(&seconds) {
        return Err("interval must be between 0.1 and 60 seconds".into());
    }
    Ok(seconds)
}

#[cfg(test)]
#[path = "../tests/selectors.rs"]
mod tests;
