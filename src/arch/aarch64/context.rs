use core::arch::asm;
use core::mem::{offset_of, size_of};

use super::registers;

pub type KernelThreadEntry = extern "C" fn(usize);

#[repr(C, align(16))]
pub struct ThreadContext {
    callee_saved: [u64; 10],
    frame_pointer: u64,
    link_register: u64,
    stack_pointer: u64,
    simd_callee_saved: [u64; 8],
    fpcr: u64,
    fpsr: u64,
}

impl ThreadContext {
    pub const fn empty() -> Self {
        Self {
            callee_saved: [0; 10],
            frame_pointer: 0,
            link_register: 0,
            stack_pointer: 0,
            simd_callee_saved: [0; 8],
            fpcr: 0,
            fpsr: 0,
        }
    }

    pub fn prepare(&mut self, stack_top: usize, entry: KernelThreadEntry, argument: usize) {
        self.callee_saved[0] = entry as usize as u64;
        self.callee_saved[1] = argument as u64;
        self.link_register = aarch64_thread_trampoline as *const () as usize as u64;
        self.stack_pointer = (stack_top & !(registers::STACK_ALIGNMENT_MASK as usize)) as u64;
    }
}

#[repr(C, align(16))]
pub struct UserContext {
    pub general: [u64; 31],
    pub stack_pointer: u64,
    pub program_counter: u64,
    pub processor_state: u64,
    pub thread_pointer: u64,
}

impl UserContext {
    pub const fn new(program_counter: u64, stack_pointer: u64) -> Self {
        Self {
            general: [0; 31],
            stack_pointer,
            program_counter,
            processor_state: 0,
            thread_pointer: 0,
        }
    }
}

#[repr(C, align(16))]
pub struct VcpuContext {
    pub general: [u64; 31],
    pub stack_pointer_el0: u64,
    pub stack_pointer_el1: u64,
    pub program_counter: u64,
    pub processor_state: u64,
    pub sctlr_el1: u64,
    pub tcr_el1: u64,
    pub ttbr0_el1: u64,
    pub ttbr1_el1: u64,
    pub mair_el1: u64,
    pub amair_el1: u64,
    pub vbar_el1: u64,
    pub cpacr_el1: u64,
    pub cntkctl_el1: u64,
    pub afsr0_el1: u64,
    pub afsr1_el1: u64,
    pub esr_el1: u64,
    pub far_el1: u64,
    pub par_el1: u64,
    pub elr_el1: u64,
    pub spsr_el1: u64,
    pub contextidr_el1: u64,
    pub tpidr_el0: u64,
    pub tpidrro_el0: u64,
    pub tpidr_el1: u64,
    timer: super::timer::VirtualTimerContext,
    pub vgic: super::VgicCpuContext,
}

impl VcpuContext {
    pub const fn new(program_counter: u64) -> Self {
        Self {
            general: [0; 31],
            stack_pointer_el0: 0,
            stack_pointer_el1: 0,
            program_counter,
            processor_state: registers::SPSR_EL1H_AND_DAIF,
            sctlr_el1: registers::SCTLR_EL1_GUEST_RESET_VALUE,
            tcr_el1: 0,
            ttbr0_el1: 0,
            ttbr1_el1: 0,
            mair_el1: 0,
            amair_el1: 0,
            vbar_el1: 0,
            cpacr_el1: 0,
            cntkctl_el1: 0,
            afsr0_el1: 0,
            afsr1_el1: 0,
            esr_el1: 0,
            far_el1: 0,
            par_el1: 0,
            elr_el1: 0,
            spsr_el1: 0,
            contextidr_el1: 0,
            tpidr_el0: 0,
            tpidrro_el0: 0,
            tpidr_el1: 0,
            timer: super::timer::VirtualTimerContext::empty(),
            vgic: super::VgicCpuContext::empty(),
        }
    }

    /// Prepares the hardware-assisted virtual interrupt interface state.
    pub fn initialize_vgic(&mut self) -> Result<super::VgicCapabilities, super::VgicError> {
        super::vgic::initialize_context(&mut self.vgic)
    }

