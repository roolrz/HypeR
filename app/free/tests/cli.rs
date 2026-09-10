// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;

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
