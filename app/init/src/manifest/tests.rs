// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

extern crate std;

use std::fmt::Write;
use std::string::String;

use super::{
    AuthorityDeclaration, AuthorityPolicy, MAX_MANIFEST_BYTES, MAX_SERVICES, ParseErrorKind,
    ValidationErrorKind, parse, validate,
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
          "purpose": 100,
          "kind": "network-endpoint",
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
          "purpose": 100,
          "kind": "console-manager",
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
            | "bootstrap.session-output-channel" => Some(AuthorityDeclaration {
                provider: None,
                object_kind: 4,
                rights: 0b1011,
                movable: true,
                duplicable: false,
                creatable: false,
            }),
            _ => None,
        }
    }

    fn object_kind(&self, name: &str) -> Option<u32> {
        match name {
            "network-endpoint" => Some(1),
            "console-manager" => Some(2),
            "console" => Some(3),
            "byte-channel" => Some(4),
            _ => None,
        }
    }

    fn right(&self, name: &str) -> Option<u64> {
        match name {
            "read" => Some(0b001),
            "write" => Some(0b010),
            "inspect" => Some(0b100),
            "wait" => Some(0b1000),
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
    assert_eq!(manifest.service_count(), 3);
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
          "purpose":101,
          "kind":"console-manager",
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
    let zero = VALID.replacen("\"purpose\": 100", "\"purpose\": 0", 1);
    let parsed = parse(&zero);
    assert!(parsed.is_ok());
    let Ok(manifest) = parsed else {
        return;
    };
    assert_eq!(
        validate(&manifest, &Policy).map_err(|error| error.kind()),
        Err(ValidationErrorKind::InvalidCapabilityPurpose)
    );

    let duplicate = VALID.replace(
        r#""rights": ["inspect"]"#,
        r#""rights": ["inspect"]
        }, {
          "source": "bootstrap.console-manager",
          "purpose": 100,
          "kind": "console-manager",
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

    let overflow = VALID.replacen("\"purpose\": 100", "\"purpose\": 4294967296", 1);
    assert_eq!(
        parse(&overflow).map_err(|error| error.kind()),
        Err(ParseErrorKind::InvalidNumber)
    );
}
