// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn selection_and_numeric_range() {
    assert!(Handle::try_parse_from(["handle", "--objects"]).is_ok());
    assert!(Handle::try_parse_from(["handle", "18446744073709551615"]).is_ok());
    for args in [
        vec!["handle"],
        vec!["handle", "0"],
        vec!["handle", "-1"],
        vec!["handle", "18446744073709551616"],
        vec!["handle", "1", "2"],
        vec!["handle", "--objects", "1"],
    ] {
        assert!(Handle::try_parse_from(args).is_err());
    }
}
