// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native U-mode register ownership and contained trap completion.
//! The dedicated vector restores the host vector, zero sscratch, and host
//! gp/tp before Rust. Raw frames never cross the Native policy service.

use super::registers;
use core::arch::asm;
use core::mem::{offset_of, size_of};
use core::ptr::NonNull;
use hyper::abi::native::{NativeInvocation, NativeResult};
use hyper::hal::interrupt::{EntryAction, InterruptId, InterruptOrigin};
use hyper::hal::user::{
    NativeCallAction, NativeCallService, UserFault, UserFaultKind, UserRunBinding,
};
#[cfg(feature = "kernel-self-test")]
use hyper::sync::atomic::AtomicUsize;
use hyper::sync::atomic::{AtomicPtr, AtomicU64, Ordering};

const READY: u64 = 0;
const RUNNING: u64 = 1;
const STOPPED: u64 = 2;
const TERMINATED: u64 = 3;
const EXIT_NONE: u64 = 0;
const EXIT_NATIVE: u64 = 1;
const EXIT_FAULT: u64 = 2;
const EXIT_INTERRUPTED: u64 = 3;
const NATIVE_STATE: u64 = (2 << 32) | (3 << 13) | registers::SSTATUS_SPIE;

#[repr(C, align(16))]
#[derive(Clone, Copy)]
struct NativeFrame {
    general: [u64; 32],
    pc: u64,
    status: u64,
    cause: u64,
    fault_address: u64,
    floating: [u64; 32],
    fcsr: u64,
    _padding: u64,
}
#[repr(C, align(16))]
struct MachineContext {
    frame: NativeFrame,
    thread: u64,
    image_generation: u64,
    run_generation: u64,
    state: u64,
    exit_kind: u64,
}
pub(crate) struct UserContext {
    machine: MachineContext,
}
impl UserContext {
    pub(crate) fn set_entry_argument(&mut self, argument: u64) {
        self.machine.frame.general[10] = argument;
    }
    pub(crate) fn try_new(
        entry: u64,
        stack: u64,
        tls: u64,
        address_limit: u64,
    ) -> Result<Self, Error> {
        if entry == 0
            || entry >= address_limit
            || entry & 1 != 0
            || stack == 0
            || stack >= address_limit
            || stack & 15 != 0
            || tls >= address_limit
        {
            return Err(Error::InvalidInitialContext);
        }
        let mut general = [0; 32];
        general[2] = stack;
        general[4] = tls;
        Ok(Self {
            machine: MachineContext {
                frame: NativeFrame {
                    general,
                    pc: entry,
                    status: NATIVE_STATE,
                    cause: 0,
                    fault_address: 0,
                    floating: [0; 32],
                    fcsr: 0,
                    _padding: 0,
                },
                thread: 0,
                image_generation: 0,
                run_generation: 0,
                state: READY,
                exit_kind: EXIT_NONE,
            },
        })
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    AlreadyRunning,
    CompletionBindingMismatch,
    InvalidInitialContext,
    InvalidMachineState,
    InvalidProcessor,
    LowerWorldUnavailable,
    RunGenerationNotIncreasing,
    Unsupported(super::user_machine::ContractError),
}
impl From<super::user_machine::ContractError> for Error {
    fn from(value: super::user_machine::ContractError) -> Self {
        Self::Unsupported(value)
    }
}
struct RunPublication {
    generation: AtomicU64,
    context: AtomicPtr<MachineContext>,
    service: AtomicPtr<NativeCallService<'static>>,
}
impl RunPublication {
    const fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            context: AtomicPtr::new(core::ptr::null_mut()),
            service: AtomicPtr::new(core::ptr::null_mut()),
        }
    }
}
static ACTIVE: [RunPublication; hyper::config::MAX_CPUS as usize] =
    [const { RunPublication::new() }; hyper::config::MAX_CPUS as usize];
