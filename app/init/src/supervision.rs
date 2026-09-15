// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Admission policy for the currently implemented service supervisor.

use crate::manifest::{Manifest, RestartPolicy};
use hyper_service::vm::{BootEvent, InstanceEvent};

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

/// Applies init's policy when boot-time fleet supervision completes.
#[must_use]
pub const fn boot_event_action(event: BootEvent) -> TerminationAction {
    match event {
        BootEvent::NoAutostart | BootEvent::InstanceTerminated(InstanceEvent::Stopped) => {
            TerminationAction::Continue
        }
        BootEvent::InstanceTerminated(InstanceEvent::Failed(_)) => TerminationAction::FailSystem,
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
