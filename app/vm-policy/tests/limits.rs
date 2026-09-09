// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::{FLEET_CONTROL_PLANE_HEADROOM, INITIAL_VM_FLEET_LIMITS, INITIAL_VM_LIMITS};

#[test]
fn fleet_domain_contains_one_instance_with_manager_headroom() {
    let fleet = INITIAL_VM_FLEET_LIMITS;
    let instance = INITIAL_VM_LIMITS;
    let headroom = FLEET_CONTROL_PLANE_HEADROOM;
    assert_eq!(
        fleet.kernel_memory_bytes,
        instance.kernel_memory_bytes + headroom.kernel_memory_bytes
    );
    assert_eq!(fleet.processes, instance.processes + headroom.processes);
    assert_eq!(fleet.threads, instance.threads + headroom.threads);
    assert_eq!(fleet.handles, instance.handles + headroom.handles);
    assert_eq!(
        fleet.kernel_objects,
        instance.kernel_objects + headroom.kernel_objects
    );
    assert_eq!(
        fleet.committed_pages,
        instance.committed_pages + headroom.committed_pages
    );
    assert_eq!(
        fleet.pinned_pages,
        instance.pinned_pages + headroom.pinned_pages
    );
    assert_eq!(
        fleet.guest_pages,
        instance.guest_pages + headroom.guest_pages
    );
    assert_eq!(
        fleet.ipc_messages,
        instance.ipc_messages + headroom.ipc_messages
    );
    assert_eq!(fleet.ipc_bytes, instance.ipc_bytes + headroom.ipc_bytes);
    assert_eq!(
        fleet.ipc_handles,
        instance.ipc_handles + headroom.ipc_handles
    );
    assert_eq!(
        fleet.subscriptions,
        instance.subscriptions + headroom.subscriptions
    );
    assert_eq!(fleet.timers, instance.timers + headroom.timers);
    assert_eq!(
        fleet.virtual_machines,
        instance.virtual_machines + headroom.virtual_machines
    );
    assert_eq!(
        fleet.virtual_cpus,
        instance.virtual_cpus + headroom.virtual_cpus
    );
    assert_eq!(
        fleet.device_leases,
        instance.device_leases + headroom.device_leases
    );
    assert_eq!(
        fleet.dma_mappings,
        instance.dma_mappings + headroom.dma_mappings
    );
    assert_eq!(
        fleet.user_address_spaces,
        instance.user_address_spaces + headroom.user_address_spaces
    );
    assert_eq!(
        fleet.user_mappings,
        instance.user_mappings + headroom.user_mappings
    );
}
