// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Legacy x86 virtual-device composition and interrupt routing.

use hyper::vm::x86::device::legacy_pc::LegacyPcDevices;

#[test]
fn routes_the_pit_only_after_the_master_pic_unmasks_irq_zero() {
    let mut devices = LegacyPcDevices::new();
    assert_eq!(devices.timer_vector(), None);

    for (port, value) in [(0x20, 0x11), (0x21, 0x20), (0x21, 0x04), (0x21, 0x01)] {
        let _ = crate::require_ok(devices.access(port, 1, true, value));
    }
    let _ = crate::require_ok(devices.access(0x21, 1, true, 0xfe));
    assert_eq!(devices.timer_vector(), Some(0x20));
}

#[test]
fn returns_an_absent_device_value_for_unimplemented_ports() {
    let mut devices = LegacyPcDevices::new();
    let access = crate::require_ok(devices.access(0x0cf8, 4, false, 0));
    assert_eq!(access.value, Some(u32::MAX));
}