    /// Sets the guest-visible virtual count to `value` at the supplied
    /// physical counter instant.
    pub fn set_virtual_count(&mut self, physical_count: u64, value: u64) {
        self.timer.set_offset(physical_count.wrapping_sub(value));
    }

    pub fn virtual_timer_deadline(&self) -> u64 {
        self.timer.compare_value()
    }

    pub fn set_virtual_timer_deadline(&mut self, deadline: u64) {
        self.timer.set_compare_value(deadline);
    }

    pub fn set_virtual_timer_enabled(&mut self, enabled: bool) {
        self.timer.set_enabled(enabled);
    }

    pub fn set_virtual_timer_masked(&mut self, masked: bool) {
        self.timer.set_masked(masked);
    }

    pub fn virtual_timer_interrupt_asserted_at(&self, physical_count: u64) -> bool {
        self.timer.interrupt_asserted_at(physical_count)
    }

    pub fn virtual_timer_interrupt_asserted_hardware(&self) -> bool {
        super::timer::virtual_timer_interrupt_asserted()
    }

    /// Loads the guest-owned EL1 system-register bank on the current CPU.
    ///
    /// # Safety
    ///
    /// The caller must exclusively own this stopped vCPU, and lower-EL guest
    /// execution must remain disabled until the rest of its state is loaded.
    pub unsafe fn activate_system_registers(&self) {
        // SAFETY: The caller owns the inactive guest context. The final ISB
        // makes all translation and exception state visible before guest entry.
        unsafe {
            asm!(
                "msr TCR_EL1, {tcr}",
                "msr TTBR0_EL1, {ttbr0}",
                "msr TTBR1_EL1, {ttbr1}",
                "msr MAIR_EL1, {mair}",
                "msr AMAIR_EL1, {amair}",
                "msr SCTLR_EL1, {sctlr}",
                "msr VBAR_EL1, {vbar}",
                "msr CPACR_EL1, {cpacr}",
                "msr CNTKCTL_EL1, {cntkctl}",
                "msr AFSR0_EL1, {afsr0}",
                "msr AFSR1_EL1, {afsr1}",
                "msr CONTEXTIDR_EL1, {contextidr}",
                "msr TPIDR_EL0, {tpidr_el0}",
                "msr TPIDRRO_EL0, {tpidrro_el0}",
                "msr TPIDR_EL1, {tpidr_el1}",
                "msr ESR_EL1, {esr}",
                "msr FAR_EL1, {far}",
                "msr PAR_EL1, {par}",
                "msr ELR_EL1, {elr}",
                "msr SPSR_EL1, {spsr}",
                "msr SP_EL0, {sp_el0}",
                "msr SP_EL1, {sp_el1}",
                "isb",
                sctlr = in(reg) self.sctlr_el1,
                tcr = in(reg) self.tcr_el1,
                ttbr0 = in(reg) self.ttbr0_el1,
                ttbr1 = in(reg) self.ttbr1_el1,
                mair = in(reg) self.mair_el1,
                amair = in(reg) self.amair_el1,
                vbar = in(reg) self.vbar_el1,
                cpacr = in(reg) self.cpacr_el1,
                cntkctl = in(reg) self.cntkctl_el1,
                afsr0 = in(reg) self.afsr0_el1,
                afsr1 = in(reg) self.afsr1_el1,
                contextidr = in(reg) self.contextidr_el1,
                tpidr_el0 = in(reg) self.tpidr_el0,
                tpidrro_el0 = in(reg) self.tpidrro_el0,
                tpidr_el1 = in(reg) self.tpidr_el1,
                esr = in(reg) self.esr_el1,
                far = in(reg) self.far_el1,
                par = in(reg) self.par_el1,
                elr = in(reg) self.elr_el1,
                spsr = in(reg) self.spsr_el1,
                sp_el0 = in(reg) self.stack_pointer_el0,
                sp_el1 = in(reg) self.stack_pointer_el1,
                options(nostack, preserves_flags)
            );
        }
    }

