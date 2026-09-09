// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Admission policy for the currently implemented service supervisor.

use crate::manifest::{Manifest, RestartPolicy};
use hyper_service::vm::InstanceEvent;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupportError {
    RestartPolicy,
    MissingCriticalService,
}

/// System-level action selected after one supervised entity terminates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminationAction {
    Continue,
    FailSystem,
}

/// Applies the manifest's criticality policy to a service exit.
#[must_use]
pub const fn service_termination_action(critical: bool) -> TerminationAction {
    if critical {
        TerminationAction::FailSystem
    } else {
        TerminationAction::Continue
    }
}

/// Applies init's initial-VM policy to a terminal instance event.
#[must_use]
pub const fn instance_termination_action(event: InstanceEvent) -> TerminationAction {
    match event {
        InstanceEvent::Stopped => TerminationAction::Continue,
        InstanceEvent::Failed(_) => TerminationAction::FailSystem,
    }
}

/// Validates only the supervision behavior implemented by init today.
pub fn validate(manifest: &Manifest<'_>) -> Result<(), SupportError> {
    if manifest
        .services()
        .any(|service| service.restart() != RestartPolicy::Never)
    {
        return Err(SupportError::RestartPolicy);
    }
    if !manifest.services().any(|service| service.critical()) {
        return Err(SupportError::MissingCriticalService);
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/supervision.rs"]
mod tests;
