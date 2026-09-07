// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Path policy for commands confined to a delegated working Directory.

pub(crate) fn is_descendant(path: &str) -> bool {
    !path.is_empty() && !path.starts_with('/') && !path.split('/').any(|part| part == "..")
}

#[cfg(test)]
mod tests {
    use super::is_descendant;

    #[test]
    fn accepts_only_relative_paths_without_parent_components() {
        assert!(is_descendant("."));
        assert!(is_descendant("child"));
        assert!(is_descendant("child/nested"));
        assert!(!is_descendant(""));
        assert!(!is_descendant("/"));
        assert!(!is_descendant("/child"));
        assert!(!is_descendant(".."));
        assert!(!is_descendant("child/../sibling"));
    }
}
