// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Call-like native EL0 execution and architecture-private return ownership.
//!
//! A raw vector frame never crosses this module. Native entry publishes a
//! generation-qualified pointer to a pinned register context, and selected
//! lower-EL exceptions copy into that owner before assembly returns to the
//! original kernel continuation.

use core::arch::asm;
use core::mem::{offset_of, size_of};
use core::ptr::NonNull;

use hyper::abi::native::{NativeInvocation, NativeResult};
use hyper::hal::user::{UserFault, UserFaultKind, UserRunBinding};
use hyper::sync::atomic::{AtomicU64, Ordering};

use super::exception::ExceptionFrame;
use super::user_contract::{LowerElReturnRegime, UserMachineContractError, UserTranslationRegime};
use super::{registers, user};

const MAX_CPUS: usize = hyper::config::MAX_CPUS as usize;

#[repr(C, align(16))]
struct MachineContext {
    general: [u64; 31],
    program_counter: u64,
    processor_state: u64,
    stack_pointer: u64,
    simd: [[u64; 2]; 32],
    fpcr: u64,
    fpsr: u64,
    tpidr_el0: u64,
    tpidrro_el0: u64,
    thread: u64,
    image_generation: u64,
    run_generation: u64,
    state: u64,
    exit_kind: u64,
    syndrome: u64,
    fault_address: u64,
    entry_hcr: u64,
}

const _: () = {
    assert!(offset_of!(MachineContext, general) == registers::USER_CONTEXT_X0_OFFSET as usize);
    assert!(
        offset_of!(MachineContext, program_counter) == registers::USER_CONTEXT_PC_OFFSET as usize
    );
    assert!(
        offset_of!(MachineContext, processor_state)
            == registers::USER_CONTEXT_PSTATE_OFFSET as usize
    );
    assert!(
        offset_of!(MachineContext, stack_pointer) == registers::USER_CONTEXT_SP_OFFSET as usize
    );
    assert!(offset_of!(MachineContext, simd) == registers::USER_CONTEXT_SIMD_OFFSET as usize);
    assert!(offset_of!(MachineContext, fpcr) == registers::USER_CONTEXT_FPCR_OFFSET as usize);
    assert!(offset_of!(MachineContext, fpsr) == registers::USER_CONTEXT_FPSR_OFFSET as usize);
    assert!(
        offset_of!(MachineContext, tpidr_el0) == registers::USER_CONTEXT_TPIDR_EL0_OFFSET as usize
    );
    assert!(
        offset_of!(MachineContext, tpidrro_el0)
            == registers::USER_CONTEXT_TPIDRRO_EL0_OFFSET as usize
    );
    assert!(offset_of!(MachineContext, thread) == registers::USER_CONTEXT_THREAD_OFFSET as usize);
    assert!(
        offset_of!(MachineContext, image_generation)
            == registers::USER_CONTEXT_IMAGE_GENERATION_OFFSET as usize
    );
    assert!(
        offset_of!(MachineContext, run_generation)
            == registers::USER_CONTEXT_RUN_GENERATION_OFFSET as usize
    );
    assert!(offset_of!(MachineContext, state) == registers::USER_CONTEXT_STATE_OFFSET as usize);
    assert!(
        offset_of!(MachineContext, exit_kind) == registers::USER_CONTEXT_EXIT_KIND_OFFSET as usize
    );
    assert!(
        offset_of!(MachineContext, syndrome) == registers::USER_CONTEXT_SYNDROME_OFFSET as usize
    );
    assert!(
        offset_of!(MachineContext, fault_address)
            == registers::USER_CONTEXT_FAULT_ADDRESS_OFFSET as usize
    );
    assert!(
        offset_of!(MachineContext, entry_hcr) == registers::USER_CONTEXT_ENTRY_HCR_OFFSET as usize
    );
    assert!(size_of::<MachineContext>() == registers::USER_CONTEXT_SIZE as usize);
};

/// Pinned architecture register owner attached to one scheduler `UserThread`.
pub(crate) struct UserContext {
    machine: MachineContext,
    regime: UserTranslationRegime,
}

