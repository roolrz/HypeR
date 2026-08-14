use core::arch::asm;
use core::ptr::{addr_of, addr_of_mut, read_volatile, write_volatile};

use super::registers;

unsafe extern "C" {
    /// Runtime selector consumed by compiler-builtins outline atomic helpers.
    static mut __aarch64_have_lse_atomics: u8;
}

/// Runtime atomic capabilities selected for the admitted processing elements.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AtomicCapabilities {
    pub lse: bool,
}

/// Selects the fastest atomic backend supported by the boot CPU.
///
/// This must run before Rust code performs an outlined atomic read-modify-write
/// operation. SMP admission rejects a secondary that does not support the
/// globally selected backend before it reaches shared kernel state.
pub fn initialize() -> AtomicCapabilities {
    let features: u64;
    // SAFETY: ID_AA64ISAR0_EL1 is readable at EL2 and has no side effects.
    unsafe {
        asm!(
            "mrs {features}, id_aa64isar0_el1",
            features = out(reg) features,
            options(nomem, nostack, preserves_flags)
        );
    }
    let atomic =
        (features >> registers::ID_AA64ISAR0_ATOMIC_SHIFT) & registers::ID_AA64ISAR0_ATOMIC_MASK;
    let lse = atomic >= registers::ID_AA64ISAR0_ATOMIC_LSE;

    // SAFETY: compiler-builtins defines this hidden one-byte selector. The
    // bootstrap is single-threaded and writes it before outlined atomics run.
    unsafe {
        write_volatile(addr_of_mut!(__aarch64_have_lse_atomics), u8::from(lse));
    }
    AtomicCapabilities { lse }
}

pub fn capabilities() -> AtomicCapabilities {
    // SAFETY: Initialization precedes kernel entry, after which the selector is
    // immutable and may be observed by diagnostics.
    let lse = unsafe { read_volatile(addr_of!(__aarch64_have_lse_atomics)) } != 0;
    AtomicCapabilities { lse }
}

/// Checks that the calling CPU can execute the globally selected backend.
pub fn current_cpu_supports_selected_backend() -> bool {
    if !capabilities().lse {
        return true;
    }
    let features: u64;
    // SAFETY: ID_AA64ISAR0_EL1 is readable at EL2 and has no side effects.
    unsafe {
        asm!(
            "mrs {features}, id_aa64isar0_el1",
            features = out(reg) features,
            options(nomem, nostack, preserves_flags)
        );
    }
    let atomic =
        (features >> registers::ID_AA64ISAR0_ATOMIC_SHIFT) & registers::ID_AA64ISAR0_ATOMIC_MASK;
    atomic >= registers::ID_AA64ISAR0_ATOMIC_LSE
}
