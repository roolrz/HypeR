// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn units_and_sampling_validate_before_inspection() -> Result<(), clap::Error> {
    let args = Free::try_parse_from(["free", "-m", "-s", "0.1", "-c", "3"])?;
    assert_eq!(args.quantity(3 * 1024 * 1024 + 1), "3 MiB");
    assert_eq!(args.count.get(), 3);
    for words in [
        vec!["free", "-bm"],
        vec!["free", "-c", "0"],
        vec!["free", "-s", "NaN"],
    ] {
        assert!(Free::try_parse_from(words).is_err());
    }
    Ok(())
}

#[test]
fn rejects_extra_arguments() {
    assert!(Free::try_parse_from(["free", "unexpected"]).is_err());
}

#[test]
fn memory_format_keeps_small_caches_visible_and_bytes_exact() {
    assert_eq!(crate::format_bytes(0, false), "0 B");
    assert_eq!(crate::format_bytes(1536, false), "1.5 KiB");
    assert_eq!(crate::format_bytes(1536, true), "1536 B");
    assert_eq!(crate::format_bytes(u64::MAX, false), "15.9 EiB");
}
