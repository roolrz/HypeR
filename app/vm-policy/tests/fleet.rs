// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;
#[test]
fn rejects_ambiguous_names_paths_and_duplicate_definitions() {
    let valid = Definition {
        name: "alpine".into(),
        image: "/vm/alpine.itb".into(),
        autostart: false,
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
