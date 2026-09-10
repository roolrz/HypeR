// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;
#[test]
fn commands_require_explicit_names() {
    for command in ["status", "start", "stop", "restart", "console", "delete"] {
        assert!(Vmm::try_parse_from(["vmm", command]).is_err());
        assert!(Vmm::try_parse_from(["vmm", command, "alpine"]).is_ok());
    }
    assert!(Vmm::try_parse_from(["vmm", "create", "test"]).is_err());
    assert!(Vmm::try_parse_from(["vmm", "create", "test", "--image", "/vm/alpine.itb"]).is_ok());
}
