// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Resource-containment policy for the initial Native VM fleet.

use hyper_os::task::ResourceLimits;

/// Limit installed on each disposable VM runtime domain.
pub const INITIAL_VM_LIMITS: ResourceLimits = ResourceLimits {
    kernel_memory_bytes: 64 * 1024 * 1024,
    processes: 8,
    threads: 16,
    handles: 256,
    kernel_objects: 1024,
    committed_pages: 40 * 1024,
    pinned_pages: 33 * 1024,
    guest_pages: 32 * 1024,
    ipc_messages: 128,
    ipc_bytes: 1024 * 1024,
    ipc_handles: 64,
    subscriptions: 128,
    timers: 128,
    virtual_machines: 1,
    virtual_cpus: 1,
    device_leases: 8,
    dma_mappings: 64,
    user_address_spaces: 8,
    user_mappings: 256,
};

const FLEET_CONTROL_PLANE_HEADROOM: ResourceLimits = ResourceLimits {
    kernel_memory_bytes: 64 * 1024 * 1024,
    processes: 4,
    threads: 8,
    handles: 256,
    kernel_objects: 1024,
    committed_pages: 8 * 1024,
    pinned_pages: 1024,
    guest_pages: 0,
    ipc_messages: 128,
    ipc_bytes: 1024 * 1024,
    ipc_handles: 64,
    subscriptions: 128,
    timers: 128,
    virtual_machines: 0,
    virtual_cpus: 0,
    device_leases: 8,
    dma_mappings: 64,
    user_address_spaces: 4,
    user_mappings: 256,
};

/// Aggregate limit installed above the VM manager and all of its descendants.
///
/// The current policy admits one VM and leaves bounded headroom for the manager
/// process and its control-plane objects. Per-instance child domains remain
/// useful for teardown and finer accounting, but this ancestor is the security
/// boundary which prevents delegated domain creation from charging init's
/// shared service domain without limit.
pub const INITIAL_VM_FLEET_LIMITS: ResourceLimits = ResourceLimits {
    kernel_memory_bytes: INITIAL_VM_LIMITS.kernel_memory_bytes
        + FLEET_CONTROL_PLANE_HEADROOM.kernel_memory_bytes,
    processes: INITIAL_VM_LIMITS.processes + FLEET_CONTROL_PLANE_HEADROOM.processes,
    threads: INITIAL_VM_LIMITS.threads + FLEET_CONTROL_PLANE_HEADROOM.threads,
    handles: INITIAL_VM_LIMITS.handles + FLEET_CONTROL_PLANE_HEADROOM.handles,
    kernel_objects: INITIAL_VM_LIMITS.kernel_objects + FLEET_CONTROL_PLANE_HEADROOM.kernel_objects,
    committed_pages: INITIAL_VM_LIMITS.committed_pages
        + FLEET_CONTROL_PLANE_HEADROOM.committed_pages,
    pinned_pages: INITIAL_VM_LIMITS.pinned_pages + FLEET_CONTROL_PLANE_HEADROOM.pinned_pages,
    guest_pages: INITIAL_VM_LIMITS.guest_pages + FLEET_CONTROL_PLANE_HEADROOM.guest_pages,
    ipc_messages: INITIAL_VM_LIMITS.ipc_messages + FLEET_CONTROL_PLANE_HEADROOM.ipc_messages,
    ipc_bytes: INITIAL_VM_LIMITS.ipc_bytes + FLEET_CONTROL_PLANE_HEADROOM.ipc_bytes,
    ipc_handles: INITIAL_VM_LIMITS.ipc_handles + FLEET_CONTROL_PLANE_HEADROOM.ipc_handles,
    subscriptions: INITIAL_VM_LIMITS.subscriptions + FLEET_CONTROL_PLANE_HEADROOM.subscriptions,
    timers: INITIAL_VM_LIMITS.timers + FLEET_CONTROL_PLANE_HEADROOM.timers,
    virtual_machines: INITIAL_VM_LIMITS.virtual_machines
        + FLEET_CONTROL_PLANE_HEADROOM.virtual_machines,
    virtual_cpus: INITIAL_VM_LIMITS.virtual_cpus + FLEET_CONTROL_PLANE_HEADROOM.virtual_cpus,
    device_leases: INITIAL_VM_LIMITS.device_leases + FLEET_CONTROL_PLANE_HEADROOM.device_leases,
    dma_mappings: INITIAL_VM_LIMITS.dma_mappings + FLEET_CONTROL_PLANE_HEADROOM.dma_mappings,
    user_address_spaces: INITIAL_VM_LIMITS.user_address_spaces
        + FLEET_CONTROL_PLANE_HEADROOM.user_address_spaces,
    user_mappings: INITIAL_VM_LIMITS.user_mappings + FLEET_CONTROL_PLANE_HEADROOM.user_mappings,
};

#[cfg(test)]
#[path = "../tests/limits.rs"]
mod tests;
