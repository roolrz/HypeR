//! Architecture-independent kernel policy, grouped by subsystem.

pub(crate) mod boot;
pub mod cpu;
pub mod crash;
pub mod debug;
pub mod device;
pub mod irq;
pub mod log;
pub mod mm;
pub mod task;
pub mod time;
pub mod vm;

pub(crate) use boot::prepare_boot_environment;

// Stable facades for existing kernel callers. New internal code should prefer
// the subsystem-qualified paths above.
pub use device::cpu_power;
pub(crate) use irq::{exception, interrupt};
pub use mm::{allocator, memory};
pub use task::{scheduler, thread};
