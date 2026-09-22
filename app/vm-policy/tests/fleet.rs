// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;
#[test]
fn rejects_ambiguous_names_paths_and_duplicate_definitions() {
    let valid = Definition {
        name: "alpine".into(),
        image: "/vm/alpine.itb".into(),
        autostart: false,
        disk: None,
    };
    assert!(valid.validate().is_ok());
    assert!(validate_definitions(&[valid.clone(), valid.clone()]).is_err());
    for name in ["", "../vm", "test\n", "-test"] {
        assert!(
            Definition {
                name: name.into(),
                ..valid.clone()
            }
            .validate()
            .is_err()
        );
    }
    assert!(
        Definition {
            image: "/vm/../etc/config".into(),
            ..valid
        }
        .validate()
        .is_err()
    );
}
#[test]
fn named_request_round_trip() -> Result<(), String> {
    let encoded = encode(&Request::Control {
        name: "second".into(),
        action: Action::Stop,
    })?;
    assert!(
        matches!(request(&encoded)?, Request::Control { name, action: Action::Stop } if name == "second")
    );
    Ok(())
}

#[test]
fn disk_identity_is_explicit_exclusive_and_survives_round_trip() {
    let bytes = br#"{"format":"hyper.vm-config","virtual-machines":[{"name":"vm","image":"/data/vm.itb","disk":{"client":1,"volume":"vm"}}]}"#;
    let config = Config::parse(bytes).unwrap();
    assert_eq!(
        Config::parse(&config.to_bytes().unwrap()).unwrap().machines,
        config.machines
    );
    let mut other = config.machines[0].clone();
    other.name = "second".into();
    assert!(validate_definitions(&[config.machines[0].clone(), other]).is_err());
    for client in [0, 128] {
        assert!(
            Disk {
                client,
                volume: "vm".into()
            }
            .validate()
            .is_err()
        );
    }
    assert!(
        Disk {
            client: 1,
            volume: "../config".into()
        }
        .validate()
        .is_err()
    );
}

#[test]
fn affinity_wire_preserves_multiple_allowed_cpus() {
    let request = Request::Affinity {
        name: "alpine".into(),
        vcpu: 1,
        affinity_words: vec![0b1101],
    };
    assert!(
        matches!(super::request(&encode(&request).unwrap()).unwrap(), Request::Affinity { vcpu: 1, affinity_words, .. } if affinity_words == [13])
    );
    assert!(
        super::request(br#"{"command":"affinity","name":"alpine","vcpu":-1,"affinity_words":[1]}"#)
            .is_err()
    );
}
