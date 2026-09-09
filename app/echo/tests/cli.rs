// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn preserves_option_looking_text() {
    assert!(
        Echo::try_parse_from(["echo", "--help", "-n", "two words", ""])
            .is_ok_and(|args| args.words == ["--help", "-n", "two words", ""])
    );
}