impl UserContext {
    pub(crate) fn try_new(
        entry: u64,
        stack: u64,
        tls: u64,
        address_limit: u64,
    ) -> Result<Self, Error> {
        if entry == 0
            || entry >= address_limit
            || !entry.is_multiple_of(registers::AARCH64_INSTRUCTION_SIZE)
            || stack == 0
            || stack >= address_limit
            || !stack.is_multiple_of(16)
            || tls >= address_limit
        {
            return Err(Error::InvalidInitialContext);
        }
        let capabilities = user::execution_capabilities()?;
        let regime = capabilities.regime();
        Ok(Self {
            machine: MachineContext {
                general: [0; 31],
                program_counter: entry,
                // Native userspace currently owns normal IRQ delivery only.
                // Keep debug, SError, and FIQ masked until those classes have
                // explicit context ownership and contained return paths.
                processor_state: native_processor_state(regime),
                stack_pointer: stack,
                simd: [[0; 2]; 32],
                fpcr: 0,
                fpsr: 0,
                tpidr_el0: tls,
                tpidrro_el0: 0,
                thread: 0,
                image_generation: 0,
                run_generation: 0,
                state: registers::USER_CONTEXT_STATE_READY,
                exit_kind: registers::USER_CONTEXT_EXIT_NONE,
                syndrome: 0,
                fault_address: 0,
                entry_hcr: 0,
            },
            regime,
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
    RunGenerationNotIncreasing,
    Unsupported(UserMachineContractError),
}

impl From<UserMachineContractError> for Error {
    fn from(error: UserMachineContractError) -> Self {
        Self::Unsupported(error)
    }
}

struct RunPublication {
    /// A nonzero generation publishes the preceding context pointer.
    generation: AtomicU64,
    context: AtomicU64,
}

impl RunPublication {
    const fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            context: AtomicU64::new(0),
        }
    }
}

static ACTIVE_RUNS: [RunPublication; MAX_CPUS] = [const { RunPublication::new() }; MAX_CPUS];

unsafe extern "C" {
    fn aarch64_run_native_user(context: *mut MachineContext);
    fn aarch64_unwind_native_user();
}

/// Runs one admitted user generation until an exit requires kernel policy.
///
/// The call returns through a vector-owned assembly unwind, not through ERET.
/// The caller must keep its native address space active and execution pinned
/// until this function returns, then deactivate it before inspecting the exit.
/// # Safety
///
/// The caller must keep `context` pinned and uniquely owned while the native
/// address space and the current-CPU execution pin remain active. No scheduler
/// transition may occur until this call returns and that address space is
/// deactivated.
pub(crate) unsafe fn run_user<'context>(
    context: &'context mut UserContext,
    binding: UserRunBinding,
) -> Result<UserExit<'context>, Error> {
    prepare_run(context, binding)?;
    let cpu = super::current_cpu_index();
    let Some(publication) = ACTIVE_RUNS.get(cpu) else {
        context.machine.state = registers::USER_CONTEXT_STATE_READY;
        return Err(Error::InvalidProcessor);
    };
    let context_address = core::ptr::from_mut(&mut context.machine).expose_provenance() as u64;
    if publication.generation.load(Ordering::Acquire) != 0
        || publication
            .context
            .compare_exchange(0, context_address, Ordering::Relaxed, Ordering::Relaxed)
            .is_err()
    {
        context.machine.state = registers::USER_CONTEXT_STATE_READY;
        return Err(Error::AlreadyRunning);
    }
    if publication
        .generation
        .swap(binding.run_generation(), Ordering::Release)
        != 0
    {
        // Same-CPU pinning excludes a competing publisher. Observing a
        // generation here means the mailbox invariants are already corrupt;
        // clearing either owner's pointer would make that corruption unsafe.
        fail_stop();
    }

    // SAFETY: The context is exclusively borrowed and pinned by the caller's
    // active address-space/run guard. Publication is complete, and assembly
    // returns only after exception entry has copied state and closed it.
    unsafe { aarch64_run_native_user(&mut context.machine) };

    if publication.generation.load(Ordering::Acquire) != 0
        || context.machine.state != registers::USER_CONTEXT_STATE_STOPPED
    {
        fail_stop();
    }
    stopped_exit(context)
}

fn prepare_run(context: &mut UserContext, binding: UserRunBinding) -> Result<(), Error> {
    if context.machine.state != registers::USER_CONTEXT_STATE_READY {
        return Err(Error::InvalidMachineState);
    }
    if binding.run_generation() <= context.machine.run_generation {
        return Err(Error::RunGenerationNotIncreasing);
    }
    let current_hcr = read_hcr_el2();
    let entry_hcr = LowerElReturnRegime::Native(context.regime).transition_hcr(current_hcr)?;
    context.machine.thread = binding.thread();
    context.machine.image_generation = binding.image_generation();
    context.machine.run_generation = binding.run_generation();
    context.machine.exit_kind = registers::USER_CONTEXT_EXIT_NONE;
    context.machine.syndrome = 0;
    context.machine.fault_address = 0;
    context.machine.entry_hcr = entry_hcr;
    context.machine.processor_state &= registers::SPSR_NZCV_MASK;
    context.machine.processor_state |= native_processor_state(context.regime);
    context.machine.state = registers::USER_CONTEXT_STATE_RUNNING;
    Ok(())
}

