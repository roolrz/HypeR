// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

#[test]
fn mailbox_owner_pin_spans_the_complete_transaction() {
    let source = include_str!("../../../../src/kernel/irq/cross_call.rs");
    let acquire_start = crate::require_some(source.find("impl Owner {"));
    let acquire_end =
        crate::require_some(source[acquire_start..].find("/// Linear reservation")) + acquire_start;
    let acquire_body = &source[acquire_start..acquire_end];
    let drop_start = crate::require_some(source.find("impl Drop for Owner {"));
    let drop_end =
        crate::require_some(source[drop_start..].find("fn release_owner_pin(")) + drop_start;
    let drop_body = &source[drop_start..drop_end];
    let execute_start = crate::require_some(source.find("fn execute_owned("));
    let execute_end =
        crate::require_some(source[execute_start..].find("fn next_generation()")) + execute_start;
    let execute_body = &source[execute_start..execute_end];

    let acquire = crate::require_some(acquire_body.find("scheduler::preempt_disable()"));
    let claim = crate::require_some(acquire_body.find("OWNER.compare_exchange("));
    assert!(acquire < claim);
    assert!(acquire_body.contains("release_owner_pin(pin)"));

    let unpublish =
        crate::require_some(drop_body.find("PUBLISHED_GENERATION.store(0, Ordering::Release)"));
    let release_owner =
        crate::require_some(drop_body.find("OWNER.store(false, Ordering::Release)"));
    let release_pin = crate::require_some(drop_body.find("release_owner_pin(pin)"));
    assert!(unpublish < release_owner);
    assert!(release_owner < release_pin);

    let publish = crate::require_some(
        execute_body.find("PUBLISHED_GENERATION.store(generation, Ordering::Release)"),
    );
    let local = crate::require_some(execute_body.find("service_local_irq_mailbox()"));
    let notify = crate::require_some(execute_body.find("notify_remote_targets("));
    let wait = crate::require_some(execute_body.find("await_acknowledgements("));
    let unpublish =
        crate::require_some(execute_body.find("PUBLISHED_GENERATION.store(0, Ordering::Release)"));
    assert!(publish < local);
    assert!(local < notify);
    assert!(notify < wait);
    assert!(wait < unpublish);
    assert!(!execute_body.contains("preempt_disable"));
    assert!(
        !source[acquire_start..release_pin + drop_start].contains("preempt_enable_and_reschedule")
    );
}

#[test]
fn native_mapping_contention_waits_before_a_logical_cut() {
    let transport = include_str!("../../../../src/kernel/irq/cross_call.rs");
    let waiting_start = crate::require_some(transport.find("pub(crate) fn acquire_waiting()"));
    let waiting_end =
        crate::require_some(transport[waiting_start..].find("pub(crate) fn execute("))
            + waiting_start;
    let waiting = &transport[waiting_start..waiting_end];
    assert!(waiting.contains("Err(AcquireError::Busy)"));
    assert!(waiting.contains("scheduler::yield_now()"));
    assert!(waiting.contains("Err(AcquireError::Unavailable) => return Err(())"));

    let machine = include_str!("../../../../src/kernel/mm/user_space/machine.rs");
    let commit_start = crate::require_some(machine.find("pub(crate) fn commit(self)"));
    let commit_end = crate::require_some(machine[commit_start..].find("let cut =")) + commit_start;
    let commit_prefix = &machine[commit_start..commit_end];
    assert!(commit_prefix.contains("UserAddressSpaceTransaction::acquire_waiting()"));
    assert!(!commit_prefix.contains("try_acquire"));
}

#[test]
fn publisher_pin_release_is_checked_without_scheduling() {
    let source = include_str!("../../../../src/kernel/task/scheduler/mod.rs");
    let start =
        crate::require_some(source.find("pub(crate) fn preempt_enable_without_reschedule("));
    let tail = &source[start..];
    let end = crate::require_some(tail.find("\n}")) + 2;
    let body = &tail[..end];

    assert!(body.contains("guard.0.release()"));
    assert!(!body.contains("cond_resched"));
    assert!(!body.contains("local_enabled"));
}
