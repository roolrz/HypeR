// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Kernel object and process-local generation-handle transactions.

use core::num::NonZeroU32;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[path = "../../../../src/kernel/capability/mod.rs"]
#[allow(dead_code, unused_imports)]
mod capability;

use capability::{
    HandleError, HandleFlags, HandleTable, HandleValue, KernelObject, ObjectKind, ObjectRef,
    PreparedHandle, Rights,
};

const TEST_KIND: ObjectKind = match NonZeroU32::new(0x7fff_ff01) {
    Some(value) => ObjectKind::for_test(value),
    None => panic!("test object kind must be nonzero"),
};
const OTHER_KIND: ObjectKind = match NonZeroU32::new(0x7fff_ff02) {
    Some(value) => ObjectKind::for_test(value),
    None => panic!("test object kind must be nonzero"),
};

struct TestObject {
    value: u64,
    zero_transitions: Arc<AtomicUsize>,
}

impl capability::private::Sealed for TestObject {}

impl KernelObject for TestObject {
    const KIND: ObjectKind = TEST_KIND;
    const SUPPORTED_RIGHTS: Rights = Rights::DUPLICATE
        .union(Rights::TRANSFER)
        .union(Rights::WAIT)
        .union(Rights::INSPECT);

    fn on_zero_active_handles(&self) {
        self.zero_transitions.fetch_add(1, Ordering::Relaxed);
    }
}

struct OtherObject;

impl capability::private::Sealed for OtherObject {}

impl KernelObject for OtherObject {
    const KIND: ObjectKind = OTHER_KIND;
    const SUPPORTED_RIGHTS: Rights = Rights::INSPECT;
}

fn object(value: u64, zero_transitions: &Arc<AtomicUsize>) -> ObjectRef {
    crate::require_ok(ObjectRef::try_new(TestObject {
        value,
        zero_transitions: zero_transitions.clone(),
    }))
}

fn prepared(object: ObjectRef, rights: Rights) -> PreparedHandle {
    crate::require_ok(PreparedHandle::try_from_new_object(
        object,
        rights,
        HandleFlags::NONE,
    ))
}

fn remove_all(table: &mut HandleTable) {
    let mut cursor = crate::require_ok(table.begin_teardown());
    while let Some(closed) = table.remove_next(&mut cursor) {
        closed.complete();
    }
    table.finish_teardown(cursor);
}

#[test]
fn reservation_values_are_unresolvable_until_one_final_publication() {
    let transitions = Arc::new(AtomicUsize::new(0));
    let first_object = object(17, &transitions);
    let mut table = HandleTable::new();
    let reservation = crate::require_ok(table.reserve::<2>());
    let values = reservation.values();
    let second_transitions = Arc::new(AtomicUsize::new(0));
    let second_object = object(18, &second_transitions);
    let handles = [
        prepared(first_object.clone(), Rights::INSPECT),
        prepared(second_object, Rights::INSPECT),
    ];
    let published = reservation.publish(&mut table, handles);

    assert_eq!(published, values);
    assert_eq!(first_object.active_handle_count(), 1);
    assert_eq!(
        crate::require_ok(table.get_info(values[0])).koid,
        first_object.koid()
    );
    assert_eq!(crate::require_ok(table.get_info(values[1])).kind, TEST_KIND);
    assert_eq!(transitions.load(Ordering::Relaxed), 0);
    remove_all(&mut table);
    assert_eq!(transitions.load(Ordering::Relaxed), 1);
}

#[test]
fn aborted_reservations_invalidate_every_exposed_future_value() {
    let mut table = HandleTable::new();
    assert!(table.free_list_is_consistent_for_test());
    let first_values = {
        let reservation = crate::require_ok(table.reserve::<2>());
        let values = reservation.values();
        reservation.abort(&mut table);
        values
    };
    assert!(table.free_list_is_consistent_for_test());
    assert_eq!(
        table.get_info(first_values[0]),
        Err(HandleError::InvalidHandle)
    );

    let second_values = {
        let reservation = crate::require_ok(table.reserve::<2>());
        let values = reservation.values();
        reservation.abort(&mut table);
        values
    };
    assert!(table.free_list_is_consistent_for_test());
    assert_ne!(second_values, first_values);
    assert_eq!(
        table.get_info(first_values[1]),
        Err(HandleError::InvalidHandle)
    );
}

#[test]
fn untrusted_handle_values_are_validated_before_slot_arithmetic() {
    assert_eq!(
        HandleValue::try_from_raw(0),
        Err(HandleError::InvalidHandle)
    );
    assert_eq!(
        HandleValue::try_from_raw(1),
        Err(HandleError::InvalidHandle)
    );
    assert_eq!(
        HandleValue::try_from_raw(1 << 24),
        Err(HandleError::InvalidHandle)
    );
    assert_eq!(
        crate::require_ok(HandleValue::try_from_raw((1 << 24) | 1)).get(),
        (1 << 24) | 1
    );
}

