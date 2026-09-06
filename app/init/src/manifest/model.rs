// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

pub const MAX_MANIFEST_BYTES: usize = 64 * 1024;
pub const MAX_SERVICES: usize = 24;
pub const MAX_DEPENDENCIES_PER_SERVICE: usize = 12;
pub const MAX_DEPENDENCY_EDGES: usize = 128;
pub const MAX_CAPABILITIES_PER_SERVICE: usize = 12;
pub const MAX_RIGHTS_PER_CAPABILITY: usize = 12;

pub(super) const MAX_SERVICE_NAME_BYTES: usize = 63;
pub(super) const MAX_IMAGE_PATH_BYTES: usize = 255;
pub(super) const MAX_BINDING_NAME_BYTES: usize = 95;
pub(super) const MAX_PURPOSE_NAME_BYTES: usize = 95;
pub(super) const MAX_RIGHT_NAME_BYTES: usize = 47;

/// Acquisition operation requested for one startup capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityOperation {
    Move,
    Duplicate,
    Create,
}

/// Supervision policy applied after a service process terminates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RestartPolicy {
    Never,
    OnFailure,
    Always,
}

/// One explicitly attenuated startup-capability binding.
#[derive(Debug, Eq, PartialEq)]
pub struct CapabilityBinding<'manifest> {
    pub(super) source: &'manifest str,
    pub(super) purpose: &'manifest str,
    pub(super) operation: CapabilityOperation,
    pub(super) rights: BoundedList<&'manifest str, MAX_RIGHTS_PER_CAPABILITY>,
}

impl CapabilityBinding<'_> {
    pub const fn source(&self) -> &str {
        self.source
    }

    pub const fn purpose(&self) -> &str {
        self.purpose
    }

    pub const fn operation(&self) -> CapabilityOperation {
        self.operation
    }

    pub fn rights(&self) -> impl Iterator<Item = &str> {
        self.rights.iter().copied()
    }
}

/// One service declaration in manifest order.
#[derive(Debug, Eq, PartialEq)]
pub struct Service<'manifest> {
    pub(super) name: &'manifest str,
    pub(super) image: &'manifest str,
    pub(super) critical: bool,
    pub(super) restart: RestartPolicy,
    pub(super) dependencies: BoundedList<&'manifest str, MAX_DEPENDENCIES_PER_SERVICE>,
    pub(super) capabilities:
        BoundedList<CapabilityBinding<'manifest>, MAX_CAPABILITIES_PER_SERVICE>,
}

impl Service<'_> {
    pub const fn name(&self) -> &str {
        self.name
    }

    pub const fn image(&self) -> &str {
        self.image
    }

    pub const fn critical(&self) -> bool {
        self.critical
    }

    pub const fn restart(&self) -> RestartPolicy {
        self.restart
    }

    pub fn dependencies(&self) -> impl Iterator<Item = &str> {
        self.dependencies.iter().copied()
    }

    pub fn capabilities(&self) -> impl Iterator<Item = &CapabilityBinding<'_>> {
        self.capabilities.iter()
    }
}

/// A parsed manifest which borrows immutable text from its `BootFs` image.
#[derive(Debug, Eq, PartialEq)]
pub struct Manifest<'manifest> {
    pub(super) services: BoundedList<Service<'manifest>, MAX_SERVICES>,
}

impl Manifest<'_> {
    pub(crate) fn empty() -> Self {
        Self {
            services: BoundedList::new(),
        }
    }

    pub fn services(&self) -> impl Iterator<Item = &Service<'_>> {
        self.services.iter()
    }

    pub const fn service_count(&self) -> usize {
        self.services.len()
    }

    pub fn service(&self, index: usize) -> Option<&Service<'_>> {
        self.services.get(index)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct BoundedList<T, const N: usize> {
    entries: [Option<T>; N],
    length: usize,
}

impl<T, const N: usize> BoundedList<T, N> {
    pub(super) fn new() -> Self {
        Self {
            entries: core::array::from_fn(|_| None),
            length: 0,
        }
    }

    pub(super) fn push(&mut self, value: T) -> Result<(), T> {
        let Some(slot) = self.entries.get_mut(self.length) else {
            return Err(value);
        };
        *slot = Some(value);
        self.length += 1;
        Ok(())
    }

    pub(super) const fn len(&self) -> usize {
        self.length
    }

    pub(super) const fn is_empty(&self) -> bool {
        self.length == 0
    }

    pub(super) fn get(&self, index: usize) -> Option<&T> {
        self.entries.get(index)?.as_ref()
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &T> {
        self.entries[..self.length]
            .iter()
            .filter_map(Option::as_ref)
    }
}

impl<T, const N: usize> IntoIterator for BoundedList<T, N> {
    type Item = T;
    type IntoIter = ListIntoIter<T, N>;

    fn into_iter(self) -> Self::IntoIter {
        ListIntoIter {
            entries: self.entries,
            position: 0,
            length: self.length,
        }
    }
}

pub(super) struct ListIntoIter<T, const N: usize> {
    entries: [Option<T>; N],
    position: usize,
    length: usize,
}

impl<T, const N: usize> Iterator for ListIntoIter<T, N> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.position == self.length {
            return None;
        }
        let entry = self.entries.get_mut(self.position)?.take();
        self.position += 1;
        entry
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.length.saturating_sub(self.position);
        (remaining, Some(remaining))
    }
}

impl<T, const N: usize> ExactSizeIterator for ListIntoIter<T, N> {}

impl<T, const N: usize> core::iter::FusedIterator for ListIntoIter<T, N> {}
