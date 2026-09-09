// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Path policy for commands confined to a delegated working Directory.

pub fn is_descendant(path: &str) -> bool {
    !path.is_empty() && !path.starts_with('/') && !path.split('/').any(|part| part == "..")
}

#[cfg(test)]
#[path = "../tests/directory_path.rs"]
mod tests;