#[cfg(feature = "kernel-self-test")]
static DIRECT_NATIVE_CALLS: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "kernel-self-test")]
pub(crate) fn direct_native_call_count_for_test() -> usize {
    DIRECT_NATIVE_CALLS.load(Ordering::Relaxed)
}
unsafe extern "C" {
    fn riscv64_run_native_user(context: *mut MachineContext);
    fn riscv64_unwind_native_user();
    #[link_name = "riscv64_call_native_irq_on_stack"]
    fn call_native_irq_on_stack(
        frame: *mut NativeFrame,
        stack: usize,
        callback: extern "C" fn(&mut NativeFrame) -> u64,
    ) -> u64;
}

/// Runs an admitted Native generation until a deferred call, fault, or IRQ unwind.
///
/// # Safety
/// Caller retains the pinned context, active translation, CPU pin, and masked
/// interrupt state. No scheduling occurs before this function returns.
pub(crate) unsafe fn run_user<'context>(
    context: &'context mut UserContext,
    binding: UserRunBinding,
    service: &NativeCallService<'_>,
) -> Result<UserExit<'context>, Error> {
    let scratch: u64;
    // SAFETY: SSCRATCH is a readable HS CSR. A nonzero value belongs to a
    // live lower-world anchor and must never be replaced by Native entry.
    unsafe {
        asm!("csrr {scratch}, sscratch", scratch = out(reg) scratch, options(nomem, nostack))
    };
    if scratch != 0 {
        return Err(Error::LowerWorldUnavailable);
    }
    if context.machine.state != READY {
        return Err(Error::InvalidMachineState);
    }
    if binding.run_generation() <= context.machine.run_generation {
        return Err(Error::RunGenerationNotIncreasing);
    }
    let publication = ACTIVE
        .get(super::current_cpu_index())
        .ok_or(Error::InvalidProcessor)?;
    if publication.generation.load(Ordering::Acquire) != 0 {
        return Err(Error::AlreadyRunning);
    }
    context.machine.thread = binding.thread();
    context.machine.image_generation = binding.image_generation();
    context.machine.run_generation = binding.run_generation();
    context.machine.frame.status = NATIVE_STATE;
    context.machine.exit_kind = EXIT_NONE;
    context.machine.state = RUNNING;
    publication
        .context
        .store(core::ptr::from_mut(&mut context.machine), Ordering::Relaxed);
    publication.service.store(
        core::ptr::from_ref(service)
            .cast::<NativeCallService<'static>>()
            .cast_mut(),
        Ordering::Relaxed,
    );
    publication
        .generation
        .store(binding.run_generation(), Ordering::Release);
    // SAFETY: Publication retains the uniquely borrowed context and service.
    // Assembly closes the run before returning with the kernel ABI restored.
    unsafe { riscv64_run_native_user(core::ptr::from_mut(&mut context.machine)) };
    if publication.generation.load(Ordering::Acquire) != 0
        || !publication.context.load(Ordering::Relaxed).is_null()
        || !publication.service.load(Ordering::Relaxed).is_null()
        || context.machine.state != STOPPED
    {
        fail_stop();
    }
    stopped_exit(context)
}
fn stopped_exit(context: &mut UserContext) -> Result<UserExit<'_>, Error> {
    let binding = current_binding(&context.machine)?;
    let payload = match context.machine.exit_kind {
        EXIT_NATIVE => ExitPayload::NativeCall(invocation(&context.machine.frame)?),
        EXIT_FAULT => ExitPayload::Fault(UserFault::new(
            fault_kind(context.machine.frame.cause),
            context.machine.frame.cause,
            context.machine.frame.fault_address,
            context.machine.frame.pc,
        )),
        EXIT_INTERRUPTED => ExitPayload::Interrupted,
        _ => return Err(Error::InvalidMachineState),
    };
    let completion = ReturnCapability {
        context: Some(&mut context.machine),
        binding,
    };
    match payload {
        ExitPayload::NativeCall(invocation) => Ok(UserExit::NativeCall {
            invocation,
            completion,
        }),
        ExitPayload::Fault(fault) => Ok(UserExit::Fault { fault, completion }),
        ExitPayload::Interrupted => Ok(UserExit::Interrupted { completion }),
    }
}
enum ExitPayload {
    NativeCall(NativeInvocation),
    Fault(UserFault),
    Interrupted,
}

