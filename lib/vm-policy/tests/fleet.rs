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
        network: None,
        configuration: configuration(),
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
    let bytes = br#"{"format":"hyper.vm-config","virtual-machines":[{"name":"vm","image":"/data/vm.itb","configuration":{"vcpus":2,"memory-bytes":134217728,"bootargs":"console=ttyAMA0"},"disk":{"client":1,"volume":"vm"}}]}"#;
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

fn network_definition(name: &str, client: u32, mac: &str) -> Definition {
    Definition {
        name: name.into(),
        image: "/data/vm/alpine.itb".into(),
        configuration: configuration(),
        autostart: false,
        disk: None,
        network: Some(Network {
            client,
            network: "default".into(),
            mac: mac.into(),
        }),
    }
}

#[test]
fn network_only_and_combined_definitions_share_one_connection() -> Result<(), String> {
    let mut definition = network_definition("network-vm", 2, "02:48:59:00:00:02");
    let connection = definition
        .io_connection()?
        .ok_or("missing network session")?;
    assert_eq!(connection.client, 2);
    assert_eq!(connection.devices(), hyper_service::io::NETWORK);
    assert_eq!(connection.volume, None);
    definition.disk = Some(Disk {
        client: 2,
        volume: "data".into(),
    });
    let connection = definition
        .io_connection()?
        .ok_or("missing combined session")?;
    assert_eq!(
        connection.devices(),
        hyper_service::io::DISK | hyper_service::io::NETWORK
    );
    let encoded = connection.encode().ok_or("invalid combined admission")?;
    assert_eq!(
        hyper_service::io::decode_connect(&encoded),
        Some(connection)
    );
    let config = Config {
        format: "hyper.vm-config".into(),
        machines: vec![definition.clone()],
        copyright: None,
        license: None,
    };
    assert_eq!(
        Config::parse(&config.to_bytes()?)?.machines,
        config.machines
    );
    definition.disk = Some(Disk {
        client: 3,
        volume: "data".into(),
    });
    assert!(definition.validate().is_err());
    assert!(definition.io_connection().is_err());
    Ok(())
}

#[test]
fn fleet_and_incremental_admission_reject_cross_device_authority_collisions() {
    let first = network_definition("first", 1, "02:48:59:00:00:01");
    let mut other = network_definition("second", 2, "02:48:59:00:00:02");
    assert!(validate_definitions(&[first.clone(), other.clone()]).is_ok());
    assert!(first.check_conflicts(&other).is_ok());
    other.network = None;
    other.disk = Some(Disk {
        client: 1,
        volume: "second".into(),
    });
    assert!(validate_definitions(&[first.clone(), other.clone()]).is_err());
    assert!(other.check_conflicts(&first).is_err());
    assert!(first.check_conflicts(&other).is_err());
    let other = network_definition("second", 2, "02:48:59:00:00:01");
    assert!(validate_definitions(&[first.clone(), other.clone()]).is_err());
    assert!(other.check_conflicts(&first).is_err());
    let mut first = first;
    first.disk = Some(Disk {
        client: 1,
        volume: "shared".into(),
    });
    let mut other = network_definition("second", 2, "02:48:59:00:00:02");
    other.disk = Some(Disk {
        client: 2,
        volume: "shared".into(),
    });
    assert!(validate_definitions(&[first.clone(), other.clone()]).is_err());
    assert!(other.check_conflicts(&first).is_err());
}

#[test]
fn network_policy_rejects_native_client_and_noncanonical_identity() {
    for client in [0, 128] {
        assert!(
            network_definition("vm", client, "02:48:59:00:00:01")
                .validate()
                .is_err()
        );
    }
    for mac in [
        "",
        "00:48:59:00:00:01",
        "03:48:59:00:00:01",
        "02:48:59:00:00:AA",
    ] {
        assert!(network_definition("vm", 1, mac).validate().is_err());
    }
    let mut definition = network_definition("vm", 1, "02:48:59:00:00:01");
    if let Some(network) = definition.network.as_mut() {
        network.network = "not/a/network".into();
    }
    assert!(definition.validate().is_err());
}

