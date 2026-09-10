// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Bounded, capability-confined traversal validated against namespace epochs.

use super::instance::{Location, MountNamespace};
use super::objects::Error;
use super::resolve_state::{NameDescriptor, PendingComponent, PendingPath, StateError, Traversal};
use super::scratch::{ScratchBudget, ScratchString, ScratchVec};
use hyper::fs::{MAX_PATH_BYTES, Name, NodeKind, Path};
use hyper::mm::FallibleArc;

#[derive(Clone, Eq, PartialEq)]
struct Step {
    location: Location,
    name: Option<NameDescriptor>,
}
struct NormalizedPath {
    text: ScratchString,
    directory_required: bool,
}

pub(super) struct Resolved {
    pub(super) location: Option<Location>,
    pub(super) parent: Location,
    pub(super) name: ScratchString,
    pub(super) canonical: ScratchString,
    pub(super) epoch: u64,
}
impl Resolved {
    pub(super) fn existing(&self) -> Result<&Location, Error> {
        self.location.as_ref().ok_or(Error::Missing)
    }
}

pub(super) fn file(
    namespace: &FallibleArc<MountNamespace>,
    root: &Location,
    value: &str,
    budget: &ScratchBudget,
) -> Result<Location, Error> {
    typed(namespace, root, root, value, NodeKind::File, true, budget)
}
pub(super) fn typed(
    namespace: &FallibleArc<MountNamespace>,
    root: &Location,
    start: &Location,
    value: &str,
    kind: NodeKind,
    follow: bool,
    budget: &ScratchBudget,
) -> Result<Location, Error> {
    let resolved = resolve(namespace, root, start, value, follow, false, budget)?;
    let location = resolved.existing()?;
    if location
        .mount()
        .filesystem()
        .attributes(location.node())?
        .kind()
        != kind
    {
        return Err(if kind == NodeKind::Directory {
            Error::NotDirectory
        } else if location
            .mount()
            .filesystem()
            .attributes(location.node())?
            .kind()
            == NodeKind::Directory
        {
            Error::IsDirectory
        } else {
            Error::NotRegularFile
        });
    }
    Ok(location.clone())
}

pub(super) fn resolve(
    _namespace: &FallibleArc<MountNamespace>,
    root: &Location,
    start: &Location,
    value: &str,
    follow: bool,
    missing: bool,
    budget: &ScratchBudget,
) -> Result<Resolved, Error> {
    if root.mount().id() != start.mount().id() {
        return Err(Error::CrossDevice);
    }
    let normalized = normalize(value, budget)?;
    let filesystem = root.mount().filesystem();
    for _ in 0..8 {
        let epoch = filesystem.epoch();
        if epoch & 1 != 0 {
            filesystem.wait_for_namespace()?;
            continue;
        }
        let result = walk(root, start, &normalized, follow, missing, epoch, budget);
        // No writer may have committed any ancestry change between the first
        // edge and the last edge. Leases keep all inspected nodes alive.
        // Every topology read acquires the same children/topology lock used
        // by its writer. Observing an edited edge therefore also observes the
        // odd epoch published before that edit, even on weakly ordered CPUs.
        if filesystem.epoch() == epoch {
            return result;
        }
        filesystem.wait_for_namespace()?;
    }
    Err(Error::Busy)
}

