// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Weak discovery index for published native Processes.

use alloc::boxed::Box;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use hyper::mm::{WeakFallibleArc, try_box};
use hyper::sync::InterruptSpinLock;

use super::ProcessId;
use super::owner::{Process, ProcessError, ProcessInner, ProcessSnapshot};
use crate::kernel::capability::{HandleScanCursor, HandleSnapshotPage};

const PAGE_CAPACITY: usize = 16;
static NEXT_SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct Entry {
    sequence: u64,
    id: ProcessId,
    process: WeakFallibleArc<ProcessInner>,
    next: Option<Box<Entry>>,
}

struct Directory {
    head: Option<Box<Entry>>,
}

type DirectoryLock<T> = InterruptSpinLock<T, crate::hal::irq::LocalMask>;

static DIRECTORY: DirectoryLock<Directory> = DirectoryLock::new(Directory { head: None });

pub(super) const fn registration_size() -> usize {
    core::mem::size_of::<Entry>()
}

/// Process-lifetime hook which removes diagnostic metadata before charges drop.
pub(super) struct Membership {
    id: ProcessId,
    published: AtomicBool,
}

impl Membership {
    pub(super) const fn new(id: ProcessId) -> Self {
        Self {
            id,
            published: AtomicBool::new(false),
        }
    }

    fn publish(&self) {
        if self.published.swap(true, Ordering::Release) {
            directory_invariant_violation();
        }
    }
}

impl Drop for Membership {
    fn drop(&mut self) {
        if self.published.load(Ordering::Acquire) {
            unregister(self.id);
        }
    }
}

/// Fully allocated directory node which remains invisible until publication.
#[must_use = "publish or discard the prepared Process registration"]
pub(super) struct PreparedRegistration {
    entry: Option<Box<Entry>>,
}

impl PreparedRegistration {
    pub(super) fn try_new(process: &Process) -> Result<Self, ProcessError> {
        let sequence = allocate_sequence()?;
        let entry = try_box(Entry {
            sequence,
            id: process.id(),
            process: process.inner.downgrade(),
            next: None,
        })
        .map_err(|_| ProcessError::Allocation)?;
        Ok(Self { entry: Some(entry) })
    }

    pub(super) fn publish(mut self, process: &Process) {
        let entry = match self.entry.take() {
            Some(entry) => entry,
            None => directory_invariant_violation(),
        };
        DIRECTORY.with(|directory| directory.insert(entry));
        process.inner.directory.publish();
    }
}

impl Drop for PreparedRegistration {
    fn drop(&mut self) {
        // Dropping an unpublished node is the ordinary abort path.
        drop(self.entry.take());
    }
}

fn allocate_sequence() -> Result<u64, ProcessError> {
    let mut current = NEXT_SEQUENCE.load(Ordering::Relaxed);
    loop {
        let next = current.checked_add(1).ok_or(ProcessError::Allocation)?;
        match NEXT_SEQUENCE.compare_exchange_weak(
            current,
            next,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => return Ok(current),
            Err(observed) => current = observed,
        }
    }
}

fn unregister(id: ProcessId) {
    let removed = DIRECTORY.with(|directory| directory.remove(id));
    match removed {
        Some(entry) => drop(entry),
        None => directory_invariant_violation(),
    }
}

impl Directory {
    fn insert(&mut self, mut entry: Box<Entry>) {
        let sequence = entry.sequence;
        let mut link = &mut self.head;
        loop {
            let insert_here = match link.as_ref() {
                Some(current) => current.sequence < sequence,
                None => true,
            };
            if insert_here {
                entry.next = link.take();
                *link = Some(entry);
                return;
            }
            link = match link.as_mut() {
                Some(current) => &mut current.next,
                None => directory_invariant_violation(),
            };
        }
    }

    fn remove(&mut self, id: ProcessId) -> Option<Box<Entry>> {
        let mut link = &mut self.head;
        loop {
            let matches = link.as_ref()?.id == id;
            if matches {
                let mut removed = link.take()?;
                *link = removed.next.take();
                return Some(removed);
            }
            link = &mut link.as_mut()?.next;
        }
    }
}

/// Position in a newest-first scan of published Processes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProcessScanCursor {
    before_sequence: u64,
}

impl ProcessScanCursor {
    pub(crate) const fn start() -> Self {
        Self {
            before_sequence: u64::MAX,
        }
    }
}

/// One pinned Process entry used only while producing diagnostic snapshots.
pub(crate) struct ProcessDiagnosticRef {
    process: Process,
}

impl ProcessDiagnosticRef {
    /// Captures lifecycle metadata without exposing the operation-capable
    /// Process owner retained by this diagnostic pin.
    pub(crate) fn snapshot(&self) -> ProcessSnapshot {
        self.process.snapshot()
    }

    /// Captures one pointer-free page of this Process's handle edges.
    pub(crate) fn scan_handles(
        &self,
        cursor: HandleScanCursor,
    ) -> Result<HandleSnapshotPage, ProcessError> {
        self.process.scan_handles(cursor)
    }
}

pub(crate) struct ProcessSnapshotPage {
    entries: [Option<ProcessDiagnosticRef>; PAGE_CAPACITY],
    len: usize,
    next: Option<ProcessScanCursor>,
}

impl ProcessSnapshotPage {
    pub(crate) fn entries(&self) -> impl Iterator<Item = &ProcessDiagnosticRef> {
        self.entries[..self.len].iter().filter_map(Option::as_ref)
    }

    pub(crate) const fn next(&self) -> Option<ProcessScanCursor> {
        self.next
    }
}

pub(crate) fn scan(cursor: ProcessScanCursor) -> ProcessSnapshotPage {
    let mut entries: [Option<ProcessDiagnosticRef>; PAGE_CAPACITY] =
        [const { None }; PAGE_CAPACITY];
    let (len, next) = DIRECTORY.with(|directory| {
        let mut len = 0;
        let mut last_sequence = 0;
        let mut entry = directory.head.as_deref();
        while let Some(current) = entry {
            if current.sequence < cursor.before_sequence
                && let Some(inner) = current.process.upgrade()
            {
                entries[len] = Some(ProcessDiagnosticRef {
                    process: Process { inner },
                });
                len += 1;
                last_sequence = current.sequence;
                if len == PAGE_CAPACITY {
                    break;
                }
            }
            entry = current.next.as_deref();
        }
        let more = len == PAGE_CAPACITY && has_older_live(directory.head.as_deref(), last_sequence);
        let next = more.then_some(ProcessScanCursor {
            before_sequence: last_sequence,
        });
        (len, next)
    });
    ProcessSnapshotPage { entries, len, next }
}

fn has_older_live(mut entry: Option<&Entry>, sequence: u64) -> bool {
    while let Some(current) = entry {
        if current.sequence < sequence && current.process.is_alive() {
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
