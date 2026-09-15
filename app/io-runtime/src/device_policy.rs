// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use hyper_os::device::{FirmwareIdentity, Profile};
use serde::Deserialize;
use std::io::Read;

#[derive(Deserialize)]
struct Board {
    #[serde(rename = "io-device")]
    device: Policy,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    profile: ProfileName,
    compatible: Option<String>,
    path: Option<String>,
}

#[derive(Deserialize)]
enum ProfileName {
    #[serde(rename = "virtio-mmio-scsi")]
    VirtioScsi,
    #[serde(rename = "bcm2712-sdhci")]
    Sdhci,
}

impl Policy {
    pub const fn profile(&self) -> Profile {
        match self.profile {
            ProfileName::VirtioScsi => Profile::VirtioMmioScsi,
            ProfileName::Sdhci => Profile::Userspace,
        }
    }

    pub fn identity(&self) -> FirmwareIdentity<'_> {
        match &self.compatible {
            Some(value) => FirmwareIdentity::Compatible(value),
            None => FirmwareIdentity::FdtPath(self.path.as_deref().unwrap_or_default()),
        }
    }
}

pub fn parse(document: &[u8]) -> Result<Policy, String> {
    let board: Board = serde_json::from_slice(document).map_err(|error| error.to_string())?;
    let policy = board.device;
    let text = match (&policy.compatible, &policy.path) {
        (Some(value), None) => value,
        (None, Some(value))
            if value.starts_with('/')
                && !value
                    .split('/')
                    .skip(1)
                    .any(|part| matches!(part, "" | "." | "..")) =>
        {
            value
        }
        _ => return Err("device requires exactly one canonical firmware identity".into()),
    };
    if text.is_empty() || text.len() > 512 || text.bytes().any(|byte| byte <= 32 || byte == 127) {
        return Err("invalid firmware identity".into());
    }
    Ok(policy)
}

pub fn load(path: &str) -> Result<Policy, String> {
    let mut data = Vec::new();
    std::fs::File::open(path)
        .map_err(|error| error.to_string())?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut data)
        .map_err(|error| error.to_string())?;
    if data.len() > 1024 * 1024 {
        return Err("board configuration exceeds 1 MiB".into());
    }
    parse(&data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_unique_selector_required() {
        assert!(
            parse(br#"{"io-device":{"profile":"virtio-mmio-scsi","compatible":"virtio,mmio"}}"#)
                .is_ok()
        );
        for identity in [
            r#""path":"relative""#,
            r#""path":"/soc/../x""#,
            r#""compatible":"x","path":"/x""#,
            r#""compatible":"""#,
        ] {
            assert!(
                parse(
                    format!(r#"{{"io-device":{{"profile":"virtio-mmio-scsi",{identity}}}}}"#)
                        .as_bytes()
                )
                .is_err()
            );
        }
    }
}