pub(crate) enum UserExit<'context> {
    NativeCall {
        invocation: NativeInvocation,
        completion: ReturnCapability<'context>,
    },
    Fault {
        fault: UserFault,
        completion: ReturnCapability<'context>,
    },
    Interrupted {
        completion: ReturnCapability<'context>,
    },
}

#[must_use = "native-user return ownership must be resumed or discarded exactly once"]
pub(crate) struct ReturnCapability<'context> {
    context: Option<&'context mut MachineContext>,
    binding: UserRunBinding,
}

impl<'context> ReturnCapability<'context> {
    pub(crate) const fn binding(&self) -> UserRunBinding {
        self.binding
    }

    pub(crate) fn complete_native(
        mut self,
        expected: UserRunBinding,
        result: NativeResult,
    ) -> Result<(), CompletionFailure<'context>> {
        if let Err(error) = self.validate(expected) {
            return Err(CompletionFailure {
                error,
                completion: self,
            });
        }
        let context = self.context_mut();
        context.frame.general[10] = result.status() as u64;
        context.frame.general[11] = result.values()[0];
        context.frame.general[12] = result.values()[1];
        context.exit_kind = EXIT_NONE;
        context.state = READY;
        self.context = None;
        Ok(())
    }

    pub(crate) fn resume_execution(
        mut self,
        expected: UserRunBinding,
    ) -> Result<(), CompletionFailure<'context>> {
        if let Err(error) = self.validate(expected) {
            return Err(CompletionFailure {
                error,
                completion: self,
            });
        }
        let context = self.context_mut();
        context.exit_kind = EXIT_NONE;
        context.state = READY;
        self.context = None;
        Ok(())
    }

    pub(crate) fn discard(
        mut self,
        expected: UserRunBinding,
    ) -> Result<(), CompletionFailure<'context>> {
        if let Err(error) = self.validate(expected) {
            return Err(CompletionFailure {
                error,
                completion: self,
            });
        }
        let context = self.context_mut();
        context.exit_kind = EXIT_NONE;
        context.state = TERMINATED;
        self.context = None;
        Ok(())
    }

    fn validate(&self, expected: UserRunBinding) -> Result<(), Error> {
        let Some(context) = self.context.as_deref() else {
            return Err(Error::InvalidMachineState);
        };
        if self.binding != expected
            || current_binding(context) != Ok(expected)
            || context.state != STOPPED
        {
            return Err(Error::CompletionBindingMismatch);
        }
        Ok(())
    }

    fn context_mut(&mut self) -> &mut MachineContext {
        let Some(context) = self.context.as_deref_mut() else {
            fail_stop();
        };
        context
    }
}

impl Drop for ReturnCapability<'_> {
    fn drop(&mut self) {
        if self.context.is_some() {
            fail_stop();
        }
    }
}

#[must_use = "completion failure retains the exactly-once return capability"]
pub(crate) struct CompletionFailure<'context> {
    error: Error,
    completion: ReturnCapability<'context>,
}

impl<'context> CompletionFailure<'context> {
    pub(crate) fn into_parts(self) -> (Error, ReturnCapability<'context>) {
        (self.error, self.completion)
    }
}

