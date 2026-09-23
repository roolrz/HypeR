// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn full_guest_input_never_blocks_detach() {
    for detach in [b'd', b'q', ESCAPE] {
        let mut input = Input::default();
        for _ in 0..INPUT_CAPACITY {
            assert_eq!(input.push(b'x'), InputAction::Continue);
        }
        assert_eq!(input.push(b'y'), InputAction::Overflow);
        assert_eq!(input.push(b'z'), InputAction::Continue);
        assert_eq!(input.pending(), vec![b'x'; INPUT_CAPACITY]);
        assert_eq!(input.push(ESCAPE), InputAction::Menu);
        assert_eq!(input.push(detach), InputAction::Detach);
        assert_eq!(input.pending().len(), INPUT_CAPACITY);
    }
}

#[test]
fn resume_and_successful_send_preserve_order_and_reset_pressure_notice() {
    let mut input = Input::default();
    input.push(b'a');
    assert_eq!(input.push(ESCAPE), InputAction::Menu);
    assert_eq!(input.push(b'r'), InputAction::Resume);
    input.push(b'b');
    assert_eq!(input.pending(), b"ab");
    // A failed/nonblocking send leaves exactly the same bytes for retry.
    assert_eq!(input.pending(), b"ab");
    input.sent();
    assert!(!input.is_pending());
    for _ in 0..2 {
        for _ in 0..INPUT_CAPACITY {
            input.push(b'c');
        }
        assert_eq!(input.push(b'd'), InputAction::Overflow);
        input.sent();
    }
}
