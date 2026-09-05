// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Weak global discovery of live kernel objects.
//!
//! The directory is an observability index, not an ownership graph. Entries
//! retain only allocation headers, and snapshots carry no operation authority.
//! Objects registered after a scan starts are excluded from its later pages.

use alloc::boxed::Box;
use core::sync::atomic::{AtomicBool, Ordering};

use hyper::mm::try_box;
use hyper::sync::InterruptSpinLock;

use super::core::ObjectRef;
use super::{Diagnostic, ErasedKernelRef, Koid, ObjectCreationError, ObjectSnapshot};

const PAGE_CAPACITY: usize = 32;

struct Entry {
    sequence: u64,
    koid: Koid,
    object: super::core::WeakObjectRef,
    next: Option<Box<Entry>>,
}

struct Directory {
    next_sequence: u64,
    head: Option<Box<Entry>>,
}

type DirectoryLock<T> = InterruptSpinLock<T, crate::hal::irq::LocalMask>;

// Final object references are allowed to disappear from interrupt context.
// Masking local interrupts therefore protects a normal-context directory scan
// from recursive same-CPU retirement. Snapshot rendering happens after this
// lock is released.
static DIRECTORY: DirectoryLock<Directory> = DirectoryLock::new(Directory {
    next_sequence: 1,
    head: None,
});

pub(super) const fn registration_size() -> usize {
    core::mem::size_of::<Entry>()
}

/// Intrusive lifetime hook which removes the weak index before payload drop.
pub(super) struct Membership {
    koid: Koid,
    published: AtomicBool,
}

impl Membership {
    pub(super) const fn new(koid: Koid) -> Self {
        Self {
            koid,
            published: AtomicBool::new(false),
        }
    }

    pub(super) fn publish(&self) {
        if self.published.swap(true, Ordering::Release) {
            directory_invariant_violation();
        }
    }
}

impl Drop for Membership {
    fn drop(&mut self) {
        if self.published.load(Ordering::Acquire) {
            unregister(self.koid);
        }
    }
}

pub(super) fn register(object: &ObjectRef) -> Result<(), ObjectCreationError> {
    let mut entry = try_box(Entry {
        sequence: 0,
        koid: object.koid(),
        object: object.downgrade(),
        next: None,
    })?;
    DIRECTORY.with(|directory| {
        let sequence = directory.next_sequence;
        let next_sequence = sequence
            .checked_add(1)
            .ok_or(ObjectCreationError::RegistrationExhausted)?;
        directory.next_sequence = next_sequence;
        entry.sequence = sequence;
        entry.next = directory.head.take();
        directory.head = Some(entry);
        Ok::<_, ObjectCreationError>(())
    })?;
    object.publish_directory_membership();
    Ok(())
}

fn unregister(koid: Koid) {
    let removed = DIRECTORY.with(|directory| directory.remove(koid));
    match removed {
        Some(entry) => drop(entry),
        None => directory_invariant_violation(),
    }
}

impl Directory {
    fn remove(&mut self, koid: Koid) -> Option<Box<Entry>> {
        let mut link = &mut self.head;
        loop {
            let matches = link.as_ref()?.koid == koid;
            if matches {
                let mut removed = link.take()?;
                *link = removed.next.take();
                return Some(removed);
            }
            link = &mut link.as_mut()?.next;
        }
    }
}

/// Position in a weakly consistent, newest-first object-directory scan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ObjectScanCursor {
    before_sequence: u64,
}

impl ObjectScanCursor {
    pub(crate) const fn start() -> Self {
        Self {
            before_sequence: u64::MAX,
        }
    }
}

/// One allocation-free page of authority-free object diagnostics.
pub(crate) struct ObjectSnapshotPage {
    entries: [Option<ObjectSnapshot>; PAGE_CAPACITY],
    len: usize,
    next: Option<ObjectScanCursor>,
}

impl ObjectSnapshotPage {
    pub(crate) fn entries(&self) -> impl Iterator<Item = &ObjectSnapshot> {
        self.entries[..self.len].iter().filter_map(Option::as_ref)
    }

    pub(crate) const fn next(&self) -> Option<ObjectScanCursor> {
        self.next
    }
}

/// Captures one directory page without retaining authority or allocating.
pub(crate) fn scan(cursor: ObjectScanCursor) -> ObjectSnapshotPage {
    let mut objects: [Option<ErasedKernelRef<Diagnostic>>; PAGE_CAPACITY] =
        [const { None }; PAGE_CAPACITY];
    let (len, next) = DIRECTORY.with(|directory| {
        let mut len = 0;
        let mut last_sequence = 0;
        let mut entry = directory.head.as_deref();
        while let Some(current) = entry {
            if current.sequence < cursor.before_sequence
                && let Some(object) = current.object.upgrade()
            {
                objects[len] = Some(object);
                len += 1;
                last_sequence = current.sequence;
                if len == PAGE_CAPACITY {
                    break;
                }
            }
            entry = current.next.as_deref();
        }
        let more = len == PAGE_CAPACITY && has_older_live(directory.head.as_deref(), last_sequence);
        let next = more.then_some(ObjectScanCursor {
            before_sequence: last_sequence,
        });
        (len, next)
    });

    let mut entries = [None; PAGE_CAPACITY];
    for index in 0..len {
        let object = objects[index].take();
        entries[index] = object.as_ref().map(ErasedKernelRef::snapshot);
        drop(object);
    }
    ObjectSnapshotPage { entries, len, next }
}

fn has_older_live(mut entry: Option<&Entry>, sequence: u64) -> bool {
    while let Some(current) = entry {
        if current.sequence < sequence && current.object.is_alive() {
            return true;
        }
        entry = current.next.as_deref();
    }
    false
}

#[cold]
fn directory_invariant_violation() -> ! {
    loop {
        core::hint::spin_loop();
    }
}
