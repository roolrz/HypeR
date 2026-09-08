// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

#![cfg(feature = "generator")]

use std::error::Error;
use std::process::Command;

#[test]
fn generator_resolves_outputs_from_its_manifest_not_the_callers_directory()
-> Result<(), Box<dyn Error>> {
    let scratch = std::env::temp_dir().join(format!(
        "hyper-abi-cli-working-directory-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir(&scratch)?;

    let output = Command::new(env!("CARGO_BIN_EXE_hyper-abi"))
        .current_dir(&scratch)
        .arg("check")
        .output()?;
    assert!(
        output.status.success(),
        "generator check failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(std::fs::read_dir(&scratch)?.next().is_none());

    std::fs::remove_dir(&scratch)?;
    Ok(())
}
