// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

extern crate std;

use std::fmt::Write;
use std::string::String;

use super::{
    AuthorityDeclaration, AuthorityPolicy, MAX_MANIFEST_BYTES, MAX_SERVICES, ParseErrorKind,
    StartupPurposeDeclaration, ValidationErrorKind, parse, validate,
};

const VALID: &str = r#"
{
  "format": "hyper.service-manifest",
  "services": [
    {
      "name": "vmm",
      "image": "/svc/vmm",
      "critical": false,
      "restart": "on-failure",
      "after": ["network"],
      "capabilities": [
        {
          "source": "network.primary",
          "purpose": "vmm.network-primary",
          "operation": "duplicate",
          "rights": ["read", "write"]
        }
      ]
    },
    {
      "name": "network",
      "image": "/svc/network-manager",
      "critical": true,
      "restart": "always",
      "after": [],
      "capabilities": []
    },
    {
      "name": "session",
      "image": "/svc/session-manager",
      "critical": true,
      "restart": "on-failure",
      "after": [],
      "capabilities": [
        {
          "source": "bootstrap.console-manager",
          "purpose": "session.console-manager",
          "operation": "create",
          "rights": ["inspect"]
        }
      ]
    }
  ]
}
"#;

struct Policy;

impl AuthorityPolicy for Policy {
    fn authority<'policy>(&'policy self, source: &str) -> Option<AuthorityDeclaration<'policy>> {
        match source {
            "network.primary" => Some(AuthorityDeclaration {
                provider: Some("network"),
                object_kind: 1,
                rights: 0b11,
                movable: true,
                duplicable: true,
                creatable: false,
            }),
            "bootstrap.console-manager" => Some(AuthorityDeclaration {
                provider: None,
                object_kind: 2,
                rights: 0b100,
                movable: true,
                duplicable: true,
                creatable: true,
            }),
            "bootstrap.console" => Some(AuthorityDeclaration {
                provider: None,
                object_kind: 3,
                rights: 0b1011,
                movable: false,
                duplicable: true,
                creatable: false,
            }),
            "bootstrap.console-input-channel"
            | "bootstrap.console-output-channel"
            | "bootstrap.session-input-channel"
            | "bootstrap.session-output-channel"
            | "bootstrap.session-client-input-channel"
            | "bootstrap.session-client-output-channel"
            | "bootstrap.session-client-error-channel"
            | "bootstrap.shell-input-channel"
            | "bootstrap.shell-output-channel"
            | "bootstrap.shell-error-channel" => Some(AuthorityDeclaration {
                provider: None,
                object_kind: 4,
                rights: 0b1011,
                movable: true,
                duplicable: false,
                creatable: false,
            }),
            "bootstrap.root-directory" => Some(AuthorityDeclaration {
                provider: None,
                object_kind: 5,
                rights: 0b10_0000_0001,
                movable: false,
                duplicable: true,
                creatable: false,
            }),
            "bootstrap.task-factory" => Some(AuthorityDeclaration {
                provider: None,
                object_kind: 6,
                rights: 0b001_0000,
                movable: false,
                duplicable: true,
                creatable: false,
            }),
            "bootstrap.task-group" => Some(AuthorityDeclaration {
                provider: None,
                object_kind: 7,
                rights: 0b010_0000,
                movable: false,
                duplicable: true,
                creatable: false,
            }),
            "bootstrap.resource-domain" => Some(AuthorityDeclaration {
                provider: None,
                object_kind: 8,
                rights: 0b100_0000,
                movable: false,
                duplicable: true,
                creatable: false,
            }),
            "bootstrap.task-inspector" => Some(AuthorityDeclaration {
                provider: None,
                object_kind: 16,
                rights: 0b1_1000_0100,
                movable: false,
                duplicable: true,
                creatable: false,
            }),
            "bootstrap.object-inspector" => Some(AuthorityDeclaration {
                provider: None,
                object_kind: 17,
                rights: 0b1_1000_0100,
                movable: false,
                duplicable: true,
                creatable: false,
            }),
            _ => None,
        }
    }

    fn startup_purpose(&self, image: &str, name: &str) -> Option<StartupPurposeDeclaration> {
        let (value, object_kind) = match (image, name) {
            ("/svc/vmm", "vmm.network-primary") => (100, 1),
            ("/svc/session-manager", "session.console-manager")
            | ("/svc/session-manager", "session.console-manager-alias") => (100, 2),
            ("/svc/network-manager", "network.console-manager") => (101, 2),
            ("/svc/console-input" | "/svc/console-output", "console.system") => (200, 3),
            ("/svc/console-input" | "/svc/console-output", "console.data") => (201, 4),
            ("/svc/session", "session.console-input") => (202, 4),
            ("/svc/session", "session.console-output") => (203, 4),
            ("/svc/session", "session.client-input") => (204, 4),
            ("/svc/session", "session.client-output") => (205, 4),
            ("/svc/session", "session.client-error") => (206, 4),
            ("/bin/sh", "stdio.input") => (300, 4),
            ("/bin/sh", "stdio.output") => (301, 4),
            ("/bin/sh", "stdio.error") => (302, 4),
            ("/bin/sh", "process.root-directory") => (303, 5),
            ("/bin/sh", "process.task-factory") => (304, 6),
            ("/bin/sh", "process.task-group") => (305, 7),
            ("/bin/sh", "process.resource-domain") => (306, 8),
            ("/bin/sh", "process.task-inspector") => (307, 16),
            ("/bin/sh", "process.object-inspector") => (308, 17),
            _ => return None,
        };
        Some(StartupPurposeDeclaration { value, object_kind })
    }

    fn right(&self, name: &str) -> Option<u64> {
        match name {
            "read" => Some(0b001),
            "write" => Some(0b010),
            "inspect" => Some(0b100),
            "wait" => Some(0b1000),
            "create-process" => Some(0b001_0000),
            "attach-process" => Some(0b010_0000),
            "sponsor" => Some(0b100_0000),
            "duplicate" => Some(0b1000_0000),
            "transfer" => Some(0b1_0000_0000),
            "execute" => Some(0b10_0000_0000),
            _ => None,
        }
    }
}

