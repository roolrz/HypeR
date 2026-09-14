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

#[test]
fn disk_arguments_are_paired_and_preserved() {
    assert!(
        Vmm::try_parse_from([
            "vmm",
            "create",
            "vm",
            "--image",
            "/data/vm.itb",
            "--disk-client",
            "1"
        ])
        .is_err()
    );
    let cli = Vmm::try_parse_from([
        "vmm",
        "create",
        "vm",
        "--image",
        "/data/vm.itb",
        "--disk-client",
        "1",
        "--disk-volume",
        "vm",
    ])
    .unwrap();
    let Request::Create { definitions } = cli.command.unwrap().request().unwrap() else {
        panic!("expected create");
    };
    assert_eq!(definitions[0].disk.as_ref().unwrap().client, 1);
    assert_eq!(definitions[0].disk.as_ref().unwrap().volume, "vm");
}
