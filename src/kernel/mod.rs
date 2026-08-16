//! Architecture-independent kernel policy, grouped by subsystem.

pub(crate) mod boot;
pub mod cpu;
pub mod crash;
pub mod debug;
pub mod device;
pub mod irq;
pub mod log;
pub mod mm;
pub mod sync;
pub mod task;
pub mod time;
pub mod vm;
