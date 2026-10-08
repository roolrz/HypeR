// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn lifecycle_wait_requires_the_selected_vm_and_completed_state() -> Result<(), String> {
    let waiting = Completion {
        name: "alpine".into(),
        target: State::Running,
        timeout: Duration::from_secs(30),
    };
    let response = |state: &str, name: &str| {
        hyper_vm_policy::fleet::response(format!(
        r#"{{"result":"entries","machines":[{{"name":"{name}","state":"{state}","image":"/vm/a.itb","autostart":false}}]}}"#
    ).as_bytes())
    };
    for state in ["stopped", "stopping", "starting"] {
        assert!(!waiting.observe(&response(state, "alpine")?)?);
    }
    assert!(waiting.observe(&response("running", "alpine")?)?);
    for state in ["failed", "unavailable"] {
        assert!(waiting.observe(&response(state, "alpine")?).is_err());
    }
    assert!(waiting.observe(&response("running", "other")?).is_err());
    assert!(waiting.observe(&Response::Accepted).is_err());
    assert!(
        waiting
            .observe(&Response::Entries { machines: vec![] })
            .is_err()
    );
    Ok(())
}
