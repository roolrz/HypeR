// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;

#[derive(Debug, Parser)]
#[command(about = "Monitor Native CPU, memory and processes")]
pub struct Top {
    /// Emit plain snapshots without terminal escape sequences or keyboard input.
    #[arg(short = 'b', long)]
    pub batch: bool,
    /// Exit after this many snapshots.
    #[arg(short = 'n', long)]
    pub iterations: Option<std::num::NonZeroU32>,
    /// Refresh period in seconds (0.1 to 60).
    #[arg(short = 'd', long, default_value = "1", value_parser = parse_delay)]
    pub delay: f64,
}

fn parse_delay(value: &str) -> Result<f64, String> {
    let delay: f64 = value.parse().map_err(|_| "expected seconds".to_string())?;
    if !delay.is_finite() || !(0.1..=60.0).contains(&delay) {
        return Err("delay must be between 0.1 and 60 seconds".into());
    }
    Ok(delay)
}

#[cfg(test)]
#[path = "../tests/cli.rs"]
mod tests;
