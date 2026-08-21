// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Virtual GIC pending, active, maintenance, and snapshot semantics.

use hyper::vm::interrupt::gicv3::{decode_list_register, encode_list_register};
use hyper::vm::interrupt::{
    Error, InterruptGroup, InterruptTrigger, ListEntry, ListState, VirtualCpuId,
    VirtualInterruptController, VirtualInterruptId,
};

fn interrupt(value: u32) -> VirtualInterruptId {
    crate::require_some(VirtualInterruptId::new(value))
}

#[test]
fn schedules_pending_interrupts_by_priority_and_cpu() {
    let mut vgic = crate::require_ok(VirtualInterruptController::new(2));
    let cpu0 = VirtualCpuId::new(0);
    let cpu1 = VirtualCpuId::new(1);
    for (id, cpu, priority) in [(27, cpu0, 0x80), (27, cpu1, 0x70), (40, cpu0, 0x20)] {
        crate::require_ok(vgic.configure(
            interrupt(id),
            cpu,
            priority,
            InterruptGroup::Group1,
            InterruptTrigger::Level,
        ));
        crate::require_ok(vgic.set_enabled(interrupt(id), cpu, true));
        crate::require_ok(vgic.inject(interrupt(id), cpu));
    }

    let mut slots = [None; 2];
    assert_eq!(crate::require_ok(vgic.refill(cpu0, &mut slots)), 2);
    assert_eq!(slots[0].map(|entry| entry.interrupt), Some(interrupt(40)));
    assert_eq!(slots[1].map(|entry| entry.interrupt), Some(interrupt(27)));

    let mut other_slots = [None; 1];
    assert_eq!(crate::require_ok(vgic.refill(cpu1, &mut other_slots)), 1);
    assert_eq!(
        other_slots[0].map(|entry| entry.interrupt),
        Some(interrupt(27))
    );
}

#[test]
fn requests_eoi_maintenance_for_a_virtual_timer_ppi() {
    let mut vgic = crate::require_ok(VirtualInterruptController::new(1));
    let cpu = VirtualCpuId::new(0);
    let timer = interrupt(27);
    crate::require_ok(vgic.configure(
        timer,
        cpu,
        0x80,
        InterruptGroup::Group1,
        InterruptTrigger::Level,
    ));
    crate::require_ok(vgic.set_maintenance_on_eoi(timer, cpu, true));
    crate::require_ok(vgic.set_enabled(timer, cpu, true));
    crate::require_ok(vgic.inject(timer, cpu));
    let mut slots = [None; 1];
    assert_eq!(crate::require_ok(vgic.refill(cpu, &mut slots)), 1);
    assert_eq!(
        slots[0].map(|entry| entry.request_eoi_maintenance),
        Some(true)
    );
}

#[test]
fn tracks_active_reinjection_and_guest_completion() {
    let mut vgic = crate::require_ok(VirtualInterruptController::new(1));
    let cpu = VirtualCpuId::new(0);
    let id = interrupt(48);
    crate::require_ok(vgic.configure(
        id,
        cpu,
        0x40,
        InterruptGroup::Group1,
        InterruptTrigger::Level,
    ));
    crate::require_ok(vgic.set_enabled(id, cpu, true));
    crate::require_ok(vgic.inject(id, cpu));

    let mut slots = [None; 1];
    assert_eq!(crate::require_ok(vgic.refill(cpu, &mut slots)), 1);
    slots[0] = Some(ListEntry {
        interrupt: id,
        priority: 0x40,
        group: InterruptGroup::Group1,
        state: ListState::Active,
        request_eoi_maintenance: true,
    });
    crate::require_ok(vgic.synchronize(cpu, &slots));
    crate::require_ok(vgic.inject(id, cpu));
    assert_eq!(crate::require_ok(vgic.refill(cpu, &mut slots)), 0);
    assert_eq!(
        slots[0].map(|entry| entry.state),
        Some(ListState::PendingActive)
    );

    crate::require_ok(vgic.synchronize(cpu, &[None]));
    let snapshot = crate::require_ok(vgic.snapshot(id, cpu));
    assert!(!snapshot.pending);
    assert!(!snapshot.active);
    assert!(!snapshot.listed);
}

#[test]
fn withdraws_disabled_pending_entries_without_losing_pending_state() {
    let mut vgic = crate::require_ok(VirtualInterruptController::new(1));
    let cpu = VirtualCpuId::new(0);
    let id = interrupt(72);
    crate::require_ok(vgic.configure(
        id,
        cpu,
        0x80,
        InterruptGroup::Group1,
        InterruptTrigger::Level,
    ));
    crate::require_ok(vgic.set_enabled(id, cpu, true));
    crate::require_ok(vgic.inject(id, cpu));
    let mut slots = [None; 1];
    assert_eq!(crate::require_ok(vgic.refill(cpu, &mut slots)), 1);

    crate::require_ok(vgic.set_enabled(id, cpu, false));
    assert_eq!(crate::require_ok(vgic.refill(cpu, &mut slots)), 0);
    assert_eq!(slots, [None]);
    let snapshot = crate::require_ok(vgic.snapshot(id, cpu));
    assert!(snapshot.pending);
    assert!(!snapshot.listed);

    crate::require_ok(vgic.set_priority(id, cpu, 0x20));
    crate::require_ok(vgic.set_enabled(id, cpu, true));
    assert_eq!(crate::require_ok(vgic.refill(cpu, &mut slots)), 1);
    assert_eq!(slots[0].map(|entry| entry.priority), Some(0x20));
}

#[test]
fn rejects_duplicate_spis_and_malformed_snapshots() {
    let mut vgic = crate::require_ok(VirtualInterruptController::new(2));
    let cpu0 = VirtualCpuId::new(0);
    let cpu1 = VirtualCpuId::new(1);
    let id = interrupt(64);
    crate::require_ok(vgic.configure(
        id,
        cpu0,
        0x80,
        InterruptGroup::Group1,
        InterruptTrigger::Edge,
    ));
    assert_eq!(
        vgic.configure(
            id,
            cpu1,
            0x80,
            InterruptGroup::Group1,
            InterruptTrigger::Edge,
        ),
        Err(Error::AlreadyConfigured)
    );
    let duplicate = Some(ListEntry {
        interrupt: id,
        priority: 0x80,
        group: InterruptGroup::Group1,
        state: ListState::Pending,
        request_eoi_maintenance: true,
    });
    assert_eq!(
        vgic.synchronize(cpu0, &[duplicate, duplicate]),
        Err(Error::SnapshotContainsDuplicate)
    );
}

#[test]
fn encodes_the_gicv3_list_register_layout() {
    let entry = ListEntry {
        interrupt: interrupt(55),
        priority: 0xa0,
        group: InterruptGroup::Group1,
        state: ListState::PendingActive,
        request_eoi_maintenance: true,
    };
    let encoded = encode_list_register(Some(entry));
    assert_eq!(
        encoded,
        55 | (1 << 41) | (0xa0 << 48) | (1 << 60) | (3 << 62)
    );
    assert_eq!(
        crate::require_ok(decode_list_register(encoded)),
        Some(entry)
    );
    assert_eq!(crate::require_ok(decode_list_register(0)), None);
}
