// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Immutable mount-table snapshots; publication never holds a lock across I/O.

use hyper::mm::FallibleArc;
use hyper::sync::SpinLock;

use super::instance::{Error, FilesystemInstance, Location, Mount};
use super::scratch::{ScratchBudget, ScratchString, ScratchVec};
use crate::kernel::accounting::{CommittedCharge, ResourceDomain};

const MAX_MOUNTS: usize = 128;

#[derive(Clone)]
struct Attachment {
    covered: Location,
    mounted: FallibleArc<Mount>,
}

struct Topology {
    _charge: CommittedCharge,
    entries: ScratchVec<Attachment>,
}

#[derive(Clone)]
pub(super) struct View(Option<FallibleArc<Topology>>);

impl View {
    fn entries(&self) -> &[Attachment] {
        match &self.0 {
            Some(topology) => &topology.entries,
            None => &[],
        }
    }

    fn same(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (None, None) => true,
            (Some(left), Some(right)) => core::ptr::eq(&**left, &**right),
            _ => false,
        }
    }

    pub(super) fn enter(&self, location: Location) -> Location {
        match self
            .entries()
            .iter()
            .find(|entry| entry.covered == location)
        {
            Some(entry) => Location::new(entry.mounted.clone(), entry.mounted.root()),
            None => location,
        }
    }

    pub(super) fn covers(&self, location: &Location) -> bool {
        self.entries()
            .iter()
            .any(|entry| entry.covered == *location)
    }

    /// Reconstruct each filesystem-local ancestry and the mount edges between
    /// them. The caller validates the epochs of every visited filesystem.
    pub(super) fn ancestry(
        &self,
        root: &Location,
        start: &Location,
        budget: &ScratchBudget,
        mut observe: impl FnMut(&FallibleArc<Mount>) -> Result<(), Error>,
    ) -> Result<ScratchVec<(Location, ScratchString)>, Error> {
        let mut segments = ScratchVec::new(budget.clone());
        let mut cursor = start.clone();
        for _ in 0..=MAX_MOUNTS {
            observe(cursor.mount())?;
            if cursor.mount().id() == root.mount().id() {
                let local =
                    cursor
                        .mount()
                        .filesystem()
                        .ancestry(root.node(), cursor.node(), budget)?;
                segments.push((cursor.mount().clone(), local))?;
                let mut result: ScratchVec<(Location, ScratchString)> =
                    ScratchVec::new(budget.clone());
                for (index, (mount, local)) in segments.into_iter().rev().enumerate() {
                    if index != 0 {
                        // Cross only the mount edge actually present in the
                        // retained start location's ancestry. Re-entering every
                        // covered node would redirect pre-mount cwd capabilities.
                        let attachment = self
                            .entries()
                            .iter()
                            .find(|entry| entry.mounted.id() == mount.id())
                            .ok_or(Error::InvalidInput)?;
                        let (location, _) = result.last_mut().ok_or(Error::InvalidInput)?;
                        if *location != attachment.covered {
                            return Err(Error::InvalidInput);
                        }
                        *location = Location::new(mount.clone(), mount.root());
                    }
                    for (node, name) in local {
                        result.push((Location::new(mount.clone(), node), name))?;
                    }
                }
                return Ok(result);
            }
            let attachment = self
                .entries()
                .iter()
                .find(|entry| entry.mounted.id() == cursor.mount().id())
                .ok_or(Error::InvalidInput)?;
            let local = cursor.mount().filesystem().ancestry(
                &cursor.mount().root(),
                cursor.node(),
                budget,
            )?;
            segments.push((cursor.mount().clone(), local))?;
            cursor = attachment.covered.clone();
        }
        Err(Error::InvalidInput)
    }
}

pub(super) struct MountTable {
    current: SpinLock<View>,
}

impl MountTable {
    pub(super) const fn new() -> Self {
        Self {
            current: SpinLock::new(View(None)),
        }
    }

    pub(super) fn snapshot(&self) -> View {
        self.current.with(|current| current.clone())
    }

    pub(super) fn is_current(&self, view: &View) -> bool {
        self.current.with(|current| current.same(view))
    }

    pub(super) fn attach(
        &self,
        covered: Location,
        filesystem: FallibleArc<FilesystemInstance>,
        sponsor: &ResourceDomain,
    ) -> Result<(), Error> {
        if covered
            .mount()
            .filesystem()
            .attributes(covered.node())?
            .kind()
            != hyper::fs::NodeKind::Directory
        {
            return Err(Error::NotDirectory);
        }
        let old = self.snapshot();
        if old.entries().len() >= MAX_MOUNTS
            || old.covers(&covered)
            || old
                .entries()
                .iter()
                .any(|entry| covered == Location::new(entry.mounted.clone(), entry.mounted.root()))
        {
            return Err(Error::Busy);
        }
        let pin = covered.mount().filesystem().pin_mount(covered.node())?;
        let mounted = Mount::try_new(filesystem, Some(pin), Some(sponsor))?;
        let mut entries = ScratchVec::new(ScratchBudget::new(sponsor));
        entries.try_reserve_exact(old.entries().len() + 1)?;
        for entry in old.entries() {
            entries.push(entry.clone())?;
        }
        entries.push(Attachment { covered, mounted })?;
        let charge = super::instance::allocation_charge::<Topology>(sponsor)?;
        let mut replacement = Some(View(Some(FallibleArc::try_new(Topology {
            _charge: charge,
            entries,
        })?)));
        let retired = self.current.with(|current| {
            if !current.same(&old) {
                return Err(Error::Busy);
            }
            let prepared = replacement.take().ok_or(Error::InvalidBackendResult)?;
            Ok(core::mem::replace(current, prepared))
        })?;
        drop(retired);
        Ok(())
    }
}
