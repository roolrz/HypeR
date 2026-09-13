// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Allocation-free admission and interrupt decisions for an assigned transport.

#[derive(Clone, Copy)]
pub(crate) struct Negotiation {
    selector: u32,
    access_platform: bool,
}
impl Negotiation {
    pub(crate) const fn new() -> Self {
        Self {
            selector: 0,
            access_platform: false,
        }
    }
    /// Reject DMA activation without the platform address translation contract.
    pub(crate) fn write(&mut self, offset: usize, value: u32) -> bool {
        match offset {
            0x24 => self.selector = value,
            0x20 if self.selector == 1 => self.access_platform = value & 2 != 0,
            0x70 if value == 0 => *self = Self::new(),
            0x70 if value & 4 != 0 && !self.access_platform => return false,
            _ => {}
        }
        true
    }
}

/// A latched physical IRQ can arrive after the guest already acknowledged it.
/// Masking that empty source would strand the next completion without an ACK.
pub(crate) const fn mask_interrupt(
    active_route: bool,
    status: u32,
    trigger: hyper::hal::interrupt::InterruptTrigger,
) -> bool {
    // Edge sources do not retrigger merely because InterruptStatus stays set.
    // Keep them enabled: each new transition prompts an updated guest level,
    // and concurrent completions remain represented by the durable status.
    !active_route
        || (matches!(trigger, hyper::hal::interrupt::InterruptTrigger::Level) && status != 0)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExtentError {
    Range,
    Nonresident,
    Noncontiguous,
}
pub(crate) fn contiguous_extent(
    offset: u64,
    length: u64,
    size: u64,
    page_size: u64,
    mut physical: impl FnMut(u64) -> Option<u64>,
) -> Result<u64, ExtentError> {
    if page_size == 0
        || length == 0
        || !offset.is_multiple_of(page_size)
        || !length.is_multiple_of(page_size)
    {
        return Err(ExtentError::Range);
    }
    let end = offset
        .checked_add(length)
        .filter(|end| *end <= size)
        .ok_or(ExtentError::Range)?;
    let first = physical(offset).ok_or(ExtentError::Nonresident)?;
    first.checked_add(length).ok_or(ExtentError::Range)?;
    let mut current = offset;
    while current < end {
        if physical(current).ok_or(ExtentError::Nonresident)? != first + (current - offset) {
            return Err(ExtentError::Noncontiguous);
        }
        current += page_size;
    }
    Ok(first)
}
