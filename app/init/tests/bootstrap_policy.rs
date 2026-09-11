// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::BootstrapPolicy;
use crate::manifest::{self, AuthorityPolicy, ValidationErrorKind};
use hyper_os::handle::Rights;

const POLICY_SERVICE: &str = r#"{
  "format": "hyper.service-manifest",
  "services": [{
    "name": "policy", "image": "/svc/power-policy", "critical": true,
    "restart": "never", "after": [], "capabilities": [
      {"source":"bootstrap.service-output-channel", "purpose":"stdio.output",
       "operation":"duplicate", "rights":["wait","write"]},
      {"source":"bootstrap.root-directory", "purpose":"process.root-directory",
       "operation":"duplicate", "rights":["read"]},
      {"source":"bootstrap.cpu-inspector", "purpose":"process.cpu-inspector",
       "operation":"duplicate", "rights":["inspect"]}
    ]
  }]
}"#;

#[test]
fn ordinary_policy_service_gets_only_explicit_attenuated_authority() -> Result<(), String> {
    let manifest = manifest::parse(POLICY_SERVICE).map_err(|e| format!("{e:?}"))?;
    let plan = manifest::validate(&manifest, &BootstrapPolicy).map_err(|e| format!("{e:?}"))?;
    for (index, rights) in [
        Rights::WAIT.union(Rights::WRITE),
        Rights::READ,
        Rights::INSPECT,
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            plan.capability_grant(0, index).map(|grant| grant.rights()),
            Some(rights.bits())
        );
    }
    assert!(plan.capability_grant(0, 3).is_none());
    Ok(())
}

#[test]
fn existing_installed_manifests_still_validate_against_real_bootstrap_policy() -> Result<(), String>
{
    for text in [
        include_str!("../config/services.json"),
        include_str!("../config/services-native.json"),
    ] {
        let manifest = manifest::parse(text).map_err(|e| format!("{e:?}"))?;
        manifest::validate(&manifest, &BootstrapPolicy).map_err(|e| format!("{e:?}"))?;
    }
    Ok(())
}

#[test]
fn common_contracts_do_not_expose_private_service_roles_or_weaken_required_rights()
-> Result<(), String> {
    for purpose in [
        "console.system",
        "vm.provisioning",
        "vm.creation-authority",
        "session.client-input",
    ] {
        assert!(
            BootstrapPolicy
                .startup_purpose("/svc/power-policy", purpose)
                .is_none()
        );
    }
    for (old, new, expected) in [
        (
            "[\"wait\",\"write\"]",
            "[\"wait\"]",
            ValidationErrorKind::MissingRequiredRights,
        ),
        (
            "[\"wait\",\"write\"]",
            "[\"wait\",\"write\",\"read\"]",
            ValidationErrorKind::RightsEscalation,
        ),
    ] {
        let text = POLICY_SERVICE.replace(old, new);
        let manifest = manifest::parse(&text).map_err(|e| format!("{e:?}"))?;
        assert_eq!(
            manifest::validate(&manifest, &BootstrapPolicy)
                .err()
                .map(|e| e.kind()),
            Some(expected)
        );
    }
    Ok(())
}
