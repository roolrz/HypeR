// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Allocation-free canonical paths owned by the shell process.

use hyper_os::fs::{MAX_NAME_BYTES, MAX_PATH_BYTES};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    Empty,
    InvalidComponent,
    TooLong,
}

/// Canonical absolute path within the shell's private root capability.
#[derive(Clone)]
pub(crate) struct CanonicalPath {
    bytes: [u8; MAX_PATH_BYTES],
    len: usize,
}

impl CanonicalPath {
    pub(crate) const fn root() -> Self {
        let mut bytes = [0; MAX_PATH_BYTES];
        bytes[0] = b'/';
        Self { bytes, len: 1 }
    }

    pub(crate) fn resolve(&self, input: &str) -> Result<Self, Error> {
        if input.is_empty() {
            return Err(Error::Empty);
        }
        let mut resolved = if input.starts_with('/') {
            Self::root()
        } else {
            self.clone()
        };
        for component in input.split('/') {
            match component {
                "" | "." => {}
                ".." => resolved.pop(),
                value => resolved.push(value)?,
            }
        }
        Ok(resolved)
    }

    pub(crate) fn as_str(&self) -> Result<&str, Error> {
        core::str::from_utf8(&self.bytes[..self.len]).map_err(|_| Error::InvalidComponent)
    }

    fn pop(&mut self) {
        while self.len > 1 && self.bytes[self.len - 1] != b'/' {
            self.len -= 1;
        }
        if self.len > 1 {
            self.len -= 1;
        }
    }

    fn push(&mut self, component: &str) -> Result<(), Error> {
        if component.len() > MAX_NAME_BYTES || component.as_bytes().contains(&0) {
            return Err(Error::InvalidComponent);
        }
        let separator = usize::from(self.len != 1);
        let end = self
            .len
            .checked_add(separator)
            .and_then(|value| value.checked_add(component.len()))
            .ok_or(Error::TooLong)?;
        if end > MAX_PATH_BYTES {
            return Err(Error::TooLong);
        }
        if separator != 0 {
            self.bytes[self.len] = b'/';
            self.len += 1;
        }
        let destination = self.bytes.get_mut(self.len..end).ok_or(Error::TooLong)?;
        destination.copy_from_slice(component.as_bytes());
        self.len = end;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::CanonicalPath;

    #[test]
    fn resolves_parent_components_without_escaping_shell_root() {
        let root = CanonicalPath::root();
        let path = root
            .resolve("/usr/bin")
            .and_then(|path| path.resolve("../lib"));
        assert!(path.is_ok());
        let Ok(path) = path else {
            return;
        };
        assert_eq!(path.as_str(), Ok("/usr/lib"));

        let root_again = root.resolve("../../..");
        assert!(root_again.is_ok());
        let Ok(root_again) = root_again else {
            return;
        };
        assert_eq!(root_again.as_str(), Ok("/"));
    }

    #[test]
    fn normalizes_repeated_separators_and_dot_components() {
        let path = CanonicalPath::root().resolve("//bin/./tools/");
        assert!(path.is_ok());
        let Ok(path) = path else {
            return;
        };
        assert_eq!(path.as_str(), Ok("/bin/tools"));
    }
}
