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
    assert!(
        Vmm::try_parse_from(["vmm", "create", "test", "--config", "/etc/hyper/vms.json"]).is_ok()
    );
}

#[test]
fn create_uses_complete_named_configuration_and_preserves_disk() {
    let config = || {
        Config::parse(
            br#"{"format":"hyper.vm-config","virtual-machines":[{
        "name":"alpine","image":"/data/vm.itb","autostart":true,
        "configuration":{"vcpus":4,"memory-bytes":134217728,"bootargs":"console=test",
                         "affinity":[{"vcpu":1,"cpus":[0,2]}]},
        "disk":{"client":1,"volume":"vm"}}]}"#,
        )
        .unwrap()
    };
    assert!(create_request(config(), "missing".into(), None, false).is_err());
    let Request::Create { definitions } =
        create_request(config(), "copy".into(), Some("alpine".into()), false).unwrap()
    else {
        panic!("expected create");
    };
    let vm = &definitions[0];
    assert_eq!(vm.name, "copy");
    assert!(!vm.autostart);
    assert_eq!(vm.configuration.vcpus, 4);
    assert_eq!(vm.configuration.bootargs, "console=test");
    assert_eq!(vm.configuration.affinity[0].vcpu, 1);
    assert_eq!(vm.configuration.affinity[0].cpus, [0, 2]);
    assert_eq!(vm.disk.as_ref().unwrap().client, 1);
    assert!(Vmm::try_parse_from(["vmm", "create", "test", "--image", "/vm/a.itb"]).is_err());
}

#[test]
fn affinity_parses_cpu_sets_and_rejects_invalid_lists() {
    let cli = Vmm::try_parse_from(["vmm", "affinity", "alpine", "1", "0,2-3"]).unwrap();
    assert!(
        matches!(cli.command.unwrap().request().unwrap(), Request::Affinity { name, vcpu: 1, affinity_words } if name == "alpine" && affinity_words[0] == 13)
    );
    for value in [
        "", "1,", "1,,2", "4-2", "-1", "0-256", "256", "1-2-3", "abc", "+1",
    ] {
        assert!(parse_cpu_list(value).is_err(), "{value}");
    }
    assert_eq!(
        parse_cpu_list("0,0,63-64").unwrap()[..2],
        [1 | (1 << 63), 1]
    );
    assert!(Vmm::try_parse_from(["vmm", "affinity", "alpine", "1"]).is_err());
    assert!(Vmm::try_parse_from(["vmm", "migrate", "alpine", "1", "2"]).is_err());
}
