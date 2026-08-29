// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Architecture-neutral Native syscall payload and result invariants.

use hyper::abi::native::{NativeInvocation, NativeResult};

#[test]
fn invocation_owns_the_complete_machine_payload() {
    let invocation = NativeInvocation::new(17, [1, 2, 3, 4, 5, 6], 0x8000);

    assert_eq!(invocation.number(), 17);
    assert_eq!(invocation.arguments(), &[1, 2, 3, 4, 5, 6]);
    assert_eq!(invocation.call_site(), 0x8000);
}

#[test]
fn failed_results_clear_all_auxiliary_words() {
    let result = NativeResult::new(-1, [0xfeed, 0xbeef]);

    assert_eq!(result.status(), -1);
    assert_eq!(result.values(), &[0, 0]);
}

#[test]
fn successful_results_preserve_auxiliary_words() {
    let result = NativeResult::new(0, [0xfeed, 0xbeef]);

    assert_eq!(result.status(), 0);
    assert_eq!(result.values(), &[0xfeed, 0xbeef]);
}
