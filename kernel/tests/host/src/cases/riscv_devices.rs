// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use hyper::vm::riscv64::plic::VirtualPlic;

#[test]
fn plic_claim_threshold_and_level_replay() -> Result<(), hyper::vm::riscv64::plic::Error> {
    let mut plic = VirtualPlic::new();
    plic.write(10 * 4, 3)?;
    plic.write(11 * 4, 3)?;
    plic.write(0x2080, (1 << 10) | (1 << 11))?;
    plic.write(0x201000, 3)?;
    plic.set_level(11, true)?;
    plic.set_level(10, true)?;
    assert!(!plic.interrupt_asserted());
    assert_eq!(plic.read(0x201004)?, 10); // Tie: smallest source, despite threshold.
    assert_eq!(plic.read(0x201004)?, 11);
    assert_eq!(plic.read(0x201004)?, 0);
    plic.write(0x201000, 0)?;
    plic.write(0x201004, 10)?;
    assert!(plic.interrupt_asserted());
    assert_eq!(plic.read(0x201004)?, 10);
    plic.set_level(10, false)?;
    plic.write(0x201004, 10)?;
    assert!(!plic.interrupt_asserted());
    plic.set_level(11, false)?;
    plic.write(0x201004, 11)?;
    assert_eq!(plic.read(0x1000)?, 0);
    assert!(plic.set_level(32, true).is_err());
    assert!(plic.read(1).is_err());
    Ok(())
}

#[test]
fn plic_masking_reserved_sources_and_in_service_gateway()
-> Result<(), hyper::vm::riscv64::plic::Error> {
    let mut plic = VirtualPlic::new();
    plic.write(0, u32::MAX)?;
    assert_eq!(plic.read(0)?, 0);
    plic.write(4, 1)?;
    plic.set_level(1, true)?;
    plic.set_level(1, false)?;
    assert_eq!(plic.read(0x1000)?, 2); // Deassertion does not retract a latched request.
    assert_eq!(plic.read(0x201004)?, 0); // Disabled source cannot be claimed.
    plic.write(0x2080, 3)?;
    assert_eq!(plic.read(0x2080)?, 2);
    assert_eq!(plic.read(0x201004)?, 1);
    plic.set_level(1, true)?;
    assert_eq!(plic.read(0x1000)?, 0); // Gateway remains busy until completion.
    plic.write(0x201004, 2)?; // Wrong source must not release source1.
    assert_eq!(plic.read(0x1000)?, 0);
    plic.write(0x201004, 1)?;
    assert_eq!(plic.read(0x1000)?, 2);
    assert_eq!(plic.read(0x200000)?, 0); // No machine delivery context.
    Ok(())
}
