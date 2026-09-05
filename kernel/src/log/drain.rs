// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Allocation-free queues and wake ownership for deferred console draining.

/// Observation of one finite console-drain watermark.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DrainBarrierStatus {
    Pending,
    Drained,
    Overrun { missed: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DrainBarrierError {
    NoFreeSlot,
    InvalidToken,
    InvalidLossRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DrainBarrierRegistration {
    Complete,
    Pending(DrainBarrierToken),
}

/// Generation-qualified identity of one registered console watermark.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DrainBarrierToken {
    slot: usize,
    generation: u64,
}

impl DrainBarrierToken {
    pub const fn slot(self) -> usize {
        self.slot
    }
}

#[derive(Clone, Copy)]
struct DrainBarrierSlot {
    generation: u64,
    active: bool,
    start: u64,
    target: u64,
    missed: u64,
    complete: bool,
    notification_pending: bool,
}

impl DrainBarrierSlot {
    const fn new() -> Self {
        Self {
            generation: 0,
            active: false,
            start: 0,
            target: 0,
            missed: 0,
            complete: false,
            notification_pending: false,
        }
    }
}

/// Fixed-capacity set of exact, independently completed drain watermarks.
///
/// A slot accumulates only loss whose sequence interval precedes its target.
/// Later console overruns therefore cannot change an already completed flush.
pub struct DrainBarrierSet<const CAPACITY: usize> {
    slots: [DrainBarrierSlot; CAPACITY],
    active: usize,
}

impl<const CAPACITY: usize> DrainBarrierSet<CAPACITY> {
    pub const fn new() -> Self {
        Self {
            slots: [const { DrainBarrierSlot::new() }; CAPACITY],
            active: 0,
        }
    }

    pub fn register(
        &mut self,
        current_sequence: u64,
        target_sequence: u64,
    ) -> Result<DrainBarrierRegistration, DrainBarrierError> {
        if current_sequence >= target_sequence {
            return Ok(DrainBarrierRegistration::Complete);
        }
        let Some((index, slot)) = self
            .slots
            .iter_mut()
            .enumerate()
            .find(|(_, slot)| !slot.active)
        else {
            return Err(DrainBarrierError::NoFreeSlot);
        };
        slot.generation = slot.generation.wrapping_add(1);
        slot.active = true;
        slot.start = current_sequence;
        slot.target = target_sequence;
        slot.missed = 0;
        slot.complete = false;
        slot.notification_pending = false;
        self.active += 1;
        Ok(DrainBarrierRegistration::Pending(DrainBarrierToken {
            slot: index,
            generation: slot.generation,
        }))
    }

    /// Accounts one exact lost interval and completes crossed watermarks.
    pub fn advance_overrun(
        &mut self,
        previous_sequence: u64,
        next_sequence: u64,
        reported_missed: u64,
    ) -> Result<(), DrainBarrierError> {
        let Some(loss_length) = next_sequence.checked_sub(previous_sequence) else {
            return Err(DrainBarrierError::InvalidLossRange);
        };
        if loss_length != reported_missed {
            return Err(DrainBarrierError::InvalidLossRange);
        }
        if self.active == 0 {
            return Ok(());
        }
        for slot in self
            .slots
            .iter_mut()
            .filter(|slot| slot.active && !slot.complete)
        {
            let intersection_start = previous_sequence.max(slot.start);
            let intersection_end = next_sequence.min(slot.target);
            if intersection_end > intersection_start {
                slot.missed = slot
                    .missed
                    .saturating_add(intersection_end - intersection_start);
            }
        }
        self.advance(next_sequence);
        Ok(())
    }

    /// Completes every active watermark crossed by the console cursor.
    pub fn advance(&mut self, next_sequence: u64) {
        if self.active == 0 {
            return;
        }
        for slot in self
            .slots
            .iter_mut()
            .filter(|slot| slot.active && !slot.complete)
        {
            if next_sequence >= slot.target {
                slot.complete = true;
                slot.notification_pending = true;
            }
        }
    }

    pub fn status(
        &self,
        token: DrainBarrierToken,
    ) -> Result<DrainBarrierStatus, DrainBarrierError> {
        let slot = self.slot(token)?;
        if !slot.complete {
            Ok(DrainBarrierStatus::Pending)
        } else if slot.missed == 0 {
            Ok(DrainBarrierStatus::Drained)
        } else {
            Ok(DrainBarrierStatus::Overrun {
                missed: slot.missed,
            })
        }
    }

    /// Returns whether `slot` newly completed and needs its waiter notified.
    pub fn take_completion_notification(&mut self, slot: usize) -> bool {
        let Some(slot) = self.slots.get_mut(slot) else {
            return false;
        };
        let pending = slot.active && slot.notification_pending;
        slot.notification_pending = false;
        pending
    }

    pub fn release(&mut self, token: DrainBarrierToken) -> Result<(), DrainBarrierError> {
        let slot = self.slot_mut(token)?;
        slot.active = false;
        slot.complete = false;
        slot.notification_pending = false;
        self.active -= 1;
        Ok(())
    }

    pub const fn active_count(&self) -> usize {
        self.active
    }

    fn slot(&self, token: DrainBarrierToken) -> Result<&DrainBarrierSlot, DrainBarrierError> {
        self.slots
            .get(token.slot)
            .filter(|slot| slot.active && slot.generation == token.generation)
            .ok_or(DrainBarrierError::InvalidToken)
    }

    fn slot_mut(
        &mut self,
        token: DrainBarrierToken,
    ) -> Result<&mut DrainBarrierSlot, DrainBarrierError> {
        self.slots
            .get_mut(token.slot)
            .filter(|slot| slot.active && slot.generation == token.generation)
            .ok_or(DrainBarrierError::InvalidToken)
    }
}

impl<const CAPACITY: usize> Default for DrainBarrierSet<CAPACITY> {
    fn default() -> Self {
        Self::new()
    }
}

/// Fixed-capacity byte FIFO used by console producers which cannot block.
pub struct ByteRing<const CAPACITY: usize> {
    bytes: [u8; CAPACITY],
    head: usize,
    tail: usize,
    length: usize,
    dropped: u64,
}

impl<const CAPACITY: usize> ByteRing<CAPACITY> {
    pub const fn new() -> Self {
        Self {
            bytes: [0; CAPACITY],
            head: 0,
            tail: 0,
            length: 0,
            dropped: 0,
        }
    }

    /// Appends one byte, retaining an overflow count when the FIFO is full.
    pub fn push(&mut self, byte: u8) -> bool {
        if CAPACITY == 0 || self.length == CAPACITY {
            self.dropped = self.dropped.saturating_add(1);
            return false;
        }
        self.bytes[self.head] = byte;
        self.head = (self.head + 1) % CAPACITY;
        self.length += 1;
        true
    }

    /// Removes as many oldest bytes as fit in `output`.
    pub fn pop_into(&mut self, output: &mut [u8]) -> usize {
        let count = self.length.min(output.len());
        for slot in &mut output[..count] {
            *slot = self.bytes[self.tail];
            self.tail = (self.tail + 1) % CAPACITY;
        }
        self.length -= count;
        count
    }

    /// Copies as many oldest bytes as fit without consuming them.
    ///
    /// A caller may use this to prepare a fallible external write and discard
    /// the exact prefix only after that write commits.
    pub fn peek_into(&self, output: &mut [u8]) -> usize {
        let count = self.length.min(output.len());
        let mut index = self.tail;
        for slot in &mut output[..count] {
            *slot = self.bytes[index];
            index = (index + 1) % CAPACITY;
        }
        count
    }

    /// Discards an already-observed prefix.
    ///
    /// Returns `false` without mutation when `count` exceeds the retained
    /// length. This lets transaction owners reject stale or duplicated
    /// commits without partially consuming the FIFO.
    pub fn discard_front(&mut self, count: usize) -> bool {
        if count > self.length {
            return false;
        }
        if CAPACITY != 0 {
            self.tail = (self.tail + count) % CAPACITY;
        }
        self.length -= count;
        true
    }

    pub const fn is_empty(&self) -> bool {
        self.length == 0
    }

    pub const fn remaining_capacity(&self) -> usize {
        CAPACITY - self.length
    }

    pub const fn front(&self) -> Option<u8> {
        if self.length == 0 {
            None
        } else {
            Some(self.bytes[self.tail])
        }
    }

    pub fn pop_front(&mut self) -> Option<u8> {
        let byte = self.front()?;
        self.tail = (self.tail + 1) % CAPACITY;
        self.length -= 1;
        Some(byte)
    }

    pub const fn dropped(&self) -> u64 {
        self.dropped
    }
}

impl<const CAPACITY: usize> Default for ByteRing<CAPACITY> {
    fn default() -> Self {
        Self::new()
    }
}