struct ActiveRun {
    context: NonNull<MachineContext>,
    service: NonNull<NativeCallService<'static>>,
    generation: u64,
}
fn active_run() -> Result<ActiveRun, Error> {
    let publication = ACTIVE
        .get(super::current_cpu_index())
        .ok_or(Error::InvalidProcessor)?;
    let generation = publication.generation.load(Ordering::Acquire);
    if generation == 0 {
        return Err(Error::InvalidMachineState);
    }
    Ok(ActiveRun {
        context: NonNull::new(publication.context.load(Ordering::Relaxed))
            .ok_or(Error::InvalidMachineState)?,
        service: NonNull::new(publication.service.load(Ordering::Relaxed))
            .ok_or(Error::InvalidMachineState)?,
        generation,
    })
}
fn invocation(frame: &NativeFrame) -> Result<NativeInvocation, Error> {
    Ok(NativeInvocation::new(
        frame.general[17],
        [
            frame.general[10],
            frame.general[11],
            frame.general[12],
            frame.general[13],
            frame.general[14],
            frame.general[15],
        ],
        frame.pc.checked_sub(4).ok_or(Error::InvalidMachineState)?,
    ))
}
fn capture(mut active: ActiveRun, frame: &NativeFrame, exit_kind: u64) -> Result<(), Error> {
    // SAFETY: Acquired publication names the exclusive context of this masked,
    // pinned run; no Rust reference into it survives the assembly entry call.
    let context = unsafe { active.context.as_mut() };
    if context.state != RUNNING || context.run_generation != active.generation {
        return Err(Error::InvalidMachineState);
    }
    context.frame = *frame;
    context.exit_kind = exit_kind;
    context.state = STOPPED;
    let publication = ACTIVE
        .get(super::current_cpu_index())
        .ok_or(Error::InvalidProcessor)?;
    publication
        .context
        .store(core::ptr::null_mut(), Ordering::Relaxed);
    publication
        .service
        .store(core::ptr::null_mut(), Ordering::Relaxed);
    publication.generation.store(0, Ordering::Release);
    Ok(())
}

#[unsafe(no_mangle)]
extern "C" fn riscv64_native_trap_dispatch(frame: &mut NativeFrame) -> u64 {
    // This vector is installed only during an admitted U run. A supervisor
    // exception during assembly transition is corruption, never a user fault.
    if frame.status & registers::SSTATUS_SPP != 0 {
        fail_stop();
    }
    if frame.cause >> 63 != 0 {
        let top = super::exception::native_irq_stack_top();
        // SAFETY: Entry restored host gp/tp, stvec and zero sscratch. The
        // dedicated IRQ stack is live and unused on this interrupt-masked CPU.
        return unsafe {
            call_native_irq_on_stack(core::ptr::from_mut(frame), top, dispatch_native_irq)
        };
    }
    match dispatch_synchronous(frame) {
        Ok(action) => action,
        Err(_) => fail_stop(),
    }
}
fn dispatch_synchronous(frame: &mut NativeFrame) -> Result<u64, Error> {
    let active = active_run()?;
    if frame.cause == 8 {
        // ECALL is always four bytes, even with compressed instructions enabled.
        frame.pc = frame.pc.checked_add(4).ok_or(Error::InvalidMachineState)?;
        let call = invocation(frame)?;
        // SAFETY: The acquired CPU-local run retains this masked, nonblocking
        // service; invocation is owned and no architecture frame is borrowed.
        match unsafe { active.service.as_ref().handle(call) } {
            NativeCallAction::Return(result) => {
                frame.general[10] = result.status() as u64;
                frame.general[11] = result.values()[0];
                frame.general[12] = result.values()[1];
                #[cfg(feature = "kernel-self-test")]
                DIRECT_NATIVE_CALLS.fetch_add(1, Ordering::Relaxed);
                return Ok(0);
            }
            NativeCallAction::Unwind => capture(active, frame, EXIT_NATIVE)?,
        }
    } else {
        capture(active, frame, EXIT_FAULT)?;
    }
    Ok(1)
}
extern "C" fn dispatch_native_irq(frame: &mut NativeFrame) -> u64 {
    let origin = InterruptOrigin::Native {
        unwind: riscv64_unwind_native_user,
    };
    let action = match frame.cause & !(1 << 63) {
        1 => {
            super::interrupts::clear_software_interrupt();
            crate::arch::irq::service_kernel_rpc_interrupt(origin)
        }
        5 => crate::arch::irq::dispatch_entry(InterruptId::new(0), origin),
        9 => match crate::arch::irq::claim_and_dispatch_external_entry(origin) {
            Some(action) => action,
            None => return 0,
        },
        _ => fail_stop(),
    };
    match action {
        EntryAction::Resume { postlude: None } => 0,
        EntryAction::Resume {
            postlude: Some(target),
        } => {
            if target as usize != riscv64_unwind_native_user as *const () as usize {
                fail_stop();
            }
            let active = match active_run() {
                Ok(active) => active,
                Err(_) => fail_stop(),
            };
            if capture(active, frame, EXIT_INTERRUPTED).is_err() {
                fail_stop();
            }
            // Assembly invokes the qualified postlude after the IRQ stack and
            // all frame borrows are gone. It restores the original host anchor.
            1
        }
        EntryAction::StopGuest { .. } | EntryAction::Stop => fail_stop(),
    }
}
fn current_binding(context: &MachineContext) -> Result<UserRunBinding, Error> {
    UserRunBinding::new(
        context.thread,
        context.image_generation,
        context.run_generation,
    )
    .ok_or(Error::InvalidMachineState)
}
fn fault_kind(cause: u64) -> UserFaultKind {
    match cause {
        0 | 4 | 6 => UserFaultKind::Alignment,
        1 | 12 => UserFaultKind::InstructionAbort,
        5 | 7 | 13 => UserFaultKind::DataAbort,
        15 => UserFaultKind::WritePageFault,
        2 => UserFaultKind::IllegalInstruction,
        3 => UserFaultKind::Breakpoint,
        _ => UserFaultKind::OtherSynchronous,
    }
}
fn fail_stop() -> ! {
    loop {
        // SAFETY: A broken run owner cannot return safely; stop this masked hart.
        unsafe { asm!("csrci sstatus, 2", "wfi", options(nomem, nostack)) };
    }
}
const _: () = {
    assert!(offset_of!(NativeFrame, general) == 0);
    assert!(offset_of!(NativeFrame, pc) == registers::USER_FRAME_PC_OFFSET as usize);
    assert!(offset_of!(NativeFrame, status) == registers::USER_FRAME_STATUS_OFFSET as usize);
    assert!(offset_of!(NativeFrame, cause) == registers::USER_FRAME_CAUSE_OFFSET as usize);
    assert!(offset_of!(NativeFrame, fault_address) == registers::USER_FRAME_FAULT_OFFSET as usize);
    assert!(offset_of!(NativeFrame, floating) == registers::USER_FRAME_FLOATING_OFFSET as usize);
    assert!(offset_of!(NativeFrame, fcsr) == registers::USER_FRAME_FCSR_OFFSET as usize);
    assert!(size_of::<NativeFrame>() == registers::USER_FRAME_SIZE as usize);
    assert!(offset_of!(MachineContext, frame) == 0);
};

