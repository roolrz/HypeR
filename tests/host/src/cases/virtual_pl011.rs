//! Virtual PL011 register, FIFO, output, and interrupt semantics.

use hyper::hw::pl011 as reg;
use hyper::vm::aarch64::device::pl011::VirtualPl011;

#[test]
fn exposes_primecell_identity_and_transmits_bytes() {
    let mut uart = VirtualPl011::new();
    let identity = crate::require_ok(uart.read(reg::PERIPH_ID0 as u64, 4));
    assert_eq!(identity.value, Some(reg::PERIPH_ID0_VALUE as u64));

    let output = crate::require_ok(uart.write(reg::DR as u64, 4, u64::from(b'X')));
    assert_eq!(output.transmitted, Some(b'X'));
}

#[test]
fn models_receive_fifo_and_level_interrupts() {
    let mut uart = VirtualPl011::new();
    let mask = reg::INT_RX | reg::INT_RT | reg::INT_ERROR_MASK;
    let _ = crate::require_ok(uart.write(reg::IMSC as u64, 2, u64::from(mask)));
    assert!(uart.receive(b'A'));

    let status = crate::require_ok(uart.read(reg::MIS as u64, 4));
    assert_ne!(
        crate::require_some(status.value) & u64::from(reg::INT_RT),
        0
    );
    let data = crate::require_ok(uart.read(reg::DR as u64, 4));
    assert_eq!(data.value, Some(u64::from(b'A')));
    assert!(!data.interrupt_asserted);
}

#[test]
fn reports_receive_overrun() {
    let mut uart = VirtualPl011::new();
    let _ = crate::require_ok(uart.write(reg::IMSC as u64, 4, u64::from(reg::INT_OE)));
    for value in 0..33 {
        let _ = uart.receive(value);
    }
    let status = crate::require_ok(uart.read(reg::RSR_ECR as u64, 4));
    assert_ne!(
        crate::require_some(status.value) & u64::from(reg::RSR_OE),
        0
    );
    assert!(status.interrupt_asserted);
}
