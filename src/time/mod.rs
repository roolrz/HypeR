// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Architecture-independent software time mechanisms.

mod owned_queue;
mod queue;

pub use owned_queue::{
    ExpiredTimer, OwnedDeadlineQueue, PendingReservedTimer, PendingTimer, ReservedTimerCallbacks,
    ReservedTimerNode, TimerRecycle,
};
pub use queue::{
    DeadlineQueue, Error as TimerQueueError, QueueStats, TimerCallback, TimerEvent, TimerHandle,
    TimerMode,
};
