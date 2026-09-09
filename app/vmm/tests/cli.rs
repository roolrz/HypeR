// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn subcommands_and_help() {
    assert!(Vmm::try_parse_from(["vmm"]).is_ok_and(|args| args.command.is_none()));
    assert!(Vmm::try_parse_from(["vmm", "console"]).is_ok());
    assert!(Vmm::try_parse_from(["vmm", "unknown"]).is_err());
    assert!(
        Vmm::try_parse_from(["vmm", "console", "--help"])
            .is_err_and(|e| e.kind() == clap::error::ErrorKind::DisplayHelp)
    );
}
