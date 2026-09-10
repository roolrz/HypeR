// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::{FLEET_CONTROL_PLANE_HEADROOM, INITIAL_VM_FLEET_LIMITS, INITIAL_VM_LIMITS};

#[test]
fn fleet_domain_contains_two_instances_with_manager_headroom() {
    let fleet = INITIAL_VM_FLEET_LIMITS;
    let instance = INITIAL_VM_LIMITS;
    let headroom = FLEET_CONTROL_PLANE_HEADROOM;
    assert_eq!(
        fleet.kernel_memory_bytes,
        2 * instance.kernel_memory_bytes + headroom.kernel_memory_bytes
    );
    assert_eq!(fleet.processes, 2 * instance.processes + headroom.processes);
    assert_eq!(fleet.threads, 2 * instance.threads + headroom.threads);
    assert_eq!(fleet.handles, 2 * instance.handles + headroom.handles);
    assert_eq!(
        fleet.kernel_objects,
        2 * instance.kernel_objects + headroom.kernel_objects
    );
    assert_eq!(
        fleet.committed_pages,
        2 * instance.committed_pages + headroom.committed_pages
    );
    assert_eq!(
        fleet.pinned_pages,
        2 * instance.pinned_pages + headroom.pinned_pages
    );
    assert_eq!(
        fleet.guest_pages,
        2 * instance.guest_pages + headroom.guest_pages
    );
    assert_eq!(
        fleet.ipc_messages,
        2 * instance.ipc_messages + headroom.ipc_messages
    );
    assert_eq!(fleet.ipc_bytes, 2 * instance.ipc_bytes + headroom.ipc_bytes);
    assert_eq!(
        fleet.ipc_handles,
        2 * instance.ipc_handles + headroom.ipc_handles
    );
    assert_eq!(
        fleet.subscriptions,
        2 * instance.subscriptions + headroom.subscriptions
    );
    assert_eq!(fleet.timers, 2 * instance.timers + headroom.timers);
    assert_eq!(
        fleet.virtual_machines,
        2 * instance.virtual_machines + headroom.virtual_machines
    );
    assert_eq!(
        fleet.virtual_cpus,
        2 * instance.virtual_cpus + headroom.virtual_cpus
    );
    assert_eq!(
        fleet.device_leases,
        2 * instance.device_leases + headroom.device_leases
    );
    assert_eq!(
        fleet.dma_mappings,
        2 * instance.dma_mappings + headroom.dma_mappings
    );
    assert_eq!(
        fleet.user_address_spaces,
        2 * instance.user_address_spaces + headroom.user_address_spaces
    );
    assert_eq!(
        fleet.user_mappings,
        2 * instance.user_mappings + headroom.user_mappings
    );
}
