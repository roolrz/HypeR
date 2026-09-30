// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! A physical device selector validated independently of board-file loading.

use hyper_os::device::{FirmwareIdentity, Profile};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(try_from = "Selector")]
pub struct Policy {
    profile: ProfileName,
    identity: Identity,
}

enum Identity {
    Compatible(String),
    Path(String),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selector {
    profile: ProfileName,
    #[serde(default, deserialize_with = "identity_field")]
    compatible: Option<String>,
    #[serde(default, deserialize_with = "identity_field")]
    path: Option<String>,
}

fn identity_field<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    // Absence selects the other identity field; an explicit null is malformed.
    String::deserialize(deserializer).map(Some)
}

#[derive(Deserialize)]
enum ProfileName {
    #[serde(rename = "virtio-mmio-scsi")]
    VirtioScsi,
    #[serde(rename = "bcm2712-sdhci")]
    Sdhci,
}

impl TryFrom<Selector> for Policy {
    type Error = String;

    fn try_from(selector: Selector) -> Result<Self, Self::Error> {
        let identity = match (selector.compatible, selector.path) {
            (Some(value), None) => Identity::Compatible(value),
            (None, Some(value))
                if value.starts_with('/')
                    && !value
                        .split('/')
                        .skip(1)
                        .any(|part| matches!(part, "" | "." | "..")) =>
            {
                Identity::Path(value)
            }
            _ => return Err("device requires exactly one canonical firmware identity".into()),
        };
        let (Identity::Compatible(text) | Identity::Path(text)) = &identity;
        if text.is_empty() || text.len() > 512 || text.bytes().any(|byte| byte <= 32 || byte == 127)
        {
            return Err("invalid firmware identity".into());
        }
        Ok(Self {
            profile: selector.profile,
            identity,
        })
    }
}

impl Policy {
    pub const fn profile(&self) -> Profile {
        match self.profile {
            ProfileName::VirtioScsi => Profile::VirtioMmioScsi,
            ProfileName::Sdhci => Profile::Userspace,
        }
    }

    pub fn identity(&self) -> FirmwareIdentity<'_> {
        match &self.identity {
            Identity::Compatible(value) => FirmwareIdentity::Compatible(value),
            Identity::Path(value) => FirmwareIdentity::FdtPath(value),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_unique_selector_required() {
        assert!(
            serde_json::from_str::<Policy>(
                r#"{"profile":"virtio-mmio-scsi","compatible":"virtio,mmio"}"#
            )
            .is_ok()
        );
        for identity in [
            r#""path":"relative""#,
            r#""path":"/soc/../x""#,
            r#""compatible":"x","path":"/x""#,
            r#""compatible":"""#,
            r#""compatible":"x","path":null"#,
            r#""compatible":null,"path":"/soc/device""#,
            r#""compatible":null"#,
            r#""path":null"#,
        ] {
            assert!(
                serde_json::from_str::<Policy>(&format!(
                    r#"{{"profile":"virtio-mmio-scsi",{identity}}}"#
                ))
                .is_err()
            );
        }
    }
}
