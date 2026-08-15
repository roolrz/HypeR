//! Architecture-independent kernel policy, grouped by subsystem.

mod boot;
pub mod cpu;
pub mod debug;
pub mod device;
pub mod irq;
pub mod log;
pub mod mm;
pub mod task;
pub mod time;
pub mod vm;

pub use boot::{boot, finish_boot};

// Stable facades for existing kernel callers. New internal code should prefer
// the subsystem-qualified paths above.
pub use device::cpu_power;
pub(crate) use irq::{exception, interrupt};
pub use mm::{allocator, memory};
pub use task::{scheduler, thread};