#[test]
fn production_manifest_matches_the_validated_schema() {
    let parsed = parse(include_str!("../../../config/services.json"));
    assert!(parsed.is_ok());
    let Ok(manifest) = parsed else {
        return;
    };
    let validated = validate(&manifest, &Policy);
    assert!(validated.is_ok());
    assert_eq!(manifest.service_count(), 4);
}

#[test]
fn parses_and_plans_a_complete_manifest_before_launch() {
    let parsed = parse(VALID);
    assert!(parsed.is_ok());
    let Ok(manifest) = parsed else {
        return;
    };
    let validated = validate(&manifest, &Policy);
    assert!(validated.is_ok());
    let Ok(plan) = validated else {
        return;
    };
    assert_eq!(manifest.service_count(), 3);
    assert_eq!(plan.service_count(), 3);
    assert_eq!(plan.service_index(0), Some(1));
    assert_eq!(plan.service_index(1), Some(0));
    assert_eq!(plan.service_index(2), Some(2));
    assert_eq!(plan.capability_rights(0, 0), Some(0b11));
    assert_eq!(plan.capability_kind(2, 0), Some(2));
    assert_eq!(plan.capability_purpose(0, 0), Some(100));
    assert_eq!(plan.capability_purpose(2, 0), Some(100));
}

#[test]
fn rejects_unknown_and_duplicate_fields() {
    let unknown = VALID.replace("\"critical\": false", "\"enabled\": false");
    let parsed = parse(&unknown);
    assert!(parsed.is_err());
    let Err(error) = parsed else {
        return;
    };
    assert_eq!(error.kind(), ParseErrorKind::UnknownField);
    assert_eq!(error.offset(), unknown.find("\"enabled\"").unwrap_or(0));

    let duplicate = VALID.replace(
        "\"critical\": false,",
        "\"critical\": false, \"critical\": false,",
    );
    assert_eq!(
        parse(&duplicate).map_err(|error| error.kind()),
        Err(ParseErrorKind::DuplicateField)
    );
}

#[test]
fn rejects_noncanonical_escaped_strings() {
    let escaped = VALID.replace("\"vmm\"", "\"v\\u006d\"");
    assert_eq!(
        parse(&escaped).map_err(|error| error.kind()),
        Err(ParseErrorKind::EscapeNotAllowed)
    );
}

#[test]
fn enforces_manifest_and_service_count_bounds() {
    let oversized = " ".repeat(MAX_MANIFEST_BYTES + 1);
    assert_eq!(
        parse(&oversized).map_err(|error| error.kind()),
        Err(ParseErrorKind::TooLarge)
    );

    let mut many = String::from("{\"format\":\"hyper.service-manifest\",\"services\":[");
    for index in 0..=MAX_SERVICES {
        if index != 0 {
            many.push(',');
        }
        let write_result = write!(
            many,
            "{{\"name\":\"s{index}\",\"image\":\"/svc/s{index}\",\"critical\":false,\"restart\":\"never\",\"after\":[],\"capabilities\":[]}}"
        );
        assert!(write_result.is_ok());
    }
    many.push_str("]}");
    assert_eq!(
        parse(&many).map_err(|error| error.kind()),
        Err(ParseErrorKind::TooManyServices)
    );
}

