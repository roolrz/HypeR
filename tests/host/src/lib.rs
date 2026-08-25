// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Host-side tests for architecture-independent HypeR mechanisms.
//!
//! Each test module follows a production subsystem or a distinct hardware or
//! lifecycle contract. Keep shared helpers here deliberately small so tests do
//! not acquire a second framework of their own.

#[cfg(test)]
fn require_ok<T, E: core::fmt::Debug>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("required success, received {error:?}"),
    }
}

#[cfg(test)]
fn require_some<T>(value: Option<T>) -> T {
    match value {
        Some(value) => value,
        None => panic!("required a value"),
    }
}

#[cfg(test)]
#[path = "cases/allocation.rs"]
mod allocation;
#[cfg(test)]
#[path = "cases/boot_allocator.rs"]
mod boot_allocator;
#[cfg(test)]
#[path = "cases/cache_publication.rs"]
mod cache_publication;
#[cfg(test)]
#[path = "cases/cpio.rs"]
mod cpio;
#[cfg(test)]
#[path = "cases/cpu.rs"]
mod cpu;
#[cfg(test)]
#[path = "cases/fallible_ownership.rs"]
mod fallible_ownership;
#[cfg(test)]
#[path = "cases/fdt.rs"]
mod fdt;
#[cfg(test)]
#[path = "cases/foreign_memory_copy.rs"]
mod foreign_memory_copy;
#[cfg(test)]
#[path = "cases/generic_timer.rs"]
mod generic_timer;
#[cfg(test)]
#[path = "cases/gicv3.rs"]
mod gicv3;
#[cfg(test)]
#[path = "cases/kallsyms.rs"]
mod kallsyms;
#[cfg(test)]
#[path = "cases/kaslr.rs"]
mod kaslr;
#[cfg(test)]
#[path = "cases/kernel_log.rs"]
mod kernel_log;
#[cfg(test)]
#[path = "cases/mmio_resources.rs"]
mod mmio_resources;
#[cfg(test)]
#[path = "cases/ns16550.rs"]
mod ns16550;
#[cfg(test)]
#[path = "cases/physical_ranges.rs"]
mod physical_ranges;
#[cfg(test)]
#[path = "cases/plic.rs"]
mod plic;
#[cfg(test)]
#[path = "cases/psci.rs"]
mod psci;
#[cfg(test)]
#[path = "cases/runtime_allocators.rs"]
mod runtime_allocators;
#[cfg(test)]
#[path = "cases/scheduler_policy.rs"]
mod scheduler_policy;
#[cfg(test)]
#[path = "cases/scheduler_requests.rs"]
mod scheduler_requests;
#[cfg(test)]
#[path = "cases/software_timers.rs"]
mod software_timers;
#[cfg(test)]
#[path = "cases/synchronization.rs"]
mod synchronization;
#[cfg(test)]
#[path = "cases/vgic.rs"]
mod vgic;
#[cfg(test)]
#[path = "cases/virtual_legacy_pc.rs"]
mod virtual_legacy_pc;
#[cfg(test)]
#[path = "cases/virtual_pl011.rs"]
mod virtual_pl011;
#[cfg(test)]
#[path = "cases/x86_svm_contract.rs"]
mod x86_svm_contract;
#[cfg(test)]
#[path = "cases/x86_virtual_cpu_contract.rs"]
mod x86_virtual_cpu_contract;
