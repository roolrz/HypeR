use core::mem::{offset_of, size_of};

use super::registers;

pub type KernelThreadEntry = extern "C" fn(usize);

#[repr(C, align(16))]
pub struct ThreadContext {
    callee_saved: [u64; 12],
    return_address: u64,
    stack_pointer: u64,
}

impl ThreadContext {
    pub const fn empty() -> Self {
        Self {
            callee_saved: [0; 12],
            return_address: 0,
            stack_pointer: 0,
        }
    }

    pub fn prepare(&mut self, stack_top: usize, entry: KernelThreadEntry, argument: usize) {
        self.callee_saved[0] = entry as usize as u64;
        self.callee_saved[1] = argument as u64;
        self.return_address = riscv64_thread_trampoline as *const () as usize as u64;
        self.stack_pointer = (stack_top & !15) as u64;
    }
}

#[repr(C, align(16))]
pub struct UserContext {
    pub general: [u64; 32],
    pub program_counter: u64,
    pub status: u64,
}

impl UserContext {
    pub const fn new(program_counter: u64, stack_pointer: u64) -> Self {
        let mut general = [0; 32];
        general[2] = stack_pointer;
        Self {
            general,
            program_counter,
            status: 0,
        }
    }
}

#[repr(C, align(16))]
pub struct VcpuContext {
    pub general: [u64; 32],
    pub program_counter: u64,
    pub vsstatus: u64,
    pub vsie: u64,
    pub vstvec: u64,
    pub vsscratch: u64,
    pub vsepc: u64,
    pub vscause: u64,
    pub vstval: u64,
    pub vsatp: u64,
    pub floating: [u64; 32],
    pub fcsr: u32,
    _floating_padding: u32,
    pub scounteren: u64,
    pub senvcfg: u64,
    virtual_count_offset: u64,
}

impl VcpuContext {
    pub const fn new(program_counter: u64) -> Self {
        // Linux uses the standard LP64D ABI on the supported RV64GC profile.
        // FS=Initial permits the guest to use F/D state; hardware promotes it
        // to Dirty when a floating-point register is modified.
        const VSSTATUS_FS_INITIAL: u64 = 1 << 13;
        Self {
            general: [0; 32],
            program_counter,
            vsstatus: VSSTATUS_FS_INITIAL,
            vsie: 0,
            vstvec: 0,
            vsscratch: 0,
            vsepc: 0,
            vscause: 0,
            vstval: 0,
            vsatp: 0,
            floating: [0; 32],
            fcsr: 0,
            _floating_padding: 0,
            scounteren: 0,
            senvcfg: 0,
            virtual_count_offset: 0,
        }
    }

    pub const fn initialize_virtual_interrupts(&mut self) -> Result<(), VirtualInterruptError> {
        Ok(())
    }
    pub fn set_virtual_count(&mut self, physical: u64, value: u64) {
        self.virtual_count_offset = physical.wrapping_sub(value);
    }
    pub unsafe fn activate_system_registers(&self) {
        unsafe { riscv64_load_guest_floating_point(core::ptr::from_ref(self).cast()) };
        unsafe {
            core::arch::asm!(
                "csrw htimedelta, {offset}",
                offset = in(reg) self.virtual_count_offset,
                options(nomem, nostack)
            )
        };
    }
    pub unsafe fn deactivate_system_registers(&mut self) {
        unsafe { riscv64_save_guest_floating_point(core::ptr::from_mut(self).cast()) };
    }
    pub unsafe fn activate_timer(&self) {}
    pub unsafe fn deactivate_timer(&mut self) {}
    pub fn enter(&mut self) -> ! {
        // SAFETY: The vCPU run loop owns this pinned context, has activated
        // HGATP, and keeps the current HS stack reserved until the next trap.
        unsafe { riscv64_enter_guest(core::ptr::from_ref(self).cast()) }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VirtualInterruptError {}

unsafe extern "C" {
    fn riscv64_switch_context(previous: *mut ThreadContext, next: *const ThreadContext);
    fn riscv64_thread_trampoline();
    fn riscv64_reset_stack_and_enter(
        bottom: usize,
        top: usize,
        watermark: u64,
        canary: u64,
        callback: extern "C" fn(usize) -> !,
        argument: usize,
    ) -> !;
    fn riscv64_run_on_stack(top: usize, callback: extern "C" fn(usize) -> !, argument: usize) -> !;
    fn riscv64_enter_guest(context: *const u8) -> !;
    fn riscv64_load_guest_floating_point(context: *const u8);
    fn riscv64_save_guest_floating_point(context: *mut u8);
}

pub unsafe fn switch_thread_context(previous: &mut ThreadContext, next: &ThreadContext) {
    unsafe { riscv64_switch_context(previous, next) };
}

pub unsafe fn reset_stack_and_enter(
    bottom: usize,
    top: usize,
    watermark: u64,
    canary: u64,
    callback: extern "C" fn(usize) -> !,
    argument: usize,
) -> ! {
    unsafe { riscv64_reset_stack_and_enter(bottom, top, watermark, canary, callback, argument) }
}

pub unsafe fn run_on_stack(top: usize, callback: extern "C" fn(usize) -> !, argument: usize) -> ! {
    unsafe { riscv64_run_on_stack(top, callback, argument) }
}

const _: () = {
    assert!(
        offset_of!(ThreadContext, callee_saved) == registers::THREAD_CONTEXT_S0_OFFSET as usize
    );
    assert!(
        offset_of!(ThreadContext, return_address) == registers::THREAD_CONTEXT_RA_OFFSET as usize
    );
    assert!(
        offset_of!(ThreadContext, stack_pointer) == registers::THREAD_CONTEXT_SP_OFFSET as usize
    );
    assert!(size_of::<ThreadContext>() == registers::THREAD_CONTEXT_SIZE as usize);
    assert!(offset_of!(VcpuContext, general) == registers::VCPU_GENERAL_OFFSET as usize);
    assert!(offset_of!(VcpuContext, program_counter) == registers::VCPU_PC_OFFSET as usize);
    assert!(offset_of!(VcpuContext, vsstatus) == registers::VCPU_VSSTATUS_OFFSET as usize);
    assert!(offset_of!(VcpuContext, vsie) == registers::VCPU_VSIE_OFFSET as usize);
    assert!(offset_of!(VcpuContext, vstvec) == registers::VCPU_VSTVEC_OFFSET as usize);
    assert!(offset_of!(VcpuContext, vsscratch) == registers::VCPU_VSSCRATCH_OFFSET as usize);
    assert!(offset_of!(VcpuContext, vsepc) == registers::VCPU_VSEPC_OFFSET as usize);
    assert!(offset_of!(VcpuContext, vscause) == registers::VCPU_VSCAUSE_OFFSET as usize);
    assert!(offset_of!(VcpuContext, vstval) == registers::VCPU_VSTVAL_OFFSET as usize);
    assert!(offset_of!(VcpuContext, vsatp) == registers::VCPU_VSATP_OFFSET as usize);
    assert!(offset_of!(VcpuContext, floating) == registers::VCPU_FLOATING_OFFSET as usize);
    assert!(offset_of!(VcpuContext, fcsr) == registers::VCPU_FCSR_OFFSET as usize);
};
