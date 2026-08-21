//! `AArch64` system-register glue for the reusable `GICv3` driver.

use core::arch::asm;

use hyper::drivers::interrupt::gicv3::CpuInterface;
use hyper::hal::interrupt::InterruptId;

use super::registers;

/// `AArch64` implementation of the `GICv3` system-register CPU interface.
pub struct Aarch64GicCpuInterface;

impl CpuInterface for Aarch64GicCpuInterface {
    unsafe fn initialize() -> bool {
        let mut sre: u64;
        // SAFETY: The boot CPU executes at EL2 with IRQs masked. These system
        // registers configure only its physical GIC CPU interface.
        unsafe {
            asm!(
                "mrs {sre}, ICC_SRE_EL2",
                "orr {sre}, {sre}, #{sre_bit}",
                "msr ICC_SRE_EL2, {sre}",
                "isb",
                "mrs {sre}, ICC_SRE_EL2",
                sre = inout(reg) 0u64 => sre,
                sre_bit = const registers::ICC_SRE_EL2_SRE,
                options(nostack, preserves_flags)
            );
        }
        if sre & registers::ICC_SRE_EL2_SRE == 0 {
            return false;
        }

        let mut control: u64;
        // SAFETY: SRE was confirmed active and the caller exclusively owns the
        // masked local CPU interface during initialization.
        unsafe {
            asm!(
                "msr ICH_HCR_EL2, xzr",
                "mov {control:w}, #{priority}",
                "msr ICC_PMR_EL1, {control}",
                "msr ICC_BPR1_EL1, xzr",
                "mrs {control}, ICC_CTLR_EL1",
                control = out(reg) control,
                priority = const registers::ICC_PMR_ALLOW_ALL,
                options(nostack, preserves_flags)
            );
        }
        control &= !registers::ICC_CTLR_EL1_EOI_MODE;
        // SAFETY: Initialization still owns the masked local interface; this
        // publishes its final priority, EOI, and Group-1 enable state.
        unsafe {
            asm!(
                "msr ICC_CTLR_EL1, {control}",
                "mov {control:w}, #{enable}",
                "msr ICC_IGRPEN1_EL1, {control}",
                "isb",
                control = inout(reg) control => _,
                enable = const registers::ICC_IGRPEN1_ENABLE,
                options(nostack, preserves_flags)
            );
        }
        true
    }

    fn acknowledge() -> u32 {
        let interrupt: u64;
        // SAFETY: Reading IAR1 acknowledges the highest-priority pending Group
        // 1 interrupt for this CPU and does not dereference memory.
        unsafe {
            asm!(
                "mrs {interrupt}, ICC_IAR1_EL1",
                interrupt = out(reg) interrupt,
                options(nostack, preserves_flags)
            );
        }
        (interrupt & registers::ICC_IAR1_INTID_MASK) as u32
    }

    fn end(interrupt: u32) {
        // SAFETY: Writing EOIR is valid at EL2. The controller contract requires
        // the caller to provide an interrupt returned by acknowledge; EOImode is
        // configured so this write also performs deactivation.
        unsafe {
            asm!(
                "msr ICC_EOIR1_EL1, {interrupt}",
                interrupt = in(reg) u64::from(interrupt),
                options(nostack, preserves_flags)
            );
        }
    }

    fn affinity() -> u32 {
        current_gic_affinity()
    }
}

/// Returns the GIC affinity encoding for the current processing element.
pub fn current_gic_affinity() -> u32 {
    let mpidr: u64;
    // SAFETY: MPIDR_EL1 is readable at EL2 and has no side effects.
    unsafe {
        asm!(
            "mrs {mpidr}, MPIDR_EL1",
            mpidr = out(reg) mpidr,
            options(nomem, nostack, preserves_flags)
        );
    }
    let affinity_0_to_2 = mpidr & registers::MPIDR_AFF0_TO_2_MASK;
    let affinity_3 = (mpidr >> registers::MPIDR_AFF3_SHIFT) & registers::MPIDR_AFF3_MASK;
    (affinity_0_to_2 | (affinity_3 << registers::GIC_AFF3_SHIFT)) as u32
}

/// Returns the architecture-reserved emergency stop interrupt.
pub const fn crash_stop_interrupt() -> Option<InterruptId> {
    Some(InterruptId::new(registers::GIC_CRASH_STOP_SGI as u32))
}

/// Tests whether an acknowledged interrupt is the emergency stop IPI.
pub fn is_crash_stop_interrupt(interrupt: InterruptId) -> bool {
    crash_stop_interrupt() == Some(interrupt)
}

/// Sends the emergency stop SGI to every participating PE except self.
pub fn broadcast_crash_stop() -> bool {
    let Some(interrupt) = crash_stop_interrupt() else {
        return false;
    };
    broadcast_sgi(interrupt)
}

fn broadcast_sgi(interrupt: InterruptId) -> bool {
    if interrupt.get() >= 16 {
        return false;
    }
    let value =
        (u64::from(interrupt.get()) << registers::ICC_SGI1R_INTID_SHIFT) | registers::ICC_SGI1R_IRM;
    // SAFETY: The GICv3 system-register interface is active before this API is
    // used. IRM routes the SGI to all participating PEs except the caller.
    unsafe {
        asm!(
            "dsb ishst",
            "msr ICC_SGI1R_EL1, {value}",
            "isb",
            value = in(reg) value,
            options(nostack, preserves_flags)
        );
    }
    true
}

/// Acknowledges the highest-priority physical Group-1 interrupt on this CPU.
pub fn acknowledge_interrupt() -> Option<InterruptId> {
    let raw = <Aarch64GicCpuInterface as CpuInterface>::acknowledge();
    (raw < registers::GIC_SPURIOUS_INTERRUPT_MIN as u32).then_some(InterruptId::new(raw))
}

/// Completes one interrupt returned by [`acknowledge_interrupt`].
pub fn end_interrupt(interrupt: InterruptId) {
    <Aarch64GicCpuInterface as CpuInterface>::end(interrupt.get());
}