#[test]
fn summaries_retain_network_policy_without_inventing_a_disk() -> Result<(), String> {
    let bytes = br#"{"result":"entries","machines":[{"name":"vm","state":"stopped","image":"/data/vm/alpine.itb","autostart":false,"network":{"client":2,"network":"default","mac":"02:48:59:00:00:02"}}]}"#;
    let Response::Entries { machines } = response(bytes)? else {
        return Err("wrong response kind".into());
    };
    assert!(machines[0].disk.is_none());
    assert_eq!(
        machines[0].network.as_ref().map(|value| value.client),
        Some(2)
    );
    let encoded = encode(&Response::Entries { machines })?;
    let Response::Entries { machines } = response(&encoded)? else {
        return Err("round trip changed response kind".into());
    };
    assert_eq!(
        machines[0].network.as_ref().map(|value| value.mac.as_str()),
        Some("02:48:59:00:00:02")
    );
    Ok(())
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

fn configuration() -> Configuration {
    Configuration {
        vcpus: 2,
        memory_bytes: 128 * 1024 * 1024,
        bootargs: "console=ttyAMA0".into(),
        affinity: Vec::new(),
    }
}

#[test]
fn configuration_is_required_and_strict() {
    let valid = serde_json::json!({"format":"hyper.vm-config", "virtual-machines":[{
        "name":"vm", "image":"/vm/test.itb", "configuration": configuration()
    }]});
    assert!(Config::parse(&serde_json::to_vec(&valid).unwrap()).is_ok());
    for field in ["vcpus", "memory-bytes", "bootargs"] {
        let mut broken = valid.clone();
        broken["virtual-machines"][0]["configuration"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(Config::parse(&serde_json::to_vec(&broken).unwrap()).is_err());
    }
    let mut broken = valid;
    broken["virtual-machines"][0]
        .as_object_mut()
        .unwrap()
        .remove("configuration");
    assert!(Config::parse(&serde_json::to_vec(&broken).unwrap()).is_err());
    for (field, value) in [
        ("vcpus", serde_json::json!(0)),
        ("vcpus", serde_json::json!(9)),
        ("memory-bytes", serde_json::json!(1024)),
        ("memory-bytes", serde_json::json!(100_000_000)),
        ("platform", serde_json::json!("unknown")),
        ("bootargs", serde_json::json!("a\0b")),
        ("extra", serde_json::json!(true)),
        ("affinity", serde_json::json!(null)),
        ("affinity", serde_json::json!([{"vcpu": 2, "cpus": [0]}])),
        ("affinity", serde_json::json!([{"vcpu": 0, "cpus": []}])),
        ("affinity", serde_json::json!([{"vcpu": 0, "cpus": [256]}])),
        ("affinity", serde_json::json!([{"vcpu": 0, "cpus": [-1]}])),
        ("affinity", serde_json::json!([{"vcpu": 0, "cpus": [true]}])),
        (
            "affinity",
            serde_json::json!([{"vcpu": 0, "cpus": [0], "extra": 1}]),
        ),
    ] {
        let mut config = serde_json::to_value(configuration()).unwrap();
        config[field] = value;
        let parsed = serde_json::from_value::<Configuration>(config);
        assert!(parsed.is_err() || parsed.unwrap().image_configuration().is_err());
    }
}

#[test]
fn runtime_arguments_preserve_policy_and_long_command_lines() {
    let mut config = configuration();
    config.bootargs = "\"\\".repeat(1024);
    let args = config.runtime_arguments().unwrap();
    assert_eq!(
        Configuration::from_runtime_arguments(args.clone()).unwrap(),
        config
    );
    assert!(Configuration::from_runtime_arguments(args[..2].to_vec()).is_err());
    let mut extra = args.to_vec();
    extra.push("unexpected".into());
    assert!(Configuration::from_runtime_arguments(extra).is_err());
    // No architecture is carried in the runtime's three policy arguments.
    assert_eq!(args.len(), 3);
}

#[test]
fn default_affinity_survives_config_and_bounded_runtime_transport() {
    let mut config = configuration();
    config.vcpus = 8;
    config.bootargs = "\"\\".repeat(1024);
    config.affinity = (0..8)
        .map(|vcpu| Affinity {
            vcpu,
            cpus: (0..crate::affinity::MAX_HOST_CPUS as u32).collect(),
        })
        .collect();
    let args = config.runtime_arguments().unwrap();
    assert!(args.iter().all(|arg| arg.len() < 4096));
    assert!(args.iter().map(|arg| arg.len() + 1).sum::<usize>() < 16 * 1024);
    assert_eq!(Configuration::from_runtime_arguments(args).unwrap(), config);
    let fleet = Config::parse(
        &serde_json::to_vec(&serde_json::json!({
            "format":"hyper.vm-config", "virtual-machines":[{
                "name":"vm", "image":"/vm/test.itb", "configuration": config
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        Config::parse(&fleet.to_bytes().unwrap()).unwrap().machines[0].configuration,
        config
    );
}

#[test]
fn checked_in_vm_configurations_are_complete() {
    for bytes in [
        include_bytes!("../../../app/init/tests/config/vms.json").as_slice(),
        include_bytes!("../../../app/init/tests/config/vms-riscv64.json").as_slice(),
        include_bytes!("../../../app/init/tests/config/vms-io.json").as_slice(),
        include_bytes!("../../../app/init/tests/config/vms-power-crash.json").as_slice(),
        include_bytes!("../../../app/init/tests/config/vms-smp-4.json").as_slice(),
        include_bytes!("../../../app/init/tests/config/vms-smp-8.json").as_slice(),
    ] {
        let config = Config::parse(bytes).unwrap();
        for definition in config.machines {
            let arguments = definition.configuration.runtime_arguments().unwrap();
            assert_eq!(
                Configuration::from_runtime_arguments(arguments).unwrap(),
                definition.configuration
            );
        }
    }
}
