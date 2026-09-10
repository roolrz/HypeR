// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Iterative capability-relative path resolution.

use alloc::vec::Vec;

use hyper::fs::{MAX_PATH_BYTES, NodeKind, Path};
use hyper::mm::FallibleArc;

use super::instance::{Location, MountNamespace};
use super::objects::Error;
use super::resolve_state::{PendingComponent, PendingPath, StateError, Traversal};

pub(super) fn file(
    namespace: &FallibleArc<MountNamespace>,
    traversal_root: &Location,
    value: &str,
) -> Result<Location, Error> {
    resolve(namespace, traversal_root, value, NodeKind::File)
}

pub(super) fn directory(
    namespace: &FallibleArc<MountNamespace>,
    traversal_root: &Location,
    value: &str,
) -> Result<Location, Error> {
    resolve(namespace, traversal_root, value, NodeKind::Directory)
}

fn resolve(
    _namespace: &FallibleArc<MountNamespace>,
    traversal_root: &Location,
    value: &str,
    expected_kind: NodeKind,
) -> Result<Location, Error> {
    let path = Path::new(value).map_err(|_| Error::InvalidPath)?;
    let mut pending = PendingPath::new(path).map_err(map_state_error)?;
    let mut traversal = Traversal::try_new(traversal_root.clone()).map_err(map_state_error)?;

    while let Some(component) = pending.pop() {
        match component {
            PendingComponent::Current => {}
            PendingComponent::Parent => traversal.parent(),
            PendingComponent::Name(descriptor) => {
                let name = pending.name(descriptor).ok_or(Error::InvalidPath)?;
                let lookup = traversal
                    .current()
                    .mount()
                    .filesystem()
                    .lookup_child(traversal.current().node(), name);
                let node = match lookup {
                    Ok(Some(node)) => node,
                    Ok(None) => return Err(Error::Missing),
                    Err(super::instance::Error::NotDirectory) => {
                        return Err(Error::NotDirectory);
                    }
                    Err(error) => return Err(Error::Backend(error)),
                };
                let candidate = Location::new(traversal.current().mount().clone(), node.clone());
                let attributes = traversal
                    .current()
                    .mount()
                    .filesystem()
                    .attributes(&node)
                    .map_err(Error::from)?;
                if attributes.kind() != NodeKind::Symlink {
                    traversal.descend(candidate);
                    continue;
                }

                let target = read_link(&candidate, attributes.size())?;
                let target = core::str::from_utf8(&target).map_err(|_| Error::InvalidPath)?;
                if pending.expand_symlink(target).map_err(map_state_error)? {
                    traversal.restart();
                }
            }
        }
    }

    let attributes = traversal
        .current()
        .mount()
        .filesystem()
        .attributes(traversal.current().node())
        .map_err(Error::from)?;
    if attributes.kind() != expected_kind {
        return Err(match expected_kind {
            NodeKind::Directory => Error::NotDirectory,
            _ => Error::NotRegularFile,
        });
    }
    Ok(traversal.current().clone())
}

fn read_link(location: &Location, size: u64) -> Result<Vec<u8>, Error> {
    let size = usize::try_from(size).map_err(|_| Error::InvalidPath)?;
    if size > MAX_PATH_BYTES {
        return Err(Error::InvalidPath);
    }
    let mut target = Vec::new();
    target
        .try_reserve_exact(size)
        .map_err(|_| Error::Allocation)?;
    target.resize(size, 0);
    let actual = location
        .mount()
        .filesystem()
        .read_link(location.node(), &mut target)
        .map_err(Error::from)?;
    if actual != size {
        return Err(Error::Backend(super::instance::Error::InvalidBackendResult));
    }
    Ok(target)
}

const fn map_state_error(error: StateError) -> Error {
    match error {
        StateError::Allocation => Error::Allocation,
        StateError::InvalidPath => Error::InvalidPath,
    }
}