    /// Saves the live guest-owned EL1 system-register bank.
    ///
    /// # Safety
    ///
    /// This context must be the vCPU currently loaded on the calling CPU, and
    /// guest execution must already have stopped with local IRQs masked.
    pub unsafe fn deactivate_system_registers(&mut self) {
        // SAFETY: The caller guarantees that the live EL1 bank belongs to this
        // context and cannot change concurrently.
        unsafe {
            asm!(
                "mrs {sctlr}, SCTLR_EL1",
                "mrs {tcr}, TCR_EL1",
                "mrs {ttbr0}, TTBR0_EL1",
                "mrs {ttbr1}, TTBR1_EL1",
                "mrs {mair}, MAIR_EL1",
                "mrs {amair}, AMAIR_EL1",
                "mrs {vbar}, VBAR_EL1",
                "mrs {cpacr}, CPACR_EL1",
                "mrs {cntkctl}, CNTKCTL_EL1",
                "mrs {afsr0}, AFSR0_EL1",
                "mrs {afsr1}, AFSR1_EL1",
                "mrs {contextidr}, CONTEXTIDR_EL1",
                "mrs {tpidr_el0}, TPIDR_EL0",
                "mrs {tpidrro_el0}, TPIDRRO_EL0",
                "mrs {tpidr_el1}, TPIDR_EL1",
                "mrs {esr}, ESR_EL1",
                "mrs {far}, FAR_EL1",
                "mrs {par}, PAR_EL1",
                "mrs {elr}, ELR_EL1",
                "mrs {spsr}, SPSR_EL1",
                "mrs {sp_el0}, SP_EL0",
                "mrs {sp_el1}, SP_EL1",
                sctlr = out(reg) self.sctlr_el1,
                tcr = out(reg) self.tcr_el1,
                ttbr0 = out(reg) self.ttbr0_el1,
                ttbr1 = out(reg) self.ttbr1_el1,
                mair = out(reg) self.mair_el1,
                amair = out(reg) self.amair_el1,
                vbar = out(reg) self.vbar_el1,
                cpacr = out(reg) self.cpacr_el1,
                cntkctl = out(reg) self.cntkctl_el1,
                afsr0 = out(reg) self.afsr0_el1,
                afsr1 = out(reg) self.afsr1_el1,
                contextidr = out(reg) self.contextidr_el1,
                tpidr_el0 = out(reg) self.tpidr_el0,
                tpidrro_el0 = out(reg) self.tpidrro_el0,
                tpidr_el1 = out(reg) self.tpidr_el1,
                esr = out(reg) self.esr_el1,
                far = out(reg) self.far_el1,
                par = out(reg) self.par_el1,
                elr = out(reg) self.elr_el1,
                spsr = out(reg) self.spsr_el1,
                sp_el0 = out(reg) self.stack_pointer_el0,
                sp_el1 = out(reg) self.stack_pointer_el1,
                options(nomem, nostack, preserves_flags)
            );
        }
    }

    /// Loads this vCPU's GIC virtualization state on the current CPU.
    ///
    /// # Safety
    ///
    /// No other CPU may run this vCPU, and guest execution must not already be
    /// active on the calling CPU.
    pub unsafe fn activate_vgic(&self) -> Result<(), super::VgicError> {
        unsafe { super::vgic::activate(&self.vgic) }
    }

    /// Saves this vCPU's GIC virtualization state and disables guest delivery.
    ///
    /// # Safety
    ///
    /// This context must be the vCPU currently loaded on the calling CPU.
    pub unsafe fn deactivate_vgic(&mut self) -> Result<(), super::VgicError> {
        unsafe { super::vgic::deactivate(&mut self.vgic) }
    }

    /// Loads this vCPU's architectural virtual timer state locally.
    ///
    /// # Safety
    ///
    /// No other CPU may run this vCPU, and guest execution must not already be
    /// active on the calling CPU.
    pub unsafe fn activate_timer(&self) {
        unsafe { super::timer::activate_virtual_timer(&self.timer) };
    }

    /// Saves and disables this vCPU's architectural virtual timer locally.
    ///
    /// # Safety
    ///
    /// Local IRQs must be masked and this must be the active local vCPU.
    pub unsafe fn deactivate_timer(&mut self) {
        unsafe { super::timer::deactivate_virtual_timer(&mut self.timer) };
    }