const fn native_processor_state(regime: UserTranslationRegime) -> u64 {
    let base = registers::SPSR_EL0T | registers::SPSR_D | registers::SPSR_A | registers::SPSR_F;
    match regime {
        // The VHE root contains both EL0 and privileged mappings. PAN remains
        // asserted across exception entry (SPAN=1) so ordinary kernel code
        // cannot accidentally bypass the typed user-copy boundary.
        UserTranslationRegime::VheHostStage1 => base | registers::SPSR_PAN,
        // nVHE user memory is stage-2-only and absent from the EL2 host root.
        UserTranslationRegime::NvheStage2Only => base,
    }
}

fn stopped_exit(context: &mut UserContext) -> Result<UserExit<'_>, Error> {
    let binding = current_binding(&context.machine)?;
    let payload = match context.machine.exit_kind {
        registers::USER_CONTEXT_EXIT_NATIVE_SYSCALL => {
            let call_site = context
                .machine
                .program_counter
                .checked_sub(registers::AARCH64_INSTRUCTION_SIZE)
                .ok_or(Error::InvalidMachineState)?;
            ExitPayload::NativeCall(NativeInvocation::new(
                context.machine.general[8],
                [
                    context.machine.general[0],
                    context.machine.general[1],
                    context.machine.general[2],
                    context.machine.general[3],
                    context.machine.general[4],
                    context.machine.general[5],
                ],
                call_site,
            ))
        }
        registers::USER_CONTEXT_EXIT_FAULT => ExitPayload::Fault(UserFault::new(
            fault_kind(context.machine.syndrome),
            context.machine.syndrome,
            context.machine.fault_address,
            context.machine.program_counter,
        )),
        registers::USER_CONTEXT_EXIT_INTERRUPTED => ExitPayload::Interrupted,
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
        context.general[0] = result.status() as u64;
        context.general[1] = result.values()[0];
        context.general[2] = result.values()[1];
        context.exit_kind = registers::USER_CONTEXT_EXIT_NONE;
        context.state = registers::USER_CONTEXT_STATE_READY;
        self.context = None;
        Ok(())
    }

    pub(crate) fn resume_interrupted(
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
        context.exit_kind = registers::USER_CONTEXT_EXIT_NONE;
        context.state = registers::USER_CONTEXT_STATE_READY;
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
        context.exit_kind = registers::USER_CONTEXT_EXIT_NONE;
        context.state = registers::USER_CONTEXT_STATE_TERMINATED;
        self.context = None;
        Ok(())
    }

    fn validate(&self, expected: UserRunBinding) -> Result<(), Error> {
        let Some(context) = self.context.as_deref() else {
            return Err(Error::InvalidMachineState);
        };
        if self.binding != expected
            || current_binding(context) != Ok(expected)
            || context.state != registers::USER_CONTEXT_STATE_STOPPED
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

/// Copies one native synchronous exception into its published context.
pub(super) fn capture_synchronous(frame: &ExceptionFrame) -> Result<bool, Error> {
    let Some((context, generation)) = active_context()? else {
        return Ok(false);
    };
    let class = (frame.esr >> registers::ESR_EC_SHIFT) & registers::ESR_EC_MASK;
    let valid_native_call =
        class == registers::ESR_EC_SVC64 && frame.esr & registers::ESR_ISS_MASK == 0;
    capture_frame(
        context,
        generation,
        frame,
        if valid_native_call {
            registers::USER_CONTEXT_EXIT_NATIVE_SYSCALL
        } else {
            registers::USER_CONTEXT_EXIT_FAULT
        },
    )?;
    Ok(true)
}

/// Copies an interrupted native context before a kernel-selected unwind.
pub(super) fn capture_interrupt(frame: &ExceptionFrame) -> Result<bool, Error> {
    let Some((context, generation)) = active_context()? else {
        return Ok(false);
    };
    capture_frame(
        context,
        generation,
        frame,
        registers::USER_CONTEXT_EXIT_INTERRUPTED,
    )?;
    Ok(true)
}

pub(super) fn active_on_current_cpu() -> bool {
    let cpu = super::current_cpu_index();
    ACTIVE_RUNS
        .get(cpu)
        .is_some_and(|publication| publication.generation.load(Ordering::Acquire) != 0)
}

pub(super) const fn unwind_callback() -> unsafe extern "C" fn() {
    aarch64_unwind_native_user
}

fn active_context() -> Result<Option<(NonNull<MachineContext>, u64)>, Error> {
    let cpu = super::current_cpu_index();
    let publication = ACTIVE_RUNS.get(cpu).ok_or(Error::InvalidProcessor)?;
    let generation = publication.generation.load(Ordering::Acquire);
    if generation == 0 {
        return Ok(None);
    }
    let raw = publication.context.load(Ordering::Relaxed);
    let pointer = NonNull::new(core::ptr::with_exposed_provenance_mut::<MachineContext>(
        raw as usize,
    ))
    .ok_or(Error::InvalidMachineState)?;
    Ok(Some((pointer, generation)))
}

fn capture_frame(
    mut pointer: NonNull<MachineContext>,
    generation: u64,
    frame: &ExceptionFrame,
    exit_kind: u64,
) -> Result<(), Error> {
    // SAFETY: The acquire in `active_context` observed a publication made
    // while the caller held the context's exclusive pinned borrow. Selected
    // native vector entry is the sole accessor on this CPU, and publication is
    // closed below before the call-like trampoline releases that borrow.
    let context = unsafe { pointer.as_mut() };
    if context.state != registers::USER_CONTEXT_STATE_RUNNING
        || context.run_generation != generation
    {
        return Err(Error::InvalidMachineState);
    }
    context.general = frame.general;
    context.program_counter = frame.elr;
    context.processor_state = frame.spsr;
    context.stack_pointer = frame.sp_el0;
    context.simd = frame.simd;
    context.fpcr = frame.fpcr;
    context.fpsr = frame.fpsr;
    // SAFETY: These thread registers are readable at EL2. The native run is
    // stopped and this context exclusively owns their lower-EL values.
    unsafe {
        asm!(
            "mrs {tpidr_el0}, TPIDR_EL0",
            "mrs {tpidrro_el0}, TPIDRRO_EL0",
            tpidr_el0 = out(reg) context.tpidr_el0,
            tpidrro_el0 = out(reg) context.tpidrro_el0,
            options(nomem, nostack, preserves_flags)
        )
    };
    context.exit_kind = exit_kind;
    context.syndrome = frame.esr;
    context.fault_address = frame.far;
    context.state = registers::USER_CONTEXT_STATE_STOPPED;

    let cpu = super::current_cpu_index();
    let publication = ACTIVE_RUNS.get(cpu).ok_or(Error::InvalidProcessor)?;
    publication.context.store(0, Ordering::Relaxed);
    publication.generation.store(0, Ordering::Release);
    Ok(())
}

fn current_binding(context: &MachineContext) -> Result<UserRunBinding, Error> {
    UserRunBinding::new(
        context.thread,
        context.image_generation,
        context.run_generation,
    )
    .ok_or(Error::InvalidMachineState)
}

fn fault_kind(syndrome: u64) -> UserFaultKind {
    let class = (syndrome >> registers::ESR_EC_SHIFT) & registers::ESR_EC_MASK;
    match class {
        registers::ESR_EC_INSTRUCTION_ABORT_LOWER => UserFaultKind::InstructionAbort,
        registers::ESR_EC_DATA_ABORT_LOWER => UserFaultKind::DataAbort,
        registers::ESR_EC_PC_ALIGNMENT | registers::ESR_EC_SP_ALIGNMENT => UserFaultKind::Alignment,
        registers::ESR_EC_SYSTEM_REGISTER => UserFaultKind::SystemAccess,
        registers::ESR_EC_BREAKPOINT_LOWER
        | registers::ESR_EC_SOFTWARE_STEP_LOWER
        | registers::ESR_EC_WATCHPOINT_LOWER
        | registers::ESR_EC_BRK64 => UserFaultKind::Breakpoint,
        registers::ESR_EC_UNKNOWN => UserFaultKind::IllegalInstruction,
        _ => UserFaultKind::OtherSynchronous,
    }
}

fn read_hcr_el2() -> u64 {
    let value: u64;
    // SAFETY: HCR_EL2 is readable at EL2 and has no memory operand.
    unsafe {
        asm!(
            "mrs {value}, HCR_EL2",
            value = out(reg) value,
            options(nomem, nostack, preserves_flags)
        )
    };
    value
}

pub(super) fn fail_stop() -> ! {
    loop {
        // SAFETY: Returning could expose a duplicated completion or an active
        // raw context. Masking and waiting is the lock-free local fail-stop.
        unsafe {
            asm!(
                "msr daifset, #0xf",
                "wfe",
                options(nomem, nostack, preserves_flags)
            )
        };
    }
}
