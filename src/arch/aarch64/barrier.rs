use core::arch::asm;

use hyper::hal::barrier::{Barrier, BarrierAccess, BarrierDomain};

/// `AArch64` implementation of the architecture-neutral barrier contract.
pub struct Aarch64Barrier;

macro_rules! execute_barrier {
    ($instruction:literal, $domain:expr, $access:expr) => {
        match ($domain, $access) {
            (BarrierDomain::NonShareable, BarrierAccess::Reads) => unsafe {
                asm!(
                    concat!($instruction, " nshld"),
                    options(nostack, preserves_flags)
                )
            },
            (BarrierDomain::NonShareable, BarrierAccess::Writes) => unsafe {
                asm!(
                    concat!($instruction, " nshst"),
                    options(nostack, preserves_flags)
                )
            },
            (BarrierDomain::NonShareable, BarrierAccess::All) => unsafe {
                asm!(
                    concat!($instruction, " nsh"),
                    options(nostack, preserves_flags)
                )
            },
            (BarrierDomain::InnerShareable, BarrierAccess::Reads) => unsafe {
                asm!(
                    concat!($instruction, " ishld"),
                    options(nostack, preserves_flags)
                )
            },
            (BarrierDomain::InnerShareable, BarrierAccess::Writes) => unsafe {
                asm!(
                    concat!($instruction, " ishst"),
                    options(nostack, preserves_flags)
                )
            },
            (BarrierDomain::InnerShareable, BarrierAccess::All) => unsafe {
                asm!(
                    concat!($instruction, " ish"),
                    options(nostack, preserves_flags)
                )
            },
            (BarrierDomain::OuterShareable, BarrierAccess::Reads) => unsafe {
                asm!(
                    concat!($instruction, " oshld"),
                    options(nostack, preserves_flags)
                )
            },
            (BarrierDomain::OuterShareable, BarrierAccess::Writes) => unsafe {
                asm!(
                    concat!($instruction, " oshst"),
                    options(nostack, preserves_flags)
                )
            },
            (BarrierDomain::OuterShareable, BarrierAccess::All) => unsafe {
                asm!(
                    concat!($instruction, " osh"),
                    options(nostack, preserves_flags)
                )
            },
            (BarrierDomain::FullSystem, BarrierAccess::Reads) => unsafe {
                asm!(
                    concat!($instruction, " ld"),
                    options(nostack, preserves_flags)
                )
            },
            (BarrierDomain::FullSystem, BarrierAccess::Writes) => unsafe {
                asm!(
                    concat!($instruction, " st"),
                    options(nostack, preserves_flags)
                )
            },
            (BarrierDomain::FullSystem, BarrierAccess::All) => unsafe {
                asm!(
                    concat!($instruction, " sy"),
                    options(nostack, preserves_flags)
                )
            },
        }
    };
}

impl Barrier for Aarch64Barrier {
    #[inline]
    fn data_memory(domain: BarrierDomain, access: BarrierAccess) {
        execute_barrier!("dmb", domain, access);
    }

    #[inline]
    fn data_synchronization(domain: BarrierDomain, access: BarrierAccess) {
        execute_barrier!("dsb", domain, access);
    }

    #[inline]
    fn instruction_synchronization() {
        // SAFETY: ISB only synchronizes the current processing element.
        unsafe { asm!("isb", options(nostack, preserves_flags)) };
    }
}
