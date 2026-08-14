use core::arch::asm;

use hyper::drivers::interrupt::gicv3::CpuInterface;

const ICC_SRE_EL2_SRE: u64 = 1 << 0;
const ICC_CTLR_EL1_EOI_MODE: u64 = 1 << 1;
const ICC_IAR1_INTID_MASK: u64 = 0x00ff_ffff;

/// AArch64 implementation of the GICv3 system-register CPU interface.
pub struct Aarch64GicCpuInterface;

impl CpuInterface for Aarch64GicCpuInterface {
    unsafe fn initialize() -> bool {
        let mut sre: u64;
        // SAFETY: The boot CPU executes at EL2 with IRQs masked. These system
        // registers configure only its physical GIC CPU interface.
        unsafe {
            asm!(
                "mrs {sre}, ICC_SRE_EL2",
                "orr {sre}, {sre}, #1",
                "msr ICC_SRE_EL2, {sre}",
                "isb",
                "mrs {sre}, ICC_SRE_EL2",
                sre = inout(reg) 0u64 => sre,
                options(nomem, nostack, preserves_flags)
            );
        }
        if sre & ICC_SRE_EL2_SRE == 0 {
            return false;
        }

        let mut control: u64;
        unsafe {
            asm!(
                "msr ICH_HCR_EL2, xzr",
                "mov {control:w}, #0xff",
                "msr ICC_PMR_EL1, {control}",
                "msr ICC_BPR1_EL1, xzr",
                "mrs {control}, ICC_CTLR_EL1",
                control = out(reg) control,
                options(nomem, nostack, preserves_flags)
            );
        }
        control &= !ICC_CTLR_EL1_EOI_MODE;
        unsafe {
            asm!(
                "msr ICC_CTLR_EL1, {control}",
                "mov {control:w}, #1",
                "msr ICC_IGRPEN1_EL1, {control}",
                "isb",
                control = inout(reg) control => _,
                options(nomem, nostack, preserves_flags)
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
                options(nomem, nostack, preserves_flags)
            );
        }
        (interrupt & ICC_IAR1_INTID_MASK) as u32
    }

    fn end(interrupt: u32) {
        // SAFETY: Writing EOIR is valid at EL2. The controller contract requires
        // the caller to provide an interrupt returned by acknowledge; EOImode is
        // configured so this write also performs deactivation.
        unsafe {
            asm!(
                "msr ICC_EOIR1_EL1, {interrupt}",
                interrupt = in(reg) u64::from(interrupt),
                options(nomem, nostack, preserves_flags)
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
    let affinity_0_to_2 = mpidr & 0x00ff_ffff;
    let affinity_3 = (mpidr >> 32) & 0xff;
    (affinity_0_to_2 | (affinity_3 << 24)) as u32
}