#[test]
fn closing_a_handle_invalidates_its_generation_before_slot_reuse() {
    let transitions = Arc::new(AtomicUsize::new(0));
    let first_object = object(1, &transitions);
    let mut table = HandleTable::new();
    let first = {
        let reservation = crate::require_ok(table.reserve::<1>());
        reservation.publish(&mut table, [prepared(first_object, Rights::INSPECT)])[0]
    };
    let closed = crate::require_ok(table.remove(first));
    assert!(table.free_list_is_consistent_for_test());
    assert_eq!(transitions.load(Ordering::Relaxed), 0);
    closed.complete();
    assert_eq!(transitions.load(Ordering::Relaxed), 1);
    assert_eq!(table.get_info(first), Err(HandleError::InvalidHandle));

    let second_object = object(2, &transitions);
    let second = {
        let reservation = crate::require_ok(table.reserve::<1>());
        reservation.publish(&mut table, [prepared(second_object, Rights::INSPECT)])[0]
    };
    assert!(table.free_list_is_consistent_for_test());
    assert_ne!(second, first);
    assert_eq!(table.get_info(first), Err(HandleError::InvalidHandle));
    assert_eq!(
        crate::require_ok(table.get_info(second)).rights,
        Rights::INSPECT
    );
    crate::require_ok(table.remove(second)).complete();
    assert!(table.free_list_is_consistent_for_test());
}

#[test]
fn duplicate_can_only_attenuate_existing_authority() {
    let transitions = Arc::new(AtomicUsize::new(0));
    let object = object(7, &transitions);
    let source_rights = Rights::DUPLICATE.union(Rights::INSPECT);
    let mut table = HandleTable::new();
    let source = {
        let reservation = crate::require_ok(table.reserve::<1>());
        reservation.publish(&mut table, [prepared(object.clone(), source_rights)])[0]
    };

    assert_eq!(
        table.duplicate(source, Rights::WAIT).err(),
        Some(HandleError::AccessDenied)
    );
    let duplicate = crate::require_ok(table.duplicate(source, Rights::INSPECT));
    assert_eq!(object.active_handle_count(), 2);
    let duplicate_value = {
        let reservation = crate::require_ok(table.reserve::<1>());
        reservation.publish(&mut table, [duplicate])[0]
    };
    assert_eq!(
        crate::require_ok(table.get_info(duplicate_value)).rights,
        Rights::INSPECT
    );
    assert_eq!(
        table.duplicate(duplicate_value, Rights::INSPECT).err(),
        Some(HandleError::AccessDenied)
    );
    remove_all(&mut table);
    assert_eq!(transitions.load(Ordering::Relaxed), 1);
}

#[test]
fn replace_moves_active_authority_in_one_infallible_commit() {
    let transitions = Arc::new(AtomicUsize::new(0));
    let object = object(8, &transitions);
    let source_rights = Rights::DUPLICATE.union(Rights::WAIT).union(Rights::INSPECT);
    let mut table = HandleTable::new();
    let source = {
        let reservation = crate::require_ok(table.reserve::<1>());
        reservation.publish(&mut table, [prepared(object.clone(), source_rights)])[0]
    };

    assert_eq!(
        table.replace(source, Rights::TRANSFER),
        Err(HandleError::AccessDenied)
    );
    assert_eq!(
        crate::require_ok(table.get_info(source)).rights,
        source_rights
    );
    let replacement = crate::require_ok(table.replace(source, Rights::WAIT));
    assert_ne!(replacement, source);
    assert_eq!(table.get_info(source), Err(HandleError::InvalidHandle));
    assert_eq!(
        crate::require_ok(table.get_info(replacement)).rights,
        Rights::WAIT
    );
    assert_eq!(object.active_handle_count(), 1);
    assert_eq!(transitions.load(Ordering::Relaxed), 0);
    crate::require_ok(table.remove(replacement)).complete();
    assert_eq!(transitions.load(Ordering::Relaxed), 1);
}

#[test]
fn replace_retires_an_exhausted_generation_without_losing_authority() {
    let transitions = Arc::new(AtomicUsize::new(0));
    let object = object(81, &transitions);
    let mut table = HandleTable::new();
    let source = {
        let reservation = crate::require_ok(table.reserve::<1>());
        reservation.publish(&mut table, [prepared(object.clone(), Rights::INSPECT)])[0]
    };
    let exhausted =
        table.set_occupied_generation_for_test(source, HandleTable::maximum_generation_for_test());

    let replacement = crate::require_ok(table.replace(exhausted, Rights::INSPECT));

    assert_ne!(replacement, exhausted);
    assert_eq!(table.get_info(exhausted), Err(HandleError::InvalidHandle));
    assert_eq!(object.active_handle_count(), 1);
    assert!(table.free_list_is_consistent_for_test());
    crate::require_ok(table.remove(replacement)).complete();
    assert_eq!(transitions.load(Ordering::Relaxed), 1);
}

#[test]
fn unknown_abi_right_bits_are_rejected() {
    assert_eq!(Rights::from_bits(1 << 63), None);
    assert_eq!(
        Rights::from_bits(Rights::DUPLICATE.bits()),
        Some(Rights::DUPLICATE)
    );
    assert!(!Rights::DUPLICATE.is_empty());
}

