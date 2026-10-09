// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn identifiers_preserve_generation_and_reject_ambiguous_input() -> Result<(), String> {
    assert_eq!(parse_id("0x100000001")?.get(), 0x100000001);
    assert_eq!(parse_id("18446744073709551615")?.get(), u64::MAX);
    for value in [
        "0",
        "0x0",
        "0x",
        "-1",
        "+1",
        " 1",
        "1_000",
        "18446744073709551616",
    ] {
        assert!(parse_id(value).is_err(), "{value}");
    }
    Ok(())
}

#[test]
fn filters_intersect_name_with_union_of_full_ids() -> Result<(), String> {
    let filter = ProcessFilter {
        process: vec![parse_id("0x100000001")?, parse_id("2")?],
        name: Some("runtime".into()),
    };
    assert!(filter.matches(0x100000001, "vm-runtime"));
    assert!(filter.matches(2, "io-runtime"));
    assert!(!filter.matches(1, "vm-runtime"));
    assert!(!filter.matches(2, "shell"));
    for value in ["NaN", "inf", "0", "0.01", "61"] {
        assert!(parse_interval(value).is_err());
    }
    Ok(())
}
