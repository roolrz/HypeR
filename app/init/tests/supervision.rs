// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

extern crate std;

use hyper_service::vm::{InstanceEvent, InstanceFailure};

use super::{
    SupportError, TerminationAction, instance_termination_action, service_termination_action,
    validate,
};
use crate::manifest::parse;

fn manifest(critical: &str, restart: &str) -> std::string::String {
    std::format!(
        r#"{{"format":"hyper.service-manifest","services":[
          {{"name":"first","image":"/svc/first","critical":{critical},"restart":"{restart}","after":[],"capabilities":[]}},
          {{"name":"second","image":"/svc/second","critical":false,"restart":"never","after":[],"capabilities":[]}}
        ]}}"#
    )
}

#[test]
fn accepts_one_or_more_nonrestartable_critical_services() {
    let text = manifest("true", "never");
    let parsed = parse(&text);
    assert!(parsed.is_ok());
    let Ok(parsed) = parsed else {
        return;
    };
    assert_eq!(validate(&parsed), Ok(()));
    let text = text.replace(
        "\"name\":\"second\",\"image\":\"/svc/second\",\"critical\":false",
        "\"name\":\"second\",\"image\":\"/svc/second\",\"critical\":true",
    );
    let parsed = parse(&text);
    assert!(parsed.is_ok());
    let Ok(parsed) = parsed else {
        return;
    };
    assert_eq!(validate(&parsed), Ok(()));
}

#[test]
fn rejects_polling_or_ambiguous_supervision_graphs() {
    for (critical, restart, expected) in [
        ("false", "never", SupportError::MissingCriticalService),
        ("true", "always", SupportError::RestartPolicy),
    ] {
        let text = manifest(critical, restart);
        let parsed = parse(&text);
        assert!(parsed.is_ok());
        let Ok(parsed) = parsed else {
            continue;
        };
        assert_eq!(validate(&parsed), Err(expected));
    }
}

#[test]
fn termination_policy_keeps_noncritical_services_and_clean_vms_nonfatal() {
    assert_eq!(
        service_termination_action(false),
        TerminationAction::Continue
    );
    assert_eq!(
        service_termination_action(true),
        TerminationAction::FailSystem
    );
    assert_eq!(
        instance_termination_action(InstanceEvent::Stopped),
        TerminationAction::Continue
    );
    assert_eq!(
        instance_termination_action(InstanceEvent::Failed(InstanceFailure::Runtime)),
        TerminationAction::FailSystem
    );
}
