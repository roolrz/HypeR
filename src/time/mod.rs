//! Architecture-independent software time mechanisms.

mod queue;

pub use queue::{
    DeadlineQueue, Error as TimerQueueError, QueueStats, TimerCallback, TimerEvent, TimerHandle,
    TimerMode,
};
