//! CPU topology, SMP startup, and per-CPU lifecycle.

mod smp;

pub(crate) use smp::online_cpu_count;
pub use smp::{Capabilities, Error, secondary_entry};

/// Starts secondary CPUs and waits for their local kernel state to come online.
pub(crate) fn initialize(boot: &mut super::boot::Initialization) {
    let capabilities = match super::boot::with_boot_state(|state| {
        smp::initialize(
            &state.platform,
            state.memory.root_address(),
            state.image_physical_start,
            state.memory.kernel_base(),
        )
    }) {
        Ok(capabilities) => capabilities,
        Err(error) => super::boot::fail("SMP initialization", error),
    };
    crate::println!(
        "HypeR: SMP online: {}/{} discovered CPUs",
        capabilities.online_cpus,
        capabilities.discovered_cpus
    );
    boot.set_cpus(capabilities);
}