fn walk(
    root: &Location,
    start: &Location,
    value: &NormalizedPath,
    follow: bool,
    missing: bool,
    epoch: u64,
    budget: &ScratchBudget,
) -> Result<Resolved, Error> {
    let path = Path::new(&value.text).map_err(|_| Error::InvalidPath)?;
    let mut pending = PendingPath::new(path, budget.clone()).map_err(map_state_error)?;
    if value.directory_required {
        pending.require_final_directory().map_err(map_state_error)?;
    }
    let mut traversal = Traversal::try_new(
        Step {
            location: root.clone(),
            name: None,
        },
        budget.clone(),
    )
    .map_err(map_state_error)?;
    if !path.is_absolute() {
        for (node, name) in root
            .mount()
            .filesystem()
            .ancestry(root.node(), start.node(), budget)?
        {
            traversal
                .descend(Step {
                    location: Location::new(root.mount().clone(), node),
                    name: Some(
                        pending
                            .retain_name(Name::new(&name).map_err(|_| Error::InvalidPath)?)
                            .map_err(map_state_error)?,
                    ),
                })
                .map_err(map_state_error)?;
        }
    }
    let mut parent = traversal.current().location.clone();
    let mut final_name = ScratchString::new(budget.clone());
    while let Some(component) = pending.pop() {
        let current = &traversal.current().location;
        if current
            .mount()
            .filesystem()
            .attributes(current.node())?
            .kind()
            != NodeKind::Directory
        {
            return Err(Error::NotDirectory);
        }
        match component {
            PendingComponent::Current => {}
            PendingComponent::Parent => traversal.parent(),
            PendingComponent::Name(descriptor) => {
                let name = pending.name(descriptor).ok_or(Error::InvalidPath)?;
                let owned_name = copy_string(name.as_str(), budget)?;
                parent = current.clone();
                final_name = copy_string(name.as_str(), budget)?;
                let node = current
                    .mount()
                    .filesystem()
                    .lookup_child(current.node(), name)?;
                let Some(node) = node else {
                    if missing && pending.is_empty() {
                        return Ok(Resolved {
                            location: None,
                            parent,
                            name: owned_name,
                            canonical: ScratchString::new(budget.clone()),
                            epoch,
                        });
                    }
                    return Err(Error::Missing);
                };
                let candidate = Location::new(current.mount().clone(), node);
                let attributes = candidate
                    .mount()
                    .filesystem()
                    .attributes(candidate.node())?;
                if attributes.kind() == NodeKind::Symlink && (follow || !pending.is_empty()) {
                    let target = read_link(&candidate, attributes.size(), budget)?;
                    let target = core::str::from_utf8(&target).map_err(|_| Error::InvalidPath)?;
                    let target = normalize(target, budget)?;
                    if pending
                        .expand_symlink(&target.text, target.directory_required)
                        .map_err(map_state_error)?
                    {
                        traversal.restart();
                    }
                } else {
                    traversal
                        .descend(Step {
                            location: candidate,
                            name: Some(descriptor),
                        })
                        .map_err(map_state_error)?;
                }
            }
        }
    }
    let mut length = 0usize;
    for step in traversal.visited() {
        let Some(descriptor) = step.name else {
            continue;
        };
        let name = pending.name(descriptor).ok_or(Error::InvalidPath)?;
        length = length
            .checked_add(name.as_bytes().len() + 1)
            .filter(|length| *length <= MAX_PATH_BYTES)
            .ok_or(Error::InvalidPath)?;
    }
    let mut canonical = ScratchString::new(budget.clone());
    canonical.try_reserve_exact(length.max(1))?;
    for step in traversal.visited() {
        let Some(descriptor) = step.name else {
            continue;
        };
        canonical.push('/')?;
        canonical.push_str(pending.name(descriptor).ok_or(Error::InvalidPath)?.as_str())?;
    }
    if canonical.is_empty() {
        canonical.push('/')?;
    }
    Ok(Resolved {
        location: Some(traversal.current().location.clone()),
        parent,
        name: final_name,
        canonical,
        epoch,
    })
}

/// Repeated separators carry no name. A terminal separator is retained as a
/// typed directory requirement, without extending the pathname byte budget.
fn normalize(value: &str, budget: &ScratchBudget) -> Result<NormalizedPath, Error> {
    if value.is_empty() || value.len() > MAX_PATH_BYTES || value.as_bytes().contains(&0) {
        return Err(Error::InvalidPath);
    }
    let mut output = ScratchString::new(budget.clone());
    output.try_reserve_exact(value.len())?;
    if value.starts_with('/') {
        output.push('/')?;
    }
    for component in value.split('/').filter(|component| !component.is_empty()) {
        if !output.is_empty() && !output.ends_with('/') {
            output.push('/')?;
        }
        output.push_str(component)?;
    }
    Path::new(&output).map_err(|_| Error::InvalidPath)?;
    Ok(NormalizedPath {
        text: output,
        directory_required: value.ends_with('/'),
    })
}

pub(super) fn read_link(
    location: &Location,
    size: u64,
    budget: &ScratchBudget,
) -> Result<ScratchVec<u8>, Error> {
    let size = usize::try_from(size).map_err(|_| Error::InvalidPath)?;
    if size > MAX_PATH_BYTES {
        return Err(Error::InvalidPath);
    }
    let mut target = ScratchVec::new(budget.clone());
    target.try_reserve_exact(size)?;
    target.resize(size, 0)?;
    let actual = location
        .mount()
        .filesystem()
        .read_link(location.node(), &mut target)?;
    if actual != size {
        return Err(Error::Backend(super::instance::Error::InvalidBackendResult));
    }
    Ok(target)
}
fn copy_string(value: &str, budget: &ScratchBudget) -> Result<ScratchString, Error> {
    ScratchString::from_str(value, budget.clone()).map_err(Into::into)
}
const fn map_state_error(error: StateError<crate::kernel::accounting::ResourceError>) -> Error {
    match error {
        StateError::Allocation => Error::Allocation,
        StateError::InvalidPath => Error::InvalidPath,
        StateError::SymlinkLoop => Error::SymlinkLoop,
        StateError::Budget(error) => Error::Resource(error),
    }
}
