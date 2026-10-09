// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::ns16550::Ns16550;

#[test]
fn uart_fifo_trigger_and_sparse_character_timeout() {
    let mut uart = Ns16550::with_clock(3_686_400, 3_686_400);
    uart.write_at(3, 3, 0); // Eight data bits, one stop bit.
    uart.write_at(2, 0x41, 0); // Four-byte trigger.
    uart.write_at(1, 1, 0);
    assert!(uart.receive(b'a', 0));
    assert!(!uart.interrupt_asserted());
    assert_eq!(uart.next_timeout(), Some(640));
    uart.advance(639);
    assert!(!uart.interrupt_asserted());
    uart.advance(640);
    assert_eq!(uart.read_at(2, 640), 0xcc);
    assert_eq!(uart.next_timeout(), None);
    assert_eq!(uart.read_at(0, 640), b'a');
    assert!(!uart.interrupt_asserted());
    for byte in b"abcd" {
        assert!(uart.receive(*byte, 700));
    }
    assert_eq!(uart.read_at(2, 700), 0xc4);
    assert_eq!(uart.read_at(0, 700), b'a');
    assert!(!uart.interrupt_asserted());
    assert_eq!(uart.next_timeout(), Some(1340));
}

#[test]
fn uart_reset_overrun_dlab_and_thre_latch() {
    let mut uart = Ns16550::new();
    uart.write(1, 2);
    assert_eq!(uart.read(2), 2);
    assert_eq!(uart.read(2), 1);
    assert_eq!(uart.write(0, b'x'), Some(b'x'));
    assert_eq!(uart.read(2), 2);
    uart.write(3, 0x80);
    uart.write(0, 12);
    uart.write(1, 3);
    assert_eq!(uart.read(0), 12);
    assert_eq!(uart.read(1), 3);
    uart.write(3, 3);
    uart.write(1, 5);
    assert!(uart.receive(1, 0));
    assert!(!uart.receive(2, 0));
    assert_eq!(uart.read(2), 6);
    assert_eq!(uart.read(5) & 3, 3);
    assert_eq!(uart.read(2), 4);
    uart.write(2, 1);
    assert_eq!(uart.read(5) & 3, 0);
    for value in 0..16 {
        assert!(uart.receive(value, 0));
    }
    assert!(!uart.can_receive());
    uart.write(2, 3);
    assert!(uart.can_receive());
    assert_eq!(uart.next_timeout(), None);
}

#[test]
fn uart_loopback_does_not_transmit_to_host() {
    let mut uart = Ns16550::new();
    uart.write(4, 0x1f);
    assert_eq!(uart.read(6) & 0xf0, 0xf0);
    assert_eq!(uart.write(0, 0x5a), None);
    assert_eq!(uart.read(0), 0x5a);
}

#[test]
fn uart_timeout_restart_and_counter_wrap() {
    let mut uart = Ns16550::with_clock(3_686_400, 3_686_400);
    uart.write_at(3, 3, 0);
    uart.write_at(2, 0xc1, 0);
    uart.write_at(1, 1, 0);
    let start = u64::MAX - 200;
    assert!(uart.receive(1, start));
    assert_eq!(uart.next_timeout(), Some(439));
    uart.advance(438);
    assert!(!uart.interrupt_asserted());
    // A stale timer callback may race with a newly received character.
    assert!(uart.receive(2, 400));
    uart.advance(439);
    assert!(!uart.interrupt_asserted());
    assert_eq!(uart.next_timeout(), Some(1040));
    uart.advance(1040);
    assert!(uart.interrupt_asserted());
    uart.write_at(2, 0, 1040);
    assert_eq!(uart.next_timeout(), None);
    assert_eq!(uart.read_at(5, 1040) & 1, 0);
}

#[test]
fn uart_loopback_excludes_external_input_without_losing_internal_data() {
    let mut uart = Ns16550::new();
    uart.write(4, 16);
    assert!(!uart.can_receive_external());
    assert!(!uart.receive_external(b'X', 0));
    assert_eq!(uart.write(0, b'L'), None);
    assert!(!uart.receive_external(b'Y', 0));
    assert_eq!(uart.read(5) & 3, 1); // No external overrun or data insertion.
    assert_eq!(uart.read(0), b'L');
    uart.write(4, 0);
    assert!(uart.can_receive_external());
    assert!(uart.receive_external(b'Z', 0));
    assert_eq!(uart.read(0), b'Z');
}
