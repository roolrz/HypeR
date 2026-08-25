// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Counting semaphore backed by a FIFO scheduler wait queue.

use hyper::sync::InterruptSpinLock;

use super::Error;
use crate::kernel::task::WaitQueue;
use crate::kernel::task::scheduler;

type StateLock = InterruptSpinLock<State, crate::hal::irq::LocalMask>;

struct State {
    permits: usize,
    waiters: WaitQueue,
}

pub struct Semaphore {
    state: StateLock,
}

impl Semaphore {
    pub const fn new(permits: usize) -> Self {
        Self {
            state: StateLock::new(State {
                permits,
                waiters: WaitQueue::new(),
            }),
        }
    }

    pub fn acquire(&self) -> Result<(), Error> {
        scheduler::ensure_sleepable()?;
        // SAFETY: The retained mask is transferred immediately into the
        // CPU-pinned park transition or dropped before this function proceeds.
        let (park, interrupt_mask) = unsafe {
            self.state.with_mask_retained(|state| {
                if state.permits != 0 {
                    state.permits -= 1;
                    Ok(None)
                } else {
                    scheduler::prepare_park_locked(&state.waiters)
                        .map(Some)
                        .map_err(Error::from)
                }
            })
        };
        let park = park?;
        if let Some(commit) = park {
            scheduler::complete_park(scheduler::retain_park_mask(commit, interrupt_mask));
        } else {
            drop(interrupt_mask);
        }
        Ok(())
    }

    /// Attempts acquisition without sleeping and is safe in IRQ context.
    pub fn try_acquire(&self) -> bool {
        self.state.with(|state| {
            if state.permits == 0 {
                false
            } else {
                state.permits -= 1;
                true
            }
        })
    }

    /// Releases one permit, handing it directly to the oldest waiter.
    pub fn release(&self) -> Result<(), Error> {
        self.state.with(|state| {
            if scheduler::wake_one(&state.waiters)?.is_none() {
                state.permits = state.permits.checked_add(1).ok_or(Error::PermitOverflow)?;
            }
            Ok(())
        })
    }

    pub fn available_permits(&self) -> usize {
        self.state.with(|state| state.permits)
    }

    pub fn waiter_count(&self) -> Result<usize, Error> {
        self.state
            .with(|state| state.waiters.len().map_err(Error::from))
    }
}

impl Default for Semaphore {
    fn default() -> Self {
        Self::new(0)
    }
}
