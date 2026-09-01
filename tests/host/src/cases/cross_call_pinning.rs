// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

#[test]
fn synchronous_publisher_pin_spans_the_complete_mailbox_protocol() {
    let source = include_str!("../../../../src/kernel/irq/cross_call.rs");
    let start = crate::require_some(source.find("fn execute_owned("));
    let end = crate::require_some(source[start..].find("fn next_generation()")) + start;
    let body = &source[start..end];

    let acquire = crate::require_some(body.find("scheduler::preempt_disable()"));
    let publish =
        crate::require_some(body.find("PUBLISHED_GENERATION.store(generation, Ordering::Release)"));
    let local = crate::require_some(body.find("service_local_irq_mailbox()"));
    let notify = crate::require_some(body.find("notify_remote_targets("));
    let wait = crate::require_some(body.find("await_acknowledgements("));
    let unpublish =
        crate::require_some(body.find("PUBLISHED_GENERATION.store(0, Ordering::Release)"));
    let release = crate::require_some(
        body.find("scheduler::preempt_enable_without_reschedule(publisher_pin)"),
    );

    assert!(acquire < publish);
    assert!(publish < local);
    assert!(local < notify);
    assert!(notify < wait);
    assert!(wait < unpublish);
    assert!(unpublish < release);
    assert!(!body.contains("InterruptMaskGuard"));
    assert!(!body.contains("preempt_enable_and_reschedule"));
    assert!(!body.contains("local_enabled"));
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
