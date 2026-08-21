// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! CPU topology, SMP startup, and per-CPU lifecycle.

mod smp;

pub(crate) use hyper::cpu::CpuIndex;
pub(crate) use smp::online_cpu_count;
pub use smp::{Capabilities, Error, secondary_entry};

/// Reads and validates the executing CPU's logical kernel index.
///
/// A separate capability token would not enforce a stronger invariant while
/// scheduler threads remain CPU-pinned, so callers receive the domain type
/// directly rather than an ornamental guard.
#[inline]
pub(crate) fn current_index() -> Option<CpuIndex> {
    crate::arch::cpu::current_index()
}

/// Starts secondary CPUs and waits for their local kernel state to come online.
pub(crate) fn initialize(boot: &mut super::boot::Initialization) -> Result<(), Error> {
    let (platform, root, image_physical_start, kernel_base) =
        super::boot::with_boot_state(|state| {
            (
                state.platform,
                state.memory.root_address(),
                state.image_physical_start,
                state.memory.kernel_base(),
            )
        });
    let capabilities = smp::initialize(&platform, root, image_physical_start, kernel_base)?;
    crate::println!(
        "HypeR: SMP online: {}/{} discovered CPUs",
        capabilities.online_cpus,
        capabilities.discovered_cpus
    );
    boot.set_cpus(capabilities);
    Ok(())
}
