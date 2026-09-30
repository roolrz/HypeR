// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! The resident I/O VM and its device come from one bounded board snapshot.

use crate::device_policy::Policy;
use hyper_vm_policy::fleet::{Configuration, Definition};
use serde::Deserialize;
use std::io::Read;

const MAX_BOARD_BYTES: usize = 1024 * 1024;
const RESIDENT_RAM_BYTES: u64 = 128 * 1024 * 1024;

// The image builder validates the complete board schema. This service owns the
// I/O node; unrelated disk, deployment-file and business-VM fields stay opaque.
#[derive(Deserialize)]
struct Board {
    format: String,
    architecture: String,
    #[serde(rename = "io-vm")]
    io_vm: IoVm,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IoVm {
    runtime: Runtime,
    name: String,
    image: String,
    configuration: Configuration,
    #[serde(rename = "io-device")]
    device: Policy,
}

#[derive(Deserialize)]
enum Runtime {
    #[serde(rename = "io-runtime")]
    IoRuntime,
}

pub struct Config {
    definition: Definition,
    device: Policy,
}

impl Config {
    pub fn load(path: &str) -> Result<Self, String> {
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .map_err(|error| error.to_string())?
            .take(MAX_BOARD_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        Self::parse(&bytes)
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_BOARD_BYTES {
            return Err("board configuration exceeds 1 MiB".into());
        }
        let board: Board = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        if board.format != "hyper.board.v1" || board.architecture != "aarch64" {
            return Err("unsupported board format or architecture".into());
        }
        let IoVm {
            runtime: Runtime::IoRuntime,
            name,
            image,
            configuration,
            device,
        } = board.io_vm;
        let definition = Definition {
            name,
            image,
            configuration,
            autostart: false,
            disk: None,
        };
        definition.validate()?;
        // The backend must boot before /data exists. Its image is packaged in
        // the bootstrap archive, independently of the configuration volume.
        if !definition.image.starts_with("/vm/")
            || !definition.image.ends_with(".itb")
            || definition.image.contains('\\')
            || definition
                .image
                .split('/')
                .skip(1)
                .any(|part| matches!(part, "" | "." | ".."))
        {
            return Err("I/O VM image must be a canonical /vm/...itb bootstrap path".into());
        }
        if definition.configuration.memory_bytes != RESIDENT_RAM_BYTES
            || definition.configuration.vcpus != 1
        {
            return Err("resident I/O VM requires 128 MiB and one vCPU".into());
        }
        Ok(Self { definition, device })
    }

    pub fn definition(&self) -> &Definition {
        &self.definition
    }

    pub fn device(&self) -> &Policy {
        &self.device
    }
}

#[cfg(test)]
#[path = "../tests/config.rs"]
mod tests;
