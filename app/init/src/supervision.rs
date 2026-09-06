// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Admission policy for the currently implemented service supervisor.

use crate::manifest::{Manifest, RestartPolicy};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupportError {
    RestartPolicy,
    CriticalServiceCount,
}

/// Validates only the supervision behavior implemented by init today.
pub fn validate(manifest: &Manifest<'_>) -> Result<(), SupportError> {
    if manifest
        .services()
        .any(|service| service.restart() != RestartPolicy::Never)
    {
        return Err(SupportError::RestartPolicy);
    }
    if manifest
        .services()
        .filter(|service| service.critical())
        .count()
        != 1
    {
        return Err(SupportError::CriticalServiceCount);
    }
    Ok(())
}

/// Returns the unique critical service after successful validation.
pub fn critical_service_index(manifest: &Manifest<'_>) -> Option<usize> {
    manifest
        .services()
        .enumerate()
        .find_map(|(index, service)| service.critical().then_some(index))
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::{SupportError, critical_service_index, validate};
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
    fn accepts_exactly_one_nonrestartable_critical_service() {
        let text = manifest("true", "never");
        let parsed = parse(&text);
        assert!(parsed.is_ok());
        let Ok(parsed) = parsed else {
            return;
        };
        assert_eq!(validate(&parsed), Ok(()));
        assert_eq!(critical_service_index(&parsed), Some(0));
    }

    #[test]
    fn rejects_polling_or_ambiguous_supervision_graphs() {
        for (critical, restart, expected) in [
            ("false", "never", SupportError::CriticalServiceCount),
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

        let text = manifest("true", "never").replace(
            "\"name\":\"second\",\"image\":\"/svc/second\",\"critical\":false",
            "\"name\":\"second\",\"image\":\"/svc/second\",\"critical\":true",
        );
        let parsed = parse(&text);
        assert!(parsed.is_ok());
        let Ok(parsed) = parsed else {
            return;
        };
        assert_eq!(validate(&parsed), Err(SupportError::CriticalServiceCount));
    }
}