    /// Restores guest GPRs and returns to the configured lower-EL PC.
    ///
    /// # Safety
    ///
    /// Stage-2 translation and every guest-owned architectural context must be
    /// active on this CPU. This function does not provide a return path.
    pub unsafe fn enter(&self) -> ! {
        unsafe { aarch64_enter_guest(self as *const VcpuContext as *const u8) }
    }
}

unsafe extern "C" {
    fn aarch64_switch_context(previous: *mut ThreadContext, next: *const ThreadContext);
    fn aarch64_thread_trampoline();
    fn aarch64_enter_guest(context: *const u8) -> !;
    fn aarch64_reset_stack_and_enter(
        bottom: usize,
        top: usize,
        watermark: u64,
        canary: u64,
        callback: extern "C" fn(usize) -> !,
        argument: usize,
    ) -> !;
    fn aarch64_run_on_emergency_stack(callback: extern "C" fn(usize) -> !, argument: usize) -> !;
}

/// Switches AAPCS64 callee-saved state and kernel stacks.
///
/// # Safety
///
/// Both contexts must remain pinned until this call eventually returns on the
/// previous context. `next` must own a valid mapped kernel stack.
pub unsafe fn switch_thread_context(previous: &mut ThreadContext, next: &ThreadContext) {
    unsafe { aarch64_switch_context(previous, next) };
}

/// Abandons the current call chain and enters a continuation on a clean stack.
///
/// # Safety
///
/// `bottom..top` must be the exclusively owned, writable stack currently in
/// use. Interrupts must be masked, and `callback` must never return.
pub unsafe fn reset_stack_and_enter(
    bottom: usize,
    top: usize,
    watermark: u64,
    canary: u64,
    callback: extern "C" fn(usize) -> !,
    argument: usize,
) -> ! {
    unsafe { aarch64_reset_stack_and_enter(bottom, top, watermark, canary, callback, argument) }
}

/// Permanently invokes fatal handling on the calling CPU's emergency stack.
///
/// # Safety
///
/// `argument` must remain valid forever or until `callback` stops the CPU.
pub unsafe fn run_on_emergency_stack(callback: extern "C" fn(usize) -> !, argument: usize) -> ! {
    unsafe { aarch64_run_on_emergency_stack(callback, argument) }
}

const _: () = {
    assert!(
        offset_of!(ThreadContext, callee_saved) == registers::THREAD_CONTEXT_X19_OFFSET as usize
    );
    assert!(
        offset_of!(ThreadContext, frame_pointer) == registers::THREAD_CONTEXT_X29_OFFSET as usize
    );
    assert!(
        offset_of!(ThreadContext, link_register) == registers::THREAD_CONTEXT_X30_OFFSET as usize
    );
    assert!(
        offset_of!(ThreadContext, stack_pointer) == registers::THREAD_CONTEXT_SP_OFFSET as usize
    );
    assert!(
        offset_of!(ThreadContext, simd_callee_saved)
            == registers::THREAD_CONTEXT_D8_OFFSET as usize
    );
    assert!(offset_of!(ThreadContext, fpcr) == registers::THREAD_CONTEXT_FPCR_OFFSET as usize);
    assert!(offset_of!(ThreadContext, fpsr) == registers::THREAD_CONTEXT_FPSR_OFFSET as usize);
    assert!(size_of::<ThreadContext>() == 192);
    assert!(offset_of!(VcpuContext, general) == registers::VCPU_CONTEXT_X0_OFFSET as usize);
    assert!(
        offset_of!(VcpuContext, general) + 30 * size_of::<u64>()
            == registers::VCPU_CONTEXT_X30_OFFSET as usize
    );
    assert!(offset_of!(VcpuContext, program_counter) == registers::VCPU_CONTEXT_PC_OFFSET as usize);
    assert!(
        offset_of!(VcpuContext, processor_state) == registers::VCPU_CONTEXT_PSTATE_OFFSET as usize
    );
};
