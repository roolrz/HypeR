// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

pub mod cli;

/// Save to a new file. Native has no rename operation yet, so existing files
/// are never truncated to emulate replacement. Failed new writes are removed.
pub fn save_config(
    path: &std::path::Path,
    machines: Vec<hyper_vm_policy::fleet::Definition>,
) -> std::io::Result<()> {
    use std::io::Write;
    let config = hyper_vm_policy::fleet::Config {
        format: "hyper.vm-config".into(),
        machines,
        copyright: None,
        license: None,
    };
    let bytes = config.to_bytes().map_err(std::io::Error::other)?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    let result = file
        .write_all(&bytes)
        .and_then(|()| file.write_all(b"\n"))
        .and_then(|()| file.flush());
    drop(file);
    if result.is_err() {
        let _ = std::fs::remove_file(path);
    }
    result
}

#[cfg(test)]
#[path = "../tests/config.rs"]
mod tests;
