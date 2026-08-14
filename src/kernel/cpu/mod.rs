//! CPU topology, SMP startup, and per-CPU lifecycle.

mod smp;

pub(crate) use smp::online_cpu_count;
pub use smp::{Capabilities, Error, initialize, secondary_entry};
