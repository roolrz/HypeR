// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Policy mechanisms for the `HypeR` Native init supervisor.
//!
//! Parsing and planning deliberately depend on no operating-system binding.
//! The runtime adapter must obtain real `BootFs` and process-construction
//! capabilities before it can apply a validated launch plan.

#![no_std]

pub mod console_contract;
pub mod manifest;
pub mod session_contract;
pub mod supervision;

use core::convert::Infallible;

use manifest::{AuthorityPolicy, LaunchPlan, Manifest, ParseError, ValidationError};

/// Read-only source for the complete service-manifest image.
///
/// The production implementation must borrow bytes from a real `BootFs` object;
/// embedding a second manifest in the init executable would bypass the boot
/// image's measured configuration.
pub trait ManifestSource {
    type Error;

    fn manifest(&self) -> Result<&str, Self::Error>;
}

/// Runtime owner which applies one fully validated launch plan.
///
/// `LaunchPlan` contains declarations only. An implementation must resolve
/// every live source handle again, prepare every `ProcessBuilder`, and let the
/// kernel revalidate type and attenuated rights at the final commit.
pub trait ServiceGraphLauncher: AuthorityPolicy {
    type Error;

    fn launch(
        &mut self,
        manifest: &Manifest<'_>,
        plan: &LaunchPlan,
    ) -> Result<Infallible, Self::Error>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootstrapError<SourceError, LaunchError> {
    Source(SourceError),
    Parse(ParseError),
    Validate(ValidationError),
    Launch(LaunchError),
}

/// Parses and validates the whole graph before crossing the launch boundary.
#[inline(never)]
pub fn bootstrap<Source, Launcher>(
    source: &Source,
    launcher: &mut Launcher,
) -> Result<Infallible, BootstrapError<Source::Error, Launcher::Error>>
where
    Source: ManifestSource,
    Launcher: ServiceGraphLauncher,
{
    let text = source.manifest().map_err(BootstrapError::Source)?;
    let mut manifest = Manifest::empty();
    manifest::parse_into(text, &mut manifest).map_err(BootstrapError::Parse)?;
    let plan = manifest::validate(&manifest, launcher).map_err(BootstrapError::Validate)?;
    launcher
        .launch(&manifest, &plan)
        .map_err(BootstrapError::Launch)
}

#[cfg(test)]
mod tests {
    use core::cell::Cell;
    use core::convert::Infallible;

    use super::manifest::{AuthorityDeclaration, AuthorityPolicy, LaunchPlan, Manifest};
    use super::{BootstrapError, ManifestSource, ServiceGraphLauncher, bootstrap};

    const EMPTY_GRAPH: &str = r#"{"format":"hyper.service-manifest","services":[]}"#;
    const ONE_SERVICE: &str = r#"{
        "format":"hyper.service-manifest",
        "services":[{
            "name":"session",
            "image":"/svc/session-manager",
            "critical":true,
            "restart":"on-failure",
            "after":[],
            "capabilities":[]
        }]
    }"#;

    struct Source(&'static str);

    impl ManifestSource for Source {
        type Error = Infallible;

        fn manifest(&self) -> Result<&str, Self::Error> {
            Ok(self.0)
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct LaunchStopped;

    struct Launcher {
        launches: Cell<usize>,
    }

    impl AuthorityPolicy for Launcher {
        fn authority<'policy>(
            &'policy self,
            _source: &str,
        ) -> Option<AuthorityDeclaration<'policy>> {
            None
        }

        fn object_kind(&self, _name: &str) -> Option<u32> {
            None
        }

        fn right(&self, _name: &str) -> Option<u64> {
            None
        }
    }

    impl ServiceGraphLauncher for Launcher {
        type Error = LaunchStopped;

        fn launch(
            &mut self,
            _manifest: &Manifest<'_>,
            _plan: &LaunchPlan,
        ) -> Result<Infallible, Self::Error> {
            self.launches.set(self.launches.get() + 1);
            Err(LaunchStopped)
        }
    }

    #[test]
    fn validation_failure_never_crosses_the_launch_boundary() {
        let mut launcher = Launcher {
            launches: Cell::new(0),
        };
        let result = bootstrap(&Source(EMPTY_GRAPH), &mut launcher);
        assert!(matches!(result, Err(BootstrapError::Validate(_))));
        assert_eq!(launcher.launches.get(), 0);
    }

    #[test]
    fn a_validated_plan_reaches_the_launch_boundary_once() {
        let mut launcher = Launcher {
            launches: Cell::new(0),
        };
        let result = bootstrap(&Source(ONE_SERVICE), &mut launcher);
        assert_eq!(result, Err(BootstrapError::Launch(LaunchStopped)));
        assert_eq!(launcher.launches.get(), 1);
    }
}
