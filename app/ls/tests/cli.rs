// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn rejects_extra_arguments() {
    assert!(Ls::try_parse_from(["ls", "a", "b"]).is_err());
}
