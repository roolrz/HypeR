// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Host checks for allocation-free reschedule request publication.

#[path = "../../../../src/kernel/task/reschedule.rs"]
mod request;

use request::PendingReschedule;

#[test]
fn reschedule_requests_coalesce_and_survive_a_completed_take() {
    let pending = PendingReschedule::new();
    assert!(!pending.is_pending());

    pending.publish();
    pending.publish();
    assert!(pending.is_pending());
    assert!(pending.take());
    assert!(!pending.take());

    pending.publish();
    assert!(pending.take());
}
