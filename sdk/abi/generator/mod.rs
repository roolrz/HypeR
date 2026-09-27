// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Validation and deterministic rendering for the `HypeR` Native ABI schema.

mod names;
mod render;
mod validation;

#[cfg(test)]
mod tests;

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[path = "../schema/native.rs"]
pub mod schema;

use render::{render_c, render_reference, render_rust};
use schema::AbiSchema;
pub use validation::validate;

const GENERATED_RUST: &str = "src/generated.rs";
const GENERATED_C: &str = "include/hyper/native.h";
const GENERATED_REFERENCE: &str = "docs/native.md";

#[derive(Debug)]
pub enum Error {
    InvalidSchema(String),
    Io { path: PathBuf, source: io::Error },
    Drift { path: PathBuf },
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSchema(message) => {
                write!(formatter, "invalid Native ABI schema: {message}")
            }
            Self::Io { path, source } => write!(formatter, "{}: {source}", path.display()),
            Self::Drift { path } => write!(
                formatter,
                "{} is stale; run `cargo run --features generator --bin hyper-abi -- write`",
                path.display()
            ),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::InvalidSchema(_) | Self::Drift { .. } => None,
        }
    }
}

#[derive(Debug)]
pub struct GeneratedFiles {
    pub rust: String,
    pub c: String,
    pub reference: String,
}

pub fn generate(schema: &AbiSchema) -> Result<GeneratedFiles, Error> {
    validate(schema)?;
    Ok(GeneratedFiles {
        rust: render_rust(schema),
        c: render_c(schema),
        reference: render_reference(schema),
    })
}

pub fn write_repository_outputs(repository: &Path) -> Result<(), Error> {
    let generated = generate(&schema::NATIVE_ABI)?;
    write_output(repository, GENERATED_RUST, &generated.rust)?;
    write_output(repository, GENERATED_C, &generated.c)?;
    write_output(repository, GENERATED_REFERENCE, &generated.reference)
}

pub fn check_repository_outputs(repository: &Path) -> Result<(), Error> {
    let generated = generate(&schema::NATIVE_ABI)?;
    check_output(repository, GENERATED_RUST, &generated.rust)?;
    check_output(repository, GENERATED_C, &generated.c)?;
    check_output(repository, GENERATED_REFERENCE, &generated.reference)
}

fn write_output(repository: &Path, relative: &str, contents: &str) -> Result<(), Error> {
    let path = repository.join(relative);
    let Some(parent) = path.parent() else {
        return Err(Error::InvalidSchema(format!(
            "generated path {relative} has no parent"
        )));
    };
    fs::create_dir_all(parent).map_err(|source| Error::Io {
        path: parent.to_path_buf(),
        source,
    })?;
    fs::write(&path, contents).map_err(|source| Error::Io { path, source })
}

fn check_output(repository: &Path, relative: &str, expected: &str) -> Result<(), Error> {
    let path = repository.join(relative);
    let actual = fs::read_to_string(&path).map_err(|source| Error::Io {
        path: path.clone(),
        source,
    })?;
    if actual == expected {
        Ok(())
    } else {
        Err(Error::Drift { path })
    }
}
