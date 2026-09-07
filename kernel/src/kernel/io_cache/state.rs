// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Allocation-bounded state model for the immutable file-data cache.

use alloc::vec::Vec;

use super::CacheKey;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Error {
    Allocation,
    InvalidCapacity,
    SequenceExhausted,
    StaleLoad,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct LoadToken {
    slot: usize,
    generation: u64,
    key: CacheKey,
    access_sequence: u64,
}

impl LoadToken {
    pub(super) const fn slot(self) -> usize {
        self.slot
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Access {
    Hit { slot: usize },
    LoadInProgress,
    Reserved { token: LoadToken, evicted: bool },
    CapacityBusy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Snapshot {
    pub(super) capacity: usize,
    pub(super) clean: usize,
    pub(super) loading: usize,
    pub(super) hits: u64,
    pub(super) misses: u64,
    pub(super) evictions: u64,
    pub(super) in_progress: u64,
    pub(super) capacity_busy: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Slot {
    Vacant,
    Loading { key: CacheKey, generation: u64 },
    Clean { key: CacheKey, last_access: u64 },
}

pub(super) struct State {
    slots: Vec<Slot>,
    next_load_generation: u64,
    next_access_sequence: u64,
    hits: u64,
    misses: u64,
    evictions: u64,
    in_progress: u64,
    capacity_busy: u64,
}

impl State {
    pub(super) fn try_new(capacity: usize) -> Result<Self, Error> {
        if capacity == 0 {
            return Err(Error::InvalidCapacity);
        }
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(capacity)
            .map_err(|_| Error::Allocation)?;
        slots.resize(capacity, Slot::Vacant);
        Ok(Self {
            slots,
            next_load_generation: 0,
            next_access_sequence: 0,
            hits: 0,
            misses: 0,
            evictions: 0,
            in_progress: 0,
            capacity_busy: 0,
        })
    }

    pub(super) fn access(&mut self, key: CacheKey) -> Result<Access, Error> {
        for (slot, entry) in self.slots.iter().enumerate() {
            match entry {
                Slot::Clean {
                    key: cached_key, ..
                } if *cached_key == key => {
                    let access_sequence = self.reserve_access_sequence()?;
                    let Some(entry) = self.slots.get_mut(slot) else {
                        return Err(Error::StaleLoad);
                    };
                    *entry = Slot::Clean {
                        key,
                        last_access: access_sequence,
                    };
                    self.hits = self.hits.saturating_add(1);
                    return Ok(Access::Hit { slot });
                }
                Slot::Loading {
                    key: loading_key, ..
                } if *loading_key == key => {
                    self.in_progress = self.in_progress.saturating_add(1);
                    return Ok(Access::LoadInProgress);
                }
                Slot::Vacant | Slot::Loading { .. } | Slot::Clean { .. } => {}
            }
        }

        let target = self
            .slots
            .iter()
            .position(|entry| matches!(entry, Slot::Vacant))
            .or_else(|| self.oldest_clean_slot());
        let Some(slot) = target else {
            self.capacity_busy = self.capacity_busy.saturating_add(1);
            return Ok(Access::CapacityBusy);
        };
        let generation = self.reserve_load_generation()?;
        let access_sequence = self.reserve_access_sequence()?;
        let Some(entry) = self.slots.get_mut(slot) else {
            return Err(Error::StaleLoad);
        };
        let evicted = matches!(entry, Slot::Clean { .. });
        *entry = Slot::Loading { key, generation };
        self.misses = self.misses.saturating_add(1);
        if evicted {
            self.evictions = self.evictions.saturating_add(1);
        }
        Ok(Access::Reserved {
            token: LoadToken {
                slot,
                generation,
                key,
                access_sequence,
            },
            evicted,
        })
    }

    pub(super) fn publish(&mut self, token: LoadToken) -> Result<(), Error> {
        self.validate(token)?;
        let Some(entry) = self.slots.get_mut(token.slot) else {
            return Err(Error::StaleLoad);
        };
        *entry = Slot::Clean {
            key: token.key,
            last_access: token.access_sequence,
        };
        Ok(())
    }

    pub(super) fn abort(&mut self, token: LoadToken) -> Result<(), Error> {
        self.validate(token)?;
        let Some(entry) = self.slots.get_mut(token.slot) else {
            return Err(Error::StaleLoad);
        };
        *entry = Slot::Vacant;
        Ok(())
    }

    pub(super) fn validate(&self, token: LoadToken) -> Result<(), Error> {
        let Some(entry) = self.slots.get(token.slot) else {
            return Err(Error::StaleLoad);
        };
        if *entry
            != (Slot::Loading {
                key: token.key,
                generation: token.generation,
            })
        {
            return Err(Error::StaleLoad);
        }
        Ok(())
    }

    pub(super) fn snapshot(&self) -> Snapshot {
        let mut clean = 0;
        let mut loading = 0;
        for entry in &self.slots {
            match entry {
                Slot::Vacant => {}
                Slot::Loading { .. } => loading += 1,
                Slot::Clean { .. } => clean += 1,
            }
        }
        Snapshot {
            capacity: self.slots.len(),
            clean,
            loading,
            hits: self.hits,
            misses: self.misses,
            evictions: self.evictions,
            in_progress: self.in_progress,
            capacity_busy: self.capacity_busy,
        }
    }

    fn oldest_clean_slot(&self) -> Option<usize> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(slot, entry)| match entry {
                Slot::Clean { last_access, .. } => Some((slot, *last_access)),
                Slot::Vacant | Slot::Loading { .. } => None,
            })
            .min_by_key(|(_, last_access)| *last_access)
            .map(|(slot, _)| slot)
    }

    fn reserve_load_generation(&mut self) -> Result<u64, Error> {
        let generation = self
            .next_load_generation
            .checked_add(1)
            .ok_or(Error::SequenceExhausted)?;
        self.next_load_generation = generation;
        Ok(generation)
    }

    fn reserve_access_sequence(&mut self) -> Result<u64, Error> {
        let sequence = self
            .next_access_sequence
            .checked_add(1)
            .ok_or(Error::SequenceExhausted)?;
        self.next_access_sequence = sequence;
        Ok(sequence)
    }

    #[cfg(test)]
    pub(super) fn invalidate_for_test(&mut self, slot: usize) {
        if let Some(entry) = self.slots.get_mut(slot) {
            *entry = Slot::Vacant;
        }
    }
}
