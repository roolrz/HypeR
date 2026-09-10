// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

pub mod cli;

pub fn readable_size(size: u64) -> String {
    let mut divisor = 1_u64;
    let mut unit = "B";
    for next in ["KiB", "MiB", "GiB", "TiB", "PiB", "EiB"] {
        if size / divisor < 1024 {
            break;
        }
        divisor *= 1024;
        unit = next;
    }
    if divisor == 1 {
        format!("{size} B")
    } else {
        format!(
            "{}.{:01} {unit}",
            size / divisor,
            (size % divisor) * 10 / divisor
        )
    }
}

pub fn mode_text(mode: u32, directory: bool, symlink: bool) -> String {
    let mut text = String::from(if directory {
        "d"
    } else if symlink {
        "l"
    } else {
        "-"
    });
    for shift in [6, 3, 0] {
        for (bit, character) in [(4, 'r'), (2, 'w'), (1, 'x')] {
            text.push(if mode >> shift & bit != 0 {
                character
            } else {
                '-'
            });
        }
    }
    for (position, bit, lower, upper) in [
        (3, 0o4000, "s", "S"),
        (6, 0o2000, "s", "S"),
        (9, 0o1000, "t", "T"),
    ] {
        if mode & bit != 0 {
            let executable = text.as_bytes()[position] == b'x';
            text.replace_range(
                position..position + 1,
                if executable { lower } else { upper },
            );
        }
    }
    text
}

#[cfg(test)]
#[path = "../tests/format.rs"]
mod tests;
