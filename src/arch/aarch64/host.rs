//! Runtime selection and world-switch controls for the EL2 host regime.

use core::arch::asm;

use hyper::sync::atomic::{AtomicU8, Ordering};

use super::registers;

const UNINITIALIZED: u8 = 0;
const NVHE: u8 = 1;
const VHE: u8 = 2;

static HOST_MODE: AtomicU8 = AtomicU8::new(UNINITIALIZED);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HostMode {
    NonVhe,
    Vhe,
}

pub(super) fn initialize() {
    let mode = current_mode();
    let encoded = encode(mode);
    match HOST_MODE.compare_exchange(UNINITIALIZED, encoded, Ordering::AcqRel, Ordering::Acquire) {
        Ok(_) => {}
        Err(selected) if selected == encoded => {}
        Err(selected) => crate::kernel::boot::fail("AArch64 host-mode selection", selected),
    }
}

pub(super) fn current_cpu_is_compatible() -> bool {
    HOST_MODE.load(Ordering::Acquire) == encode(current_mode())
}

pub(super) fn is_vhe() -> bool {
    match HOST_MODE.load(Ordering::Acquire) {
        VHE => true,
        NVHE => false,
        _ => crate::kernel::boot::fail("AArch64 host-mode access", UNINITIALIZED),
    }
}

pub fn mode_name() -> &'static str {
    if is_vhe() { "VHE" } else { "nVHE" }
}

fn current_mode() -> HostMode {
    let hcr: u64;
    // SAFETY: HCR_EL2 is local EL2 state and reading it has no side effects.
    unsafe {
        asm!(
            "mrs {hcr}, HCR_EL2",
            hcr = out(reg) hcr,
            options(nomem, nostack, preserves_flags)
        );
    }
    if hcr & registers::HCR_EL2_E2H != 0 {
        HostMode::Vhe
    } else {
        HostMode::NonVhe
    }
}

const fn encode(mode: HostMode) -> u8 {
    match mode {
        HostMode::NonVhe => NVHE,
        HostMode::Vhe => VHE,
    }
}
