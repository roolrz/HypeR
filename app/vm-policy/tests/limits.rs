// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::{
    BUSINESS_VM_CAPACITY, FLEET_CONTROL_PLANE_HEADROOM, INITIAL_VM_FLEET_LIMITS, INITIAL_VM_LIMITS,
    IO_RUNTIME_LIMITS,
};

#[test]
fn fleet_domain_contains_all_business_slots_and_resident_io() {
    let fleet = INITIAL_VM_FLEET_LIMITS;
    let instance = INITIAL_VM_LIMITS;
    let headroom = FLEET_CONTROL_PLANE_HEADROOM;
    let resident = IO_RUNTIME_LIMITS;
    assert_eq!(
        BUSINESS_VM_CAPACITY as usize + 1,
        hyper_os::vm::IO_MAX_CLIENTS
    );
    assert_eq!(
        fleet.kernel_memory_bytes,
        BUSINESS_VM_CAPACITY * instance.kernel_memory_bytes
            + resident.kernel_memory_bytes
            + headroom.kernel_memory_bytes
    );
    assert_eq!(
        fleet.processes,
        BUSINESS_VM_CAPACITY * instance.processes + resident.processes + headroom.processes
    );
    assert_eq!(
        fleet.threads,
        BUSINESS_VM_CAPACITY * instance.threads + resident.threads + headroom.threads
    );
    assert_eq!(
        fleet.handles,
        BUSINESS_VM_CAPACITY * instance.handles + resident.handles + headroom.handles
    );
    assert_eq!(
        fleet.kernel_objects,
        BUSINESS_VM_CAPACITY * instance.kernel_objects
            + resident.kernel_objects
            + headroom.kernel_objects
    );
    assert_eq!(
        fleet.committed_pages,
        BUSINESS_VM_CAPACITY * instance.committed_pages
            + resident.committed_pages
            + headroom.committed_pages
    );
    assert_eq!(
        fleet.pinned_pages,
        BUSINESS_VM_CAPACITY * instance.pinned_pages
            + resident.pinned_pages
            + headroom.pinned_pages
    );
    assert_eq!(
        fleet.guest_pages,
        BUSINESS_VM_CAPACITY * instance.guest_pages + resident.guest_pages + headroom.guest_pages
    );
    assert_eq!(
        fleet.ipc_messages,
        BUSINESS_VM_CAPACITY * instance.ipc_messages
            + resident.ipc_messages
            + headroom.ipc_messages
    );
    assert_eq!(
        fleet.ipc_bytes,
        BUSINESS_VM_CAPACITY * instance.ipc_bytes + resident.ipc_bytes + headroom.ipc_bytes
    );
    assert_eq!(
        fleet.ipc_handles,
        BUSINESS_VM_CAPACITY * instance.ipc_handles + resident.ipc_handles + headroom.ipc_handles
    );
    assert_eq!(
        fleet.subscriptions,
        BUSINESS_VM_CAPACITY * instance.subscriptions
            + resident.subscriptions
            + headroom.subscriptions
    );
    assert_eq!(
        fleet.timers,
        BUSINESS_VM_CAPACITY * instance.timers + resident.timers + headroom.timers
    );
    assert_eq!(
        fleet.virtual_machines,
        BUSINESS_VM_CAPACITY * instance.virtual_machines
            + resident.virtual_machines
            + headroom.virtual_machines
    );
    assert_eq!(
        fleet.virtual_cpus,
        BUSINESS_VM_CAPACITY * instance.virtual_cpus
            + resident.virtual_cpus
            + headroom.virtual_cpus
    );
    assert_eq!(
        fleet.device_leases,
        BUSINESS_VM_CAPACITY * instance.device_leases
            + resident.device_leases
            + headroom.device_leases
    );
    assert_eq!(
        fleet.dma_mappings,
        BUSINESS_VM_CAPACITY * instance.dma_mappings
            + resident.dma_mappings
            + headroom.dma_mappings
    );
    assert_eq!(
        fleet.user_address_spaces,
        BUSINESS_VM_CAPACITY * instance.user_address_spaces
            + resident.user_address_spaces
            + headroom.user_address_spaces
    );
    assert_eq!(
        fleet.user_mappings,
        BUSINESS_VM_CAPACITY * instance.user_mappings
            + resident.user_mappings
            + headroom.user_mappings
    );
}

#[test]
fn resident_io_and_two_full_guests_fit_without_weakening_child_limits() {
    assert_eq!(INITIAL_VM_LIMITS.guest_pages, 64 * 1024);
    assert_eq!(INITIAL_VM_LIMITS.virtual_machines, 1);
    assert!(
        IO_RUNTIME_LIMITS.guest_pages + 2 * INITIAL_VM_LIMITS.guest_pages
            <= INITIAL_VM_FLEET_LIMITS.guest_pages
    );
    assert!(IO_RUNTIME_LIMITS.virtual_machines + 2 <= INITIAL_VM_FLEET_LIMITS.virtual_machines);
    // Filling every business slot plus I/O leaves no unaccounted VM slot.
    assert_eq!(
        INITIAL_VM_FLEET_LIMITS.virtual_machines,
        BUSINESS_VM_CAPACITY + 1
    );
}
