// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Individually charged namespace bindings, without a retained array high-water mark.

use alloc::boxed::Box;

use super::{Error, Record};

pub(super) struct PreparedRecord(Box<Link>);

struct Link {
    record: Record,
    next: Option<Box<Link>>,
}

/// Two lists give housekeeping an allocation-free, constant-work cursor.
/// Namespace operations search both lists under the same volume mutex.
pub(super) struct Records {
    unexamined: Chain,
    examined: Chain,
    len: usize,
}

#[derive(Default)]
struct Chain(Option<Box<Link>>);

impl Records {
    pub(super) const fn new() -> Self {
        Self {
            unexamined: Chain(None),
            examined: Chain(None),
            len: 0,
        }
    }

    pub(super) const fn allocation_size() -> usize {
        core::mem::size_of::<Link>()
    }

    pub(super) fn prepare(record: Record) -> Result<PreparedRecord, Error> {
        hyper::mm::try_box(Link { record, next: None })
            .map(PreparedRecord)
            .map_err(Error::from)
    }

    pub(super) fn insert(&mut self, record: PreparedRecord) {
        self.unexamined.push(record.0);
        self.len += 1;
    }

    pub(super) const fn len(&self) -> usize {
        self.len
    }

    /// A partially completed pass can revisit the live unexamined prefix
    /// after swapping lists. Two lengths cover every initial binding without
    /// retaining a pointer or rescanning a prefix to recover the cursor.
    pub(super) fn cleanup_bound(&self) -> usize {
        self.len.saturating_mul(2)
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &Record> {
        self.unexamined.iter().chain(self.examined.iter())
    }

    pub(super) fn iter_mut(&mut self) -> impl Iterator<Item = &mut Record> {
        self.unexamined.iter_mut().chain(self.examined.iter_mut())
    }

    pub(super) fn retain(&mut self, mut keep: impl FnMut(&Record) -> bool) {
        self.len -= self.unexamined.retain(&mut keep);
        self.len -= self.examined.retain(&mut keep);
    }

    /// Inspects exactly one binding. Any removed owner is detached from its
    /// list before returning, so dropping it cannot recursively free a tail.
    pub(super) fn inspect_next(&mut self) -> Option<Option<PreparedRecord>> {
        if self.unexamined.0.is_none() {
            core::mem::swap(&mut self.unexamined, &mut self.examined);
        }
        let entry = self.unexamined.pop()?;
        if entry.record.content.is_alive() {
            self.examined.push(entry);
            Some(None)
        } else {
            self.len -= 1;
            Some(Some(PreparedRecord(entry)))
        }
    }
}

impl PreparedRecord {
    pub(super) fn value(&self) -> &Record {
        &self.0.record
    }

    pub(super) fn set_path(&mut self, path: alloc::string::String) {
        self.0.record.path = path;
    }
}

impl Chain {
    fn pop(&mut self) -> Option<Box<Link>> {
        let mut entry = self.0.take()?;
        self.0 = entry.next.take();
        Some(entry)
    }

    fn push(&mut self, mut entry: Box<Link>) {
        entry.next = self.0.take();
        self.0 = Some(entry);
    }

    fn iter(&self) -> impl Iterator<Item = &Record> {
        core::iter::successors(self.0.as_deref(), |entry| entry.next.as_deref())
            .map(|entry| &entry.record)
    }

    fn iter_mut(&mut self) -> RecordsMut<'_> {
        RecordsMut(self.0.as_deref_mut())
    }

    fn retain(&mut self, keep: &mut impl FnMut(&Record) -> bool) -> usize {
        let mut cursor = &mut self.0;
        let mut removed = 0;
        while let Some(retained) = cursor.as_ref().map(|entry| keep(&entry.record)) {
            if retained {
                let Some(entry) = cursor else { break };
                cursor = &mut entry.next;
            } else {
                let Some(mut entry) = cursor.take() else {
                    break;
                };
                *cursor = entry.next.take();
                removed += 1;
            }
        }
        removed
    }
}

impl Drop for Chain {
    fn drop(&mut self) {
        while self.pop().is_some() {}
    }
}

struct RecordsMut<'a>(Option<&'a mut Link>);

impl<'a> Iterator for RecordsMut<'a> {
    type Item = &'a mut Record;

    fn next(&mut self) -> Option<Self::Item> {
        let entry = self.0.take()?;
        self.0 = entry.next.as_deref_mut();
        Some(&mut entry.record)
    }
}
