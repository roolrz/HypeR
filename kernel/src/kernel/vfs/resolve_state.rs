// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Owned, bounded pending state for iterative path resolution.

use alloc::vec::Vec;

use hyper::fs::{MAX_PATH_BYTES, MAX_PATH_COMPONENTS, Name, Path, PathComponent};

const MAX_SYMLINK_HOPS: usize = 40;
const MAX_EXPANDED_PATH_BYTES: usize = MAX_PATH_BYTES * 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StateError {
    Allocation,
    InvalidPath,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PendingComponent {
    Current,
    Parent,
    Name(NameDescriptor),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct NameDescriptor {
    start: u16,
    length: u8,
}

pub(super) struct PendingPath {
    components: Vec<PendingComponent>,
    names: Vec<u8>,
    expanded_components: usize,
    expanded_bytes: usize,
    symlink_hops: usize,
}

/// Kernel-owned ancestry for one capability-confined traversal.
///
/// Backends supply child identities but never supply trusted parent movement.
/// Keeping the visited chain here makes `..` and absolute symlink handling
/// independent of backend integrity.
pub(super) struct Traversal<T> {
    root: T,
    current: T,
    ancestors: Vec<T>,
}

impl<T: Clone + Eq> Traversal<T> {
    pub(super) fn try_new(root: T) -> Result<Self, StateError> {
        let mut ancestors = Vec::new();
        ancestors
            .try_reserve_exact(MAX_PATH_COMPONENTS)
            .map_err(|_| StateError::Allocation)?;
        Ok(Self {
            current: root.clone(),
            root,
            ancestors,
        })
    }

    pub(super) const fn current(&self) -> &T {
        &self.current
    }

    pub(super) fn descend(&mut self, child: T) {
        let parent = core::mem::replace(&mut self.current, child);
        self.ancestors.push(parent);
    }

    pub(super) fn parent(&mut self) {
        if self.current == self.root {
            self.ancestors.clear();
            return;
        }
        self.current = match self.ancestors.pop() {
            Some(parent) => parent,
            None => self.root.clone(),
        };
    }

    pub(super) fn restart(&mut self) {
        self.current = self.root.clone();
        self.ancestors.clear();
    }
}

impl PendingPath {
    pub(super) fn new(path: Path<'_>) -> Result<Self, StateError> {
        let mut state = Self {
            components: Vec::new(),
            names: Vec::new(),
            expanded_components: path.component_count(),
            expanded_bytes: path.as_str().len(),
            symlink_hops: 0,
        };
        state.push(path)?;
        Ok(state)
    }

    pub(super) fn pop(&mut self) -> Option<PendingComponent> {
        self.components.pop()
    }

    pub(super) fn name(&self, descriptor: NameDescriptor) -> Option<Name<'_>> {
        let start = usize::from(descriptor.start);
        let end = start.checked_add(usize::from(descriptor.length))?;
        let bytes = self.names.get(start..end)?;
        let value = core::str::from_utf8(bytes).ok()?;
        Name::new(value).ok()
    }

    /// Inserts a symlink target ahead of the unresolved suffix.
    ///
    /// The returned flag tells namespace policy whether traversal must restart
    /// at its capability root. Components are copied before the caller's
    /// backend buffer is released, so no adapter needs to lend stable storage.
    pub(super) fn expand_symlink(&mut self, target: &str) -> Result<bool, StateError> {
        self.symlink_hops = self
            .symlink_hops
            .checked_add(1)
            .ok_or(StateError::InvalidPath)?;
        if self.symlink_hops > MAX_SYMLINK_HOPS {
            return Err(StateError::InvalidPath);
        }
        let target = Path::new(target).map_err(|_| StateError::InvalidPath)?;
        self.expanded_bytes = self
            .expanded_bytes
            .checked_add(target.as_str().len())
            .ok_or(StateError::InvalidPath)?;
        if self.expanded_bytes > MAX_EXPANDED_PATH_BYTES {
            return Err(StateError::InvalidPath);
        }
        self.expanded_components = self
            .expanded_components
            .checked_add(target.component_count())
            .ok_or(StateError::InvalidPath)?;
        if self.expanded_components > MAX_PATH_COMPONENTS {
            return Err(StateError::InvalidPath);
        }
        let absolute = target.is_absolute();
        self.push(target)?;
        Ok(absolute)
    }

    fn push(&mut self, path: Path<'_>) -> Result<(), StateError> {
        let name_bytes = path.components().try_fold(0usize, |total, component| {
            let length = match component {
                PathComponent::Name(name) => name.as_bytes().len(),
                PathComponent::Current | PathComponent::Parent => 0,
            };
            total.checked_add(length).ok_or(StateError::InvalidPath)
        })?;
        self.components
            .try_reserve(path.component_count())
            .map_err(|_| StateError::Allocation)?;
        self.names
            .try_reserve(name_bytes)
            .map_err(|_| StateError::Allocation)?;
        let start = self.components.len();
        for component in path.components() {
            let owned = match component {
                PathComponent::Current => PendingComponent::Current,
                PathComponent::Parent => PendingComponent::Parent,
                PathComponent::Name(name) => {
                    let name_start =
                        u16::try_from(self.names.len()).map_err(|_| StateError::InvalidPath)?;
                    let name_length =
                        u8::try_from(name.as_bytes().len()).map_err(|_| StateError::InvalidPath)?;
                    self.names.extend_from_slice(name.as_bytes());
                    PendingComponent::Name(NameDescriptor {
                        start: name_start,
                        length: name_length,
                    })
                }
            };
            self.components.push(owned);
        }
        let added = self
            .components
            .get_mut(start..)
            .ok_or(StateError::Allocation)?;
        added.reverse();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use alloc::format;

    use super::{
        MAX_EXPANDED_PATH_BYTES, MAX_SYMLINK_HOPS, PendingComponent, PendingPath, StateError,
        Traversal,
    };
    use hyper::fs::{MAX_PATH_BYTES, Path};

    fn path(value: &str) -> Path<'_> {
        match Path::new(value) {
            Ok(path) => path,
            Err(error) => panic!("test path is invalid: {error:?}"),
        }
    }

    #[test]
    fn symlink_target_precedes_the_existing_suffix() {
        let mut pending = match PendingPath::new(path("tail")) {
            Ok(pending) => pending,
            Err(error) => panic!("pending path failed: {error:?}"),
        };
        assert_eq!(pending.expand_symlink("directory/file"), Ok(false));
        let first = match pending.pop() {
            Some(PendingComponent::Name(name)) => name,
            other => panic!("unexpected first component: {other:?}"),
        };
        assert_eq!(
            pending.name(first).map(|name| name.as_str()),
            Some("directory")
        );
        let second = match pending.pop() {
            Some(PendingComponent::Name(name)) => name,
            other => panic!("unexpected second component: {other:?}"),
        };
        assert_eq!(pending.name(second).map(|name| name.as_str()), Some("file"));
        let third = match pending.pop() {
            Some(PendingComponent::Name(name)) => name,
            other => panic!("unexpected third component: {other:?}"),
        };
        assert_eq!(pending.name(third).map(|name| name.as_str()), Some("tail"));
    }

    #[test]
    fn absolute_targets_are_reported_without_ambient_root_state() {
        let mut pending = match PendingPath::new(path("tail")) {
            Ok(pending) => pending,
            Err(error) => panic!("pending path failed: {error:?}"),
        };
        assert_eq!(pending.expand_symlink("/etc/config"), Ok(true));
    }

    #[test]
    fn expansion_work_and_symlink_hops_are_bounded() {
        let mut pending = match PendingPath::new(path("tail")) {
            Ok(pending) => pending,
            Err(error) => panic!("pending path failed: {error:?}"),
        };
        let maximum_target = (0..256)
            .map(|index| format!("n{index}"))
            .collect::<alloc::vec::Vec<_>>()
            .join("/");
        assert_eq!(
            pending.expand_symlink(&maximum_target),
            Err(StateError::InvalidPath)
        );

        let mut pending = match PendingPath::new(path("tail")) {
            Ok(pending) => pending,
            Err(error) => panic!("pending path failed: {error:?}"),
        };
        for _ in 0..MAX_SYMLINK_HOPS {
            assert_eq!(pending.expand_symlink("."), Ok(false));
        }
        assert_eq!(pending.expand_symlink("."), Err(StateError::InvalidPath));

        let target = alloc::vec!["x".repeat(255); 16].join("/");
        let mut pending = match PendingPath::new(path("tail")) {
            Ok(pending) => pending,
            Err(error) => panic!("pending path failed: {error:?}"),
        };
        for _ in 0..4 {
            assert_eq!(pending.expand_symlink(&target), Ok(false));
        }
        assert_eq!(
            pending.expand_symlink(&target),
            Err(StateError::InvalidPath)
        );
        assert_eq!(MAX_EXPANDED_PATH_BYTES, MAX_PATH_BYTES * 4);
    }

    #[test]
    fn traversal_owns_parent_clamping_and_absolute_restart() {
        let mut traversal = match Traversal::try_new(10_u64) {
            Ok(traversal) => traversal,
            Err(error) => panic!("traversal construction failed: {error:?}"),
        };
        traversal.parent();
        assert_eq!(*traversal.current(), 10);
        traversal.descend(11);
        traversal.descend(12);
        traversal.parent();
        assert_eq!(*traversal.current(), 11);
        traversal.restart();
        assert_eq!(*traversal.current(), 10);
        traversal.parent();
        assert_eq!(*traversal.current(), 10);
    }
}
