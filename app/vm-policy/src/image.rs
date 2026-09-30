// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Admit only native guest architectures; ITB metadata owns image identity.

mod read_cache;

pub use read_cache::{CachedSource, Error as CachedReadError};

use crate::fleet::Configuration;
use hyper_vm_image::{Architecture, ConfiguredImage, GuestImage, ReadAt};

pub fn configure(image: GuestImage, config: &Configuration) -> Result<ConfiguredImage, String> {
    let host = match std::env::consts::ARCH {
        "aarch64" => Architecture::Aarch64,
        "riscv64" => Architecture::Riscv64,
        "x86_64" => Architecture::X86_64,
        _ => return Err("unsupported host architecture".into()),
    };
    image
        .configure_for_host(config.image_configuration()?, host)
        .map_err(|error| match error {
            hyper_vm_image::ConfigurationError::ArchitectureMismatch { image, host } => {
                format!("VM architecture mismatch: ITB {image:?}, host {host:?}")
            }
            other => format!("invalid VM configuration for ITB architecture: {other:?}"),
        })
}

/// Manager admission uses its existing READ authority, without transferring it.
pub fn validate_file(file: &hyper_os::fs::File, config: &Configuration) -> Result<(), String> {
    struct Source<'a>(&'a hyper_os::fs::File);
    impl ReadAt for Source<'_> {
        type Error = hyper_os::Error;
        fn length(&self) -> Result<u64, Self::Error> {
            self.0.size()
        }
        fn read_exact_at(&self, offset: u64, output: &mut [u8]) -> Result<(), Self::Error> {
            self.0.read_exact_at(offset, output)
        }
    }
    // FIT metadata alternates between structure tokens and the string table,
    // often on distant pages separated by payloads. Keep this cache local to
    // admission; it is not a snapshot, and the runtime still revalidates on start.
    let source = read_cache::CachedSource::new(Source(file))
        .map_err(|error| format!("cannot read ITB: {error:?}"))?;
    let image =
        hyper_vm_image::parse(&source).map_err(|error| format!("invalid ITB: {error:?}"))?;
    configure(image, config)?;
    Ok(())
}

#[cfg(test)]
#[path = "../tests/image.rs"]
mod tests;
