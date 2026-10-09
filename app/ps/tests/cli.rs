// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn process_filters_accept_hex_lists_and_name_intersection() -> Result<(), clap::Error> {
    let args = Ps::try_parse_from([
        "ps",
        "-p",
        "0x100000001,2",
        "--name",
        "shell",
        "--no-headers",
    ])?;
    assert!(args.no_headers);
    assert!(args.filter.matches(0x100000001, "shell"));
    assert!(!args.filter.matches(1, "shell"));
    assert!(!args.filter.matches(2, "vm-manager"));
    Ok(())
}

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