/// Raw LP64D register-isolation payload, copied into test-owned user memory.
#[cfg(feature = "kernel-self-test")]
pub(crate) fn native_register_test_program_for_test() -> &'static [u8] {
    unsafe extern "C" {
        static riscv64_native_register_test_start: u8;
        static riscv64_native_register_test_end: u8;
    }
    let start = core::ptr::addr_of!(riscv64_native_register_test_start);
    let end = core::ptr::addr_of!(riscv64_native_register_test_end);
    let length = end.addr() - start.addr();
    // SAFETY: Assembly bounds one immutable, retained position-independent
    // payload. Its bytes remain mapped and unchanged for the machine lifetime.
    unsafe { core::slice::from_raw_parts(start, length) }
}

/// Containment probes: supervisor-only load through a0, then privileged SATP.
/// A forbidden instruction which incorrectly completes exits with -99, so an
/// unrelated later instruction fault cannot masquerade as successful isolation.
#[cfg(feature = "kernel-self-test")]
pub(crate) fn native_fault_test_programs_for_test() -> [&'static [u8]; 2] {
    const SUPERVISOR_LOAD: [u8; 16] = [
        0x83, 0x32, 0x05, 0x00, // ld t0, 0(a0).
        0x13, 0x05, 0xd0, 0xf9, // li a0, -99.
        0x93, 0x08, 0x70, 0x00, // li a7, 7; thread_exit.
        0x73, 0x00, 0x00, 0x00, // ecall.
    ];
    const PRIVILEGED_CSR: [u8; 16] = [
        0xf3, 0x22, 0x00, 0x18, // csrr t0, satp.
        0x13, 0x05, 0xd0, 0xf9, // li a0, -99.
        0x93, 0x08, 0x70, 0x00, // li a7, 7; thread_exit.
        0x73, 0x00, 0x00, 0x00, // ecall.
    ];
    [&SUPERVISOR_LOAD, &PRIVILEGED_CSR]
}