#[test]
fn rejects_dependency_cycles() {
    let cyclic = r#"{
      "format":"hyper.service-manifest",
      "services":[
        {"name":"a","image":"/svc/a","critical":true,"restart":"never","after":["b"],"capabilities":[]},
        {"name":"b","image":"/svc/b","critical":true,"restart":"never","after":["a"],"capabilities":[]}
      ]
    }"#;
    let parsed = parse(cyclic);
    assert!(parsed.is_ok());
    let Ok(manifest) = parsed else {
        return;
    };
    assert_eq!(
        validate(&manifest, &Policy).map_err(|error| error.kind()),
        Err(ValidationErrorKind::DependencyCycle)
    );
}

#[test]
fn rejects_rights_escalation() {
    let escalated = VALID.replace(
        "\"rights\": [\"inspect\"]",
        "\"rights\": [\"inspect\", \"write\"]",
    );
    let parsed = parse(&escalated);
    assert!(parsed.is_ok());
    let Ok(manifest) = parsed else {
        return;
    };
    assert_eq!(
        validate(&manifest, &Policy).map_err(|error| error.kind()),
        Err(ValidationErrorKind::RightsEscalation)
    );
}

#[test]
fn rejects_hidden_provider_dependencies() {
    let hidden = VALID.replace("\"after\": [\"network\"]", "\"after\": []");
    let parsed = parse(&hidden);
    assert!(parsed.is_ok());
    let Ok(manifest) = parsed else {
        return;
    };
    assert_eq!(
        validate(&manifest, &Policy).map_err(|error| error.kind()),
        Err(ValidationErrorKind::MissingProviderDependency)
    );
}

#[test]
fn rejects_one_shot_authority_for_a_restartable_service() {
    let one_shot = VALID.replace("\"operation\": \"create\"", "\"operation\": \"move\"");
    let parsed = parse(&one_shot);
    assert!(parsed.is_ok());
    let Ok(manifest) = parsed else {
        return;
    };
    assert_eq!(
        validate(&manifest, &Policy).map_err(|error| error.kind()),
        Err(ValidationErrorKind::RestartConsumesAuthority)
    );
}

#[test]
fn rejects_reuse_of_a_moved_authority() {
    let repeated = VALID
        .replace(
            "\"capabilities\": []",
            r#""capabilities": [{
          "source":"bootstrap.console-manager",
          "purpose":"network.console-manager",
          "operation":"duplicate",
          "rights":["inspect"]
        }]"#,
        )
        .replace("\"operation\": \"create\"", "\"operation\": \"move\"");
    let parsed = parse(&repeated);
    assert!(parsed.is_ok());
    let Ok(manifest) = parsed else {
        return;
    };
    assert_eq!(
        validate(&manifest, &Policy).map_err(|error| error.kind()),
        Err(ValidationErrorKind::MoveSourceReused)
    );
}

#[test]
fn rejects_invalid_and_duplicate_startup_purposes() {
    let invalid = VALID.replacen(
        "\"purpose\": \"vmm.network-primary\"",
        "\"purpose\": \"0bad\"",
        1,
    );
    let parsed = parse(&invalid);
    assert!(parsed.is_ok());
    let Ok(manifest) = parsed else {
        return;
    };
    assert_eq!(
        validate(&manifest, &Policy).map_err(|error| error.kind()),
        Err(ValidationErrorKind::InvalidPurposeName)
    );

    let duplicate = VALID.replace(
        r#""rights": ["inspect"]"#,
        r#""rights": ["inspect"]
        }, {
          "source": "bootstrap.console-manager",
          "purpose": "session.console-manager-alias",
          "operation": "duplicate",
          "rights": ["inspect"]"#,
    );
    let parsed = parse(&duplicate);
    assert!(parsed.is_ok());
    let Ok(manifest) = parsed else {
        return;
    };
    assert_eq!(
        validate(&manifest, &Policy).map_err(|error| error.kind()),
        Err(ValidationErrorKind::DuplicateCapabilityPurpose)
    );

    let unknown = VALID.replacen(
        "\"purpose\": \"vmm.network-primary\"",
        "\"purpose\": \"vmm.unknown\"",
        1,
    );
    let parsed = parse(&unknown);
    assert!(parsed.is_ok());
    let Ok(manifest) = parsed else {
        return;
    };
    assert_eq!(
        validate(&manifest, &Policy).map_err(|error| error.kind()),
        Err(ValidationErrorKind::UnknownCapabilityPurpose)
    );
}
