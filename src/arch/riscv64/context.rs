// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use core::mem::{offset_of, size_of};

use super::registers;

pub type KernelThreadEntry = extern "C" fn(usize);

#[repr(C, align(16))]
pub struct ThreadContext {
    callee_saved: [u64; 12],
    return_address: u64,
    stack_pointer: u64,
    interrupt_enable: u64,
}

impl ThreadContext {
    pub const fn empty() -> Self {
        Self {
            callee_saved: [0; 12],
            return_address: 0,
            stack_pointer: 0,
            interrupt_enable: registers::SSTATUS_SIE,
        }
    }

    pub fn prepare(&mut self, stack_top: usize, entry: KernelThreadEntry, argument: usize) {
        self.callee_saved[0] = entry as usize as u64;
        self.callee_saved[1] = argument as u64;
        self.return_address = riscv64_thread_trampoline as *const () as usize as u64;
        self.stack_pointer = (stack_top & !15) as u64;
    }

    pub fn prepare_vcpu(&mut self, stack_top: usize, entry: KernelThreadEntry, argument: usize) {
        self.prepare(stack_top, entry, argument);
        self.interrupt_enable = 0;
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
    pub hvip: u64,
    pub vsatp: u64,
    pub vstimecmp: u64,
    pub scounteren: u64,
    pub senvcfg: u64,
    virtual_count_offset: u64,
    pub floating: [u64; 32],
    pub fcsr: u32,
    _floating_padding: u32,
    run_state: u64,
}

const GUEST_RUN_READY: u64 = 0;
const GUEST_RUN_RUNNING: u64 = 1;
const GUEST_RUN_IRQ_TAIL: u64 = 2;

#[repr(C)]
struct GuestAnchorExit {
    kind: u64,
    target: usize,
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
            hvip: 0,
            vsatp: 0,
            vstimecmp: u64::MAX,
            scounteren: 0,
            senvcfg: 0,
            virtual_count_offset: 0,
            floating: [0; 32],
            fcsr: 0,
            _floating_padding: 0,
            run_state: GUEST_RUN_READY,
        }
    }

    pub const fn initialize_virtual_interrupts(&mut self) -> Result<(), VirtualInterruptError> {
        Ok(())
    }
    pub fn set_virtual_count(&mut self, physical: u64, value: u64) {
        // HTIMEDELTA is added to TIME while V=1, unlike AArch64 CNTVOFF which
        // is subtracted from the physical counter.
        self.virtual_count_offset = value.wrapping_sub(physical);
    }
    /// Loads this stopped vCPU's hart-local floating-point and timer state.
    ///
    /// # Safety
    ///
    /// The caller must exclusively own this pinned context, keep local
    /// interrupts masked, and ensure no other vCPU is active on this hart.
    pub unsafe fn activate_system_registers(&self) {
        // SAFETY: HTIMEDELTA is writable in HS mode and the input is a plain value.
        unsafe {
            core::arch::asm!(
                "csrw htimedelta, {offset}",
                offset = in(reg) self.virtual_count_offset,
                options(nostack)
            )
        };
    }
    /// Saves hart-local guest state into this vCPU context.
    ///
    /// # Safety
    ///
    /// This context must be the active vCPU on the current hart, exclusively
    /// owned by the caller, with local interrupts masked.
    pub unsafe fn deactivate_system_registers(&mut self) {
        self.capture_virtual_supervisor_registers();
    }
    /// Enters the guest represented by `context`.
    ///
    /// # Safety
    ///
    /// `context` must be non-null, aligned, pinned, and exclusively owned by the
    /// active vCPU for the guest-run lifetime. Trap handling may mutate it.
    pub unsafe fn enter(context: *mut Self) -> ! {
        // Keep only the pinned raw pointer across guest entry and scheduling.
        // The IRQ-tail transition temporarily lends this object to VM
        // deactivation/reactivation, so retaining a Rust reference here would
        // violate exclusive-borrow provenance even though execution is nested.
        loop {
            // SAFETY: The run owner has exclusive access before guest entry;
            // this short reference ends before assembly or policy can borrow it.
            if unsafe { (&mut *context).begin_run() }.is_err() {
                super::halt()
            }
            // SAFETY: RUNNING publishes the exact context to the assembly
            // anchor. A typed anchor exit destroys that publication before it
            // returns here with local interrupts still masked.
            let exit = unsafe { riscv64_enter_guest(context.cast_const().cast()) };
            // SAFETY: Typed assembly return restored exclusive access; this
            // short reference ends before invoking the scheduler callback.
            let target = match unsafe { (&mut *context).consume_irq_tail(exit) } {
                Ok(target) => target,
                Err(_) => super::halt(),
            };
            // SAFETY: Trap dispatch accepts only an opaque callback previously
            // qualified by the selected HAL. The anchor is gone, no raw frame
            // borrow remains, and SIE stays masked across the callback.
            let postlude: unsafe extern "C" fn() = unsafe { core::mem::transmute(target) };
            // SAFETY: The qualified callback contract is established above.
            unsafe { postlude() };
        }
    }

    /// Captures guest state before a typed IRQ-tail anchor unwind.
    ///
    /// # Safety
    ///
    /// `self` must be the exact context published in the current hart's live
    /// guest anchor. Guest floating-point state must already have been copied
    /// by trap entry and local interrupts must remain masked.
    pub(crate) unsafe fn capture_irq_tail(
        &mut self,
        general: &[u64; 32],
        program_counter: u64,
    ) -> Result<(), GuestAnchorError> {
        if self.run_state != GUEST_RUN_RUNNING {
            return Err(GuestAnchorError::State);
        }
        self.general = *general;
        self.program_counter = program_counter;
        self.capture_virtual_supervisor_registers();
        // This state is the final publication consumed by `enter`; all guest
        // register copies happen-before it in same-hart program order.
        self.publish_irq_tail()
    }

    fn begin_run(&mut self) -> Result<(), GuestAnchorError> {
        if self.run_state != GUEST_RUN_READY {
            return Err(GuestAnchorError::State);
        }
        self.run_state = GUEST_RUN_RUNNING;
        Ok(())
    }

    fn publish_irq_tail(&mut self) -> Result<(), GuestAnchorError> {
        if self.run_state != GUEST_RUN_RUNNING {
            return Err(GuestAnchorError::State);
        }
        self.run_state = GUEST_RUN_IRQ_TAIL;
        Ok(())
    }

    fn consume_irq_tail(&mut self, exit: GuestAnchorExit) -> Result<usize, GuestAnchorError> {
        if self.run_state != GUEST_RUN_IRQ_TAIL
            || exit.kind != registers::GUEST_ANCHOR_EXIT_IRQ_TAIL
            || exit.target == 0
        {
            return Err(GuestAnchorError::Exit);
        }
        self.run_state = GUEST_RUN_READY;
        Ok(exit.target)
    }

    fn capture_virtual_supervisor_registers(&mut self) {
        // SAFETY: These virtual-supervisor CSRs are accessible in HS mode while
        // this vCPU owns the current hart. No memory ordering is implied or
        // required by these same-hart register snapshots.
        unsafe {
            core::arch::asm!(
                "csrr {vsstatus}, vsstatus",
                "csrr {vsie}, vsie",
                "csrr {vstvec}, vstvec",
                "csrr {vsscratch}, vsscratch",
                "csrr {vsepc}, vsepc",
                "csrr {vscause}, vscause",
                "csrr {vstval}, vstval",
                vsstatus = out(reg) self.vsstatus,
                vsie = out(reg) self.vsie,
                vstvec = out(reg) self.vstvec,
                vsscratch = out(reg) self.vsscratch,
                vsepc = out(reg) self.vsepc,
                vscause = out(reg) self.vscause,
                vstval = out(reg) self.vstval,
                options(nomem, nostack)
            );
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GuestAnchorError {
    State,
    Exit,
}

pub(super) fn validate_anchor_state_machine() -> bool {
    let mut context = VcpuContext::new(0);
    let mut other = VcpuContext::new(0);
    context.set_virtual_count(0xf000, 0x1000);
    if 0xf000u64.wrapping_add(context.virtual_count_offset) != 0x1000 {
        return false;
    }
    if context.begin_run().is_err()
        || context.publish_irq_tail().is_err()
        || other
            .consume_irq_tail(GuestAnchorExit {
                kind: registers::GUEST_ANCHOR_EXIT_IRQ_TAIL,
                target: 1,
            })
            .is_ok()
        || context.consume_irq_tail(GuestAnchorExit {
            kind: registers::GUEST_ANCHOR_EXIT_IRQ_TAIL,
            target: 1,
        }) != Ok(1)
    {
        return false;
    }
    context
        .consume_irq_tail(GuestAnchorExit {
            kind: registers::GUEST_ANCHOR_EXIT_IRQ_TAIL,
            target: 1,
        })
        .is_err()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VirtualInterruptError {}

unsafe extern "C" {
    fn riscv64_switch_context(
        previous: *mut ThreadContext,
        next: *const ThreadContext,
        previous_interrupt_state: usize,
        completion: extern "C" fn(usize),
        completion_ticket: usize,
    );
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
    fn riscv64_enter_guest(context: *const u8) -> GuestAnchorExit;
}

/// Switches from `previous` to `next` without returning through a normal call.
///
/// # Safety
///
/// Both pointers must be valid pinned scheduler contexts, `previous` must be
/// uniquely writable, and `next` must contain a saved or prepared context. No
/// Rust reference may remain live because `completion` re-enters scheduler
/// ownership. Local interrupts must be masked and the callback must not switch.
pub unsafe fn switch_thread_context(
    previous: *mut ThreadContext,
    next: *const ThreadContext,
    previous_interrupt_state: usize,
    completion: extern "C" fn(usize),
    completion_ticket: usize,
) {
    // SAFETY: The caller establishes ownership and lifetime for both contexts.
    unsafe {
        riscv64_switch_context(
            previous,
            next,
            previous_interrupt_state,
            completion,
            completion_ticket,
        )
    };
}

/// Resets a stack and transfers control to `callback`.
///
/// # Safety
///
/// `[bottom, top)` must be exclusively owned, writable stack memory. `top`
/// must meet the RISC-V ABI alignment requirement and callback must not return.
pub unsafe fn reset_stack_and_enter(
    bottom: usize,
    top: usize,
    watermark: u64,
    canary: u64,
    callback: extern "C" fn(usize) -> !,
    argument: usize,
) -> ! {
    // SAFETY: The caller provides an exclusive valid stack and non-returning callback.
    unsafe { riscv64_reset_stack_and_enter(bottom, top, watermark, canary, callback, argument) }
}

/// Transfers control to a non-returning callback on another stack.
///
/// # Safety
///
/// `top` must be the aligned top of a live, exclusively owned writable stack.
pub unsafe fn run_on_stack(top: usize, callback: extern "C" fn(usize) -> !, argument: usize) -> ! {
    // SAFETY: The caller provides an aligned, live, exclusively owned stack.
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
    assert!(
        offset_of!(ThreadContext, interrupt_enable)
            == registers::THREAD_CONTEXT_SIE_OFFSET as usize
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
    assert!(offset_of!(VcpuContext, hvip) == registers::VCPU_HVIP_OFFSET as usize);
    assert!(offset_of!(VcpuContext, vsatp) == registers::VCPU_VSATP_OFFSET as usize);
    assert!(offset_of!(VcpuContext, vstimecmp) == registers::VCPU_VSTIMECMP_OFFSET as usize);
    assert!(offset_of!(VcpuContext, scounteren) == registers::VCPU_SCOUNTEREN_OFFSET as usize);
    assert!(offset_of!(VcpuContext, senvcfg) == registers::VCPU_SENVCFG_OFFSET as usize);
    assert!(
        offset_of!(VcpuContext, virtual_count_offset)
            == registers::VCPU_VIRTUAL_COUNT_OFFSET as usize
    );
    assert!(offset_of!(VcpuContext, floating) == registers::VCPU_FLOATING_OFFSET as usize);
    assert!(offset_of!(VcpuContext, fcsr) == registers::VCPU_FCSR_OFFSET as usize);
    assert!(offset_of!(VcpuContext, run_state) == registers::VCPU_RUN_STATE_OFFSET as usize);
    assert!(size_of::<GuestAnchorExit>() == 16);
};