#[test]
fn typed_resolution_uses_compiler_checked_downcast_and_survives_close() {
    let transitions = Arc::new(AtomicUsize::new(0));
    let object = object(99, &transitions);
    let koid = object.koid();
    let mut table = HandleTable::new();
    let value = {
        let reservation = crate::require_ok(table.reserve::<1>());
        reservation.publish(&mut table, [prepared(object, Rights::INSPECT)])[0]
    };

    assert!(matches!(
        table.resolve::<OtherObject>(value, Rights::INSPECT),
        Err(HandleError::WrongObjectType)
    ));
    assert!(matches!(
        table.resolve::<TestObject>(value, Rights::WAIT),
        Err(HandleError::AccessDenied)
    ));
    let resolved = crate::require_ok(table.resolve::<TestObject>(value, Rights::INSPECT));
    assert_eq!(resolved.koid(), koid);
    let closed = crate::require_ok(table.remove(value));
    assert_eq!(transitions.load(Ordering::Relaxed), 0);
    closed.complete();
    assert_eq!(transitions.load(Ordering::Relaxed), 1);
    assert_eq!(resolved.object().value, 99);
}

#[test]
fn unpublished_active_handle_rollback_runs_the_zero_transition_once() {
    let transitions = Arc::new(AtomicUsize::new(0));
    let object = object(4, &transitions);
    let handle = prepared(object.clone(), Rights::INSPECT);
    assert_eq!(object.active_handle_count(), 1);
    assert_eq!(
        PreparedHandle::try_from_new_object(object.clone(), Rights::INSPECT, HandleFlags::NONE,)
            .err(),
        Some(HandleError::ObjectAlreadyActive)
    );
    drop(handle);

    assert!(object.is_retired());
    assert_eq!(transitions.load(Ordering::Relaxed), 1);
    assert_eq!(
        PreparedHandle::try_from_new_object(object, Rights::INSPECT, HandleFlags::NONE).err(),
        Some(HandleError::ObjectRetired)
    );
}

#[test]
fn concurrent_active_handle_owners_publish_one_zero_transition() {
    let transitions = Arc::new(AtomicUsize::new(0));
    let object = object(12, &transitions);
    let anchor = Arc::new(prepared(object.clone(), Rights::INSPECT));
    let barrier = Arc::new(std::sync::Barrier::new(9));
    let mut workers = Vec::new();
    for _ in 0..8 {
        let anchor = anchor.clone();
        let barrier = barrier.clone();
        workers.push(std::thread::spawn(move || {
            let handle = crate::require_ok(anchor.duplicate_for_test(Rights::INSPECT));
            barrier.wait();
            drop(handle);
        }));
    }

    barrier.wait();
    for worker in workers {
        assert!(worker.join().is_ok());
    }
    assert_eq!(object.active_handle_count(), 1);
    assert_eq!(transitions.load(Ordering::Relaxed), 0);
    drop(anchor);
    assert!(object.is_retired());
    assert_eq!(transitions.load(Ordering::Relaxed), 1);
}

#[test]
fn teardown_cursor_blocks_publication_and_scans_each_slot_once() {
    let transitions = Arc::new(AtomicUsize::new(0));
    let first_object = object(20, &transitions);
    let second_transitions = Arc::new(AtomicUsize::new(0));
    let second_object = object(21, &second_transitions);
    let mut table = HandleTable::new();
    let values = {
        let reservation = crate::require_ok(table.reserve::<2>());
        reservation.publish(
            &mut table,
            [
                prepared(first_object, Rights::INSPECT),
                prepared(second_object, Rights::INSPECT),
            ],
        )
    };

    let mut cursor = crate::require_ok(table.begin_teardown());
    assert_eq!(table.get_info(values[0]), Err(HandleError::TableRetired));
    assert!(matches!(
        table.reserve::<1>(),
        Err(HandleError::TableRetired)
    ));
    let first = crate::require_some(table.remove_next(&mut cursor));
    first.complete();
    let second = crate::require_some(table.remove_next(&mut cursor));
    second.complete();
    assert!(table.remove_next(&mut cursor).is_none());
    table.finish_teardown(cursor);
    assert_eq!(transitions.load(Ordering::Relaxed), 1);
}

#[test]
fn teardown_waits_for_detached_slot_reservations() {
    let mut table = HandleTable::new();
    let reservation = crate::require_ok(table.reserve::<1>());
    let future = reservation.values()[0];

    assert!(matches!(
        table.begin_teardown(),
        Err(HandleError::OutstandingReservation)
    ));
    assert_eq!(table.get_info(future), Err(HandleError::InvalidHandle));
    reservation.abort(&mut table);

    let cursor = crate::require_ok(table.begin_teardown());
    table.finish_teardown(cursor);
}

#[test]
fn koids_are_unique_but_do_not_participate_in_lookup() {
    let transitions = Arc::new(AtomicUsize::new(0));
    let first = object(1, &transitions);
    let second = object(2, &transitions);

    assert_ne!(first.koid(), second.koid());
    assert_ne!(first.koid().get(), 0);
    assert_eq!(first.kind().get(), TEST_KIND.get());
}
