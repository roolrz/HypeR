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
        self.stack_pointer = (stack_top & !0xf) as u64;
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
    pub vbar_el1: u64,
    pub contextidr_el1: u64,
    pub tpidr_el0: u64,
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
            processor_state: 0x3c5,
            sctlr_el1: 0,
            tcr_el1: 0,
            ttbr0_el1: 0,
            ttbr1_el1: 0,
            mair_el1: 0,
            vbar_el1: 0,
            contextidr_el1: 0,
            tpidr_el0: 0,
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
}

unsafe extern "C" {
    fn aarch64_switch_context(previous: *mut ThreadContext, next: *const ThreadContext);
    fn aarch64_thread_trampoline();
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
};
