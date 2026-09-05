// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Indexed, eagerly maintained virtual-interrupt ready queue.

use alloc::vec::Vec;
use core::cmp::Ordering;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct EntryIndex(pub(super) u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ReadyRank {
    pub(super) priority: u8,
    pub(super) interrupt: u32,
}

impl Ord for ReadyRank {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.priority, self.interrupt).cmp(&(other.priority, other.interrupt))
    }
}

impl PartialOrd for ReadyRank {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub(super) trait ReadyEntries {
    fn rank(&self, index: EntryIndex) -> ReadyRank;
    fn position(&self, index: EntryIndex) -> Option<usize>;
    fn set_position(&mut self, index: EntryIndex, position: Option<usize>);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ReadyError {
    Allocation,
    Capacity,
    CorruptPosition,
}

/// Heap-backed storage whose allocation is completed before runtime use.
pub(super) struct BoundedVec<T> {
    entries: Vec<T>,
    limit: usize,
}

impl<T> BoundedVec<T> {
    pub(super) fn try_new(limit: usize) -> Result<Self, ReadyError> {
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(limit)
            .map_err(|_| ReadyError::Allocation)?;
        Ok(Self { entries, limit })
    }

    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(super) fn remaining(&self) -> usize {
        self.limit - self.entries.len()
    }

    pub(super) fn get(&self, index: usize) -> Option<&T> {
        self.entries.get(index)
    }

    pub(super) fn first(&self) -> Option<&T> {
        self.entries.first()
    }

    pub(super) fn iter(&self) -> core::slice::Iter<'_, T> {
        self.entries.iter()
    }

    pub(super) fn push(&mut self, value: T) -> Result<(), ReadyError> {
        if self.entries.len() == self.limit {
            return Err(ReadyError::Capacity);
        }
        // `try_new` reserved the immutable limit before publication, so this
        // push never enters Vec's allocation path.
        self.entries.push(value);
        Ok(())
    }

    pub(super) fn pop(&mut self) -> Option<T> {
        self.entries.pop()
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
    }

    pub(super) fn swap(&mut self, left: usize, right: usize) {
        self.entries.swap(left, right);
    }
}

impl<T> core::ops::Index<usize> for BoundedVec<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.entries[index]
    }
}

pub(super) struct ReadyQueue {
    entries: BoundedVec<EntryIndex>,
}

impl ReadyQueue {
    pub(super) fn try_with_capacity(capacity: usize) -> Result<Self, ReadyError> {
        Ok(Self {
            entries: BoundedVec::try_new(capacity)?,
        })
    }

    pub(super) fn contains<S: ReadyEntries>(
        &self,
        index: EntryIndex,
        store: &S,
    ) -> Result<bool, ReadyError> {
        let Some(position) = store.position(index) else {
            return Ok(false);
        };
        if self.entries.get(position).copied() != Some(index) {
            return Err(ReadyError::CorruptPosition);
        }
        Ok(true)
    }

    pub(super) fn can_insert(&self) -> Result<(), ReadyError> {
        if self.entries.remaining() != 0 {
            Ok(())
        } else {
            Err(ReadyError::Capacity)
        }
    }

    pub(super) fn remaining(&self) -> usize {
        self.entries.remaining()
    }

    pub(super) fn is_empty(&self) -> bool {
        self.entries.len() == 0
    }

    pub(super) fn insert<S: ReadyEntries>(
        &mut self,
        index: EntryIndex,
        store: &mut S,
    ) -> Result<(), ReadyError> {
        self.can_insert()?;
        let position = self.entries.len();
        self.entries.push(index)?;
        store.set_position(index, Some(position));
        self.sift_up(position, store);
        Ok(())
    }

    pub(super) fn remove<S: ReadyEntries>(&mut self, index: EntryIndex, store: &mut S) {
        let position = match store.position(index) {
            Some(position) => position,
            None => return,
        };
        let last = self.entries.len() - 1;
        self.swap(position, last, store);
        let _ = self.entries.pop();
        store.set_position(index, None);
        if position < self.entries.len() {
            self.repair(position, store);
        }
    }

    pub(super) fn reprioritize<S: ReadyEntries>(&mut self, index: EntryIndex, store: &mut S) {
        if let Some(position) = store.position(index) {
            self.repair(position, store);
        }
    }

    pub(super) fn pop<S: ReadyEntries>(&mut self, store: &mut S) -> Option<EntryIndex> {
        let index = self.entries.first().copied()?;
        self.remove(index, store);
        Some(index)
    }

    fn repair<S: ReadyEntries>(&mut self, position: usize, store: &mut S) {
        if position > 0 {
            let parent = (position - 1) / 2;
            if store.rank(self.entries[position]) < store.rank(self.entries[parent]) {
                self.sift_up(position, store);
                return;
            }
        }
        self.sift_down(position, store);
    }

    fn sift_up<S: ReadyEntries>(&mut self, mut position: usize, store: &mut S) {
        while position > 0 {
            let parent = (position - 1) / 2;
            if store.rank(self.entries[parent]) <= store.rank(self.entries[position]) {
                break;
            }
            self.swap(parent, position, store);
            position = parent;
        }
    }

    fn sift_down<S: ReadyEntries>(&mut self, mut position: usize, store: &mut S) {
        loop {
            let left = position * 2 + 1;
            if left >= self.entries.len() {
                break;
            }
            let right = left + 1;
            let smallest = if right < self.entries.len()
                && store.rank(self.entries[right]) < store.rank(self.entries[left])
            {
                right
            } else {
                left
            };
            if store.rank(self.entries[position]) <= store.rank(self.entries[smallest]) {
                break;
            }
            self.swap(position, smallest, store);
            position = smallest;
        }
    }

    fn swap<S: ReadyEntries>(&mut self, left: usize, right: usize, store: &mut S) {
        if left == right {
            return;
        }
        self.entries.swap(left, right);
        store.set_position(self.entries[left], Some(left));
        store.set_position(self.entries[right], Some(right));
    }
}
