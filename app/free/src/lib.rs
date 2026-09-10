// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

pub mod cli;

/// Retain sub-MiB cache observations in the default human-readable output.
pub fn format_bytes(bytes: u64, exact: bool) -> String {
    if exact || bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut divisor = 1024u64;
    let mut unit = "KiB";
    for next in ["MiB", "GiB", "TiB", "PiB", "EiB"] {
        if bytes / divisor < 1024 {
            break;
        }
        divisor *= 1024;
        unit = next;
    }
    // The largest divisor is 2^60, so the remainder times ten fits u64.
    let tenths = bytes % divisor * 10 / divisor;
    format!("{}.{tenths} {unit}", bytes / divisor)
}
