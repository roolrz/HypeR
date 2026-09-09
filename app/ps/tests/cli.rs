// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn flags_and_help() {
    assert!(Ps::try_parse_from(["ps", "-T"]).is_ok_and(|args| args.threads));
    assert!(Ps::try_parse_from(["ps", "--threads"]).is_ok_and(|args| args.threads));
    assert!(Ps::try_parse_from(["ps", "unexpected"]).is_err());
    assert!(
        Ps::try_parse_from(["ps", "--help"])
            .is_err_and(|e| e.kind() == clap::error::ErrorKind::DisplayHelp)
    );
}
