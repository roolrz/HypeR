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
        include_str!("../config/services-io.json"),
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
        "io.device-authority",
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

const IO_READY_SERVICE: &str = r#"{
  "format": "hyper.service-manifest",
  "services": [{
    "name": "storage", "image": "/svc/io-runtime", "critical": true,
    "restart": "never", "after": [], "capabilities": [
      {"source":"bootstrap.io-ready-channel", "purpose":"io.ready",
       "operation":"move", "rights":["wait","write"]}
    ]
  }]
}"#;

#[test]
fn storage_readiness_requires_the_dedicated_moved_authority() -> Result<(), String> {
    let manifest = manifest::parse(IO_READY_SERVICE).map_err(|e| format!("{e:?}"))?;
    let plan = manifest::validate(&manifest, &BootstrapPolicy).map_err(|e| format!("{e:?}"))?;
    assert_eq!(super::io_ready_service(&plan), Ok(Some(0)));
    for text in [
        IO_READY_SERVICE.replace(
            "bootstrap.io-ready-channel",
            "bootstrap.shell-output-channel",
        ),
        IO_READY_SERVICE.replace("io.ready", "stdio.output"),
    ] {
        let manifest = manifest::parse(&text).map_err(|e| format!("{e:?}"))?;
        let plan = manifest::validate(&manifest, &BootstrapPolicy).map_err(|e| format!("{e:?}"))?;
        assert_eq!(
            super::io_ready_service(&plan),
            Err(super::IoReadyPlanError::InvalidGrant)
        );
    }
    Ok(())
}

#[test]
fn legacy_manifests_do_not_wait_for_storage() -> Result<(), String> {
    for text in [
        include_str!("../config/services.json"),
        include_str!("../config/services-native.json"),
        include_str!("../config/services-io.json"),
    ] {
        let manifest = manifest::parse(text).map_err(|e| format!("{e:?}"))?;
        let plan = manifest::validate(&manifest, &BootstrapPolicy).map_err(|e| format!("{e:?}"))?;
        assert_eq!(super::io_ready_service(&plan), Ok(None));
    }
    assert!(
        BootstrapPolicy
            .startup_purpose("/svc/power-policy", "io.ready")
            .is_none()
    );
    Ok(())
}

#[test]
fn io_broker_requires_both_unique_bootstrap_endpoints() -> Result<(), String> {
    const MANIFEST: &str = r#"{"format":"hyper.service-manifest","services":[
      {"name":"storage","image":"/svc/io-runtime","critical":true,"restart":"never","after":[],"capabilities":[
        {"source":"bootstrap.io-broker-server","purpose":"io.broker-server","operation":"move","rights":["wait","read"]}]},
      {"name":"manager","image":"/svc/vm-manager","critical":true,"restart":"never","after":[],"capabilities":[
        {"source":"bootstrap.io-broker-client","purpose":"io.broker-client","operation":"move","rights":["wait","write"]}]}
    ]}"#;
    let manifest = manifest::parse(MANIFEST).map_err(|e| format!("{e:?}"))?;
    let plan = manifest::validate(&manifest, &BootstrapPolicy).map_err(|e| format!("{e:?}"))?;
    assert_eq!(super::io_broker_enabled(&plan), Ok(true));
    let one_sided = MANIFEST.replace(
        r#"{"source":"bootstrap.io-broker-client","purpose":"io.broker-client","operation":"move","rights":["wait","write"]}"#, "");
    let manifest = manifest::parse(&one_sided).map_err(|e| format!("{e:?}"))?;
    let plan = manifest::validate(&manifest, &BootstrapPolicy).map_err(|e| format!("{e:?}"))?;
    assert!(super::io_broker_enabled(&plan).is_err());
    Ok(())
}

#[test]
fn terminal_alias_can_duplicate_before_move_but_never_after_consumption() -> Result<(), String> {
    const INPUT: &str = r#"{"format":"hyper.service-manifest","services":[{"name":"shell","image":"/bin/sh","critical":true,"restart":"never","after":[],"capabilities":[
{"source":"bootstrap.shell-input-channel","purpose":"stdio.terminal-input","operation":"duplicate","rights":["wait","read","duplicate","transfer","inspect"]},
{"source":"bootstrap.shell-input-channel","purpose":"stdio.input","operation":"move","rights":["wait","read","duplicate","transfer","inspect"]}]}]}"#;
    let parsed = manifest::parse(INPUT).map_err(|e| format!("{e:?}"))?;
    manifest::validate(&parsed, &BootstrapPolicy).map_err(|e| format!("{e:?}"))?;
    let reversed = INPUT
        .replace("\"operation\":\"duplicate\"", "\"operation\":\"temporary\"")
        .replace("\"operation\":\"move\"", "\"operation\":\"duplicate\"")
        .replace("\"operation\":\"temporary\"", "\"operation\":\"move\"");
    let parsed = manifest::parse(&reversed).map_err(|e| format!("{e:?}"))?;
    assert!(
        matches!(manifest::validate(&parsed, &BootstrapPolicy), Err(error)
        if error.kind() == manifest::ValidationErrorKind::MoveSourceReused)
    );
    Ok(())
}
