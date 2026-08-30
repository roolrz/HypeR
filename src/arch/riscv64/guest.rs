// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! RISC-V guest-exit capture, hardware emulation, and private completion.
//!
//! A trap frame belongs to architecture entry. [`dispatch`] copies the
//! guest-visible exit into fixed-width owned state, ends the frame borrow,
//! invokes the registered kernel service, and only then applies the returned
//! action. Neither the raw frame nor a vCPU-context borrow crosses that call.

use core::arch::asm;

use hyper::vm::exit::{GuestMemoryFault, GuestPhysicalAddress, MemoryAccess, MemoryFaultAction};

use super::{VcpuContext, VmInterruptController};

const CAUSE_VIRTUAL_SUPERVISOR_ECALL: u64 = 10;
const CAUSE_INSTRUCTION_GUEST_PAGE_FAULT: u64 = 20;
const CAUSE_LOAD_GUEST_PAGE_FAULT: u64 = 21;
const CAUSE_VIRTUAL_INSTRUCTION: u64 = 22;
const CAUSE_STORE_GUEST_PAGE_FAULT: u64 = 23;
const HVIP_VSTIP: usize = 1 << 6;

const SBI_EXT_BASE: u64 = 0x10;
const SBI_EXT_TIME: u64 = 0x5449_4d45;
const SBI_SUCCESS: u64 = 0;
const SBI_NOT_SUPPORTED: u64 = (-2isize) as u64;

const SYSTEM_OPCODE: u32 = 0x73;
const WFI: u32 = 0x1050_0073;
const CSR_SIE: u32 = 0x104;
const CSR_SIP: u32 = 0x144;
const CSR_SCOUNTEREN: u32 = 0x106;
const CSR_SENVCFG: u32 = 0x10a;

/// One owned RISC-V synchronous guest exit presented to kernel policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GuestSyncExit {
    VirtualInstruction(VirtualInstruction),
    SupervisorCall(SupervisorCall),
    Unsupported(UnsupportedGuestExit),
}

impl GuestSyncExit {
    /// Returns the byte carried by a legacy debug-console operation.
    pub(crate) const fn legacy_console_byte(self) -> Option<u8> {
        match self {
            Self::SupervisorCall(call) => call.legacy_console_byte(),
            Self::VirtualInstruction(_) | Self::Unsupported(_) => None,
        }
    }
}

/// A virtual instruction which can be completed by the RISC-V backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum VirtualInstruction {
    WaitForInterrupt,
    Csr(CsrInstruction),
}

/// One validated virtual CSR access.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CsrInstruction {
    register: VirtualCsr,
    operation: CsrOperation,
    source: u64,
}

/// Guest-visible virtual CSRs implemented by the initial backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VirtualCsr {
    InterruptEnable,
    InterruptPending,
    CounterEnable,
    EnvironmentConfiguration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CsrOperation {
    Write,
    SetBits,
    ClearBits,
}

/// One copied supervisor binary interface invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SupervisorCall {
    extension: u64,
    function: u64,
    arguments: [u64; 6],
}

impl SupervisorCall {
    /// Returns the byte carried by the legacy debug-console call.
    ///
    /// Console routing is kernel VM policy; architecture code only identifies
    /// the ABI operation and later completes its register convention.
    pub(crate) const fn legacy_console_byte(self) -> Option<u8> {
        if self.extension == 1 {
            Some(self.arguments[0] as u8)
        } else {
            None
        }
    }
}

/// Diagnostic details for a synchronous exit which cannot be resumed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct UnsupportedGuestExit {
    pub(crate) cause: u64,
    pub(crate) trap_value: u64,
    pub(crate) transformed_instruction: u64,
    pub(crate) reason: UnsupportedReason,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UnsupportedReason {
    GuestPhysicalAddressUnavailable,
    SynchronousCause,
    VirtualInstruction,
}

/// The exhaustive architecture completion returned by VM-exit policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[must_use]
pub(crate) enum GuestSyncAction {
    ResumeVirtualInstruction {
        value: Option<u64>,
    },
    CompleteSupervisorCall(SupervisorCallResult),
    ProgramTimer {
        deadline: u64,
        result: SupervisorCallResult,
    },
    Stop,
}

impl GuestSyncAction {
    /// Completes one legacy debug-console call after kernel policy emitted it.
    pub(crate) const fn complete_legacy_console() -> Self {
        Self::CompleteSupervisorCall(SupervisorCallResult::Legacy { value: SBI_SUCCESS })
    }
}

/// Register result following the legacy or modern SBI convention.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SupervisorCallResult {
    /// Legacy SBI v0.1 writes only `a0`; `a1` remains guest-owned.
    Legacy { value: u64 },
    /// SBI v0.2 and later write the error to `a0` and value to `a1`.
    Modern { error: u64, value: u64 },
}

#[derive(Debug, Eq, PartialEq)]
enum CapturedExit {
    MemoryFault(GuestMemoryFault),
    Synchronous {
        exit: GuestSyncExit,
        completion: Completion,
    },
}

#[derive(Debug, Eq, PartialEq)]
enum Completion {
    VirtualInstruction { destination: usize },
    SupervisorCall(SupervisorCall),
    Unsupported,
}

/// Captures, dispatches, and completes one synchronous guest exit.
///
/// Local interrupts are masked by hardware on trap entry. Capture returns only
/// copied state, so the raw frame borrow ends before either kernel service may
/// allocate or take VM locks.
pub(crate) fn dispatch(frame: &mut super::exception::TrapFrame) -> bool {
    let captured = capture(frame);
    let action = match &captured {
        CapturedExit::MemoryFault(fault) => {
            return matches!(
                crate::arch::vm::dispatch_memory_fault(*fault),
                MemoryFaultAction::Retry
            );
        }
        CapturedExit::Synchronous { exit, .. } => crate::arch::vm::dispatch_guest_sync(*exit),
    };
    apply(frame, captured, action)
}

fn capture(frame: &super::exception::TrapFrame) -> CapturedExit {
    if let Some(fault) =
        decode_guest_memory_fault(frame.scause, frame.stval, frame.htval, frame.htinst)
    {
        return CapturedExit::MemoryFault(fault);
    }
    if matches!(
        frame.scause,
        CAUSE_INSTRUCTION_GUEST_PAGE_FAULT
            | CAUSE_LOAD_GUEST_PAGE_FAULT
            | CAUSE_STORE_GUEST_PAGE_FAULT
    ) {
        return unsupported(frame, UnsupportedReason::GuestPhysicalAddressUnavailable);
    }
    if frame.scause == CAUSE_VIRTUAL_INSTRUCTION {
        return capture_virtual_instruction(frame);
    }
    if frame.scause == CAUSE_VIRTUAL_SUPERVISOR_ECALL {
        let call = SupervisorCall {
            extension: frame.general[17],
            function: frame.general[16],
            arguments: [
                frame.general[10],
                frame.general[11],
                frame.general[12],
                frame.general[13],
                frame.general[14],
                frame.general[15],
            ],
        };
        return CapturedExit::Synchronous {
            exit: GuestSyncExit::SupervisorCall(call),
            completion: Completion::SupervisorCall(call),
        };
    }
    unsupported(frame, UnsupportedReason::SynchronousCause)
}

fn capture_virtual_instruction(frame: &super::exception::TrapFrame) -> CapturedExit {
    // For a virtual-instruction exception, STVAL contains the trapped
    // instruction. HTINST is reserved for transformed instructions associated
    // with guest-page faults and must not be preferred here.
    let instruction = frame.stval as u32;
    if instruction == WFI {
        return CapturedExit::Synchronous {
            exit: GuestSyncExit::VirtualInstruction(VirtualInstruction::WaitForInterrupt),
            completion: Completion::VirtualInstruction { destination: 0 },
        };
    }
    let Some((instruction, destination)) = decode_csr_instruction(instruction, &frame.general)
    else {
        return unsupported(frame, UnsupportedReason::VirtualInstruction);
    };
    CapturedExit::Synchronous {
        exit: GuestSyncExit::VirtualInstruction(VirtualInstruction::Csr(instruction)),
        completion: Completion::VirtualInstruction { destination },
    }
}

fn decode_csr_instruction(
    instruction: u32,
    general: &[u64; 32],
) -> Option<(CsrInstruction, usize)> {
    if instruction & 0x7f != SYSTEM_OPCODE {
        return None;
    }
    let function = (instruction >> 12) & 7;
    let source_register = ((instruction >> 15) & 0x1f) as usize;
    let destination = ((instruction >> 7) & 0x1f) as usize;
    let register = match instruction >> 20 {
        CSR_SIE => VirtualCsr::InterruptEnable,
        CSR_SIP => VirtualCsr::InterruptPending,
        CSR_SCOUNTEREN => VirtualCsr::CounterEnable,
        CSR_SENVCFG => VirtualCsr::EnvironmentConfiguration,
        _ => return None,
    };
    let operation = match function {
        1 | 5 => CsrOperation::Write,
        2 | 6 => CsrOperation::SetBits,
        3 | 7 => CsrOperation::ClearBits,
        _ => return None,
    };
    let source = if function >= 5 {
        source_register as u64
    } else {
        general.get(source_register).copied()?
    };
    Some((
        CsrInstruction {
            register,
            operation,
            source,
        },
        destination,
    ))
}

fn unsupported(frame: &super::exception::TrapFrame, reason: UnsupportedReason) -> CapturedExit {
    CapturedExit::Synchronous {
        exit: GuestSyncExit::Unsupported(UnsupportedGuestExit {
            cause: frame.scause,
            trap_value: frame.stval,
            transformed_instruction: frame.htinst,
            reason,
        }),
        completion: Completion::Unsupported,
    }
}

fn decode_guest_memory_fault(
    cause: u64,
    trap_value: u64,
    guest_physical_address: u64,
    transformed_instruction: u64,
) -> Option<GuestMemoryFault> {
    let guest_access = match cause {
        CAUSE_INSTRUCTION_GUEST_PAGE_FAULT => MemoryAccess::Execute,
        CAUSE_LOAD_GUEST_PAGE_FAULT => MemoryAccess::Read,
        CAUSE_STORE_GUEST_PAGE_FAULT => MemoryAccess::Write,
        _ => return None,
    };
    // HTVAL may be zero when the implementation omits the GPA, but zero is
    // also a valid encoded GPA. Refusing demand paging is the only safe choice.
    if guest_physical_address == 0 {
        return None;
    }
    let (access, during_guest_page_walk) = guest_page_walk_access(transformed_instruction)
        .map_or((guest_access, false), |access| (access, true));
    Some(GuestMemoryFault::new(
        GuestPhysicalAddress::new(guest_physical_fault_address(
            guest_physical_address,
            trap_value,
            during_guest_page_walk,
        )),
        access,
        during_guest_page_walk,
    ))
}

fn guest_page_walk_access(instruction: u64) -> Option<MemoryAccess> {
    // The privileged ISA requires one of these values when an implicit
    // VS-stage page-table access faults and HTVAL is nonzero. Other nonzero
    // HTINST values encode the explicit instruction which faulted.
    // https://docs.riscv.org/reference/isa/priv/hypervisor.html
    match instruction {
        0x2000 | 0x3000 => Some(MemoryAccess::Read),
        0x2020 | 0x3020 => Some(MemoryAccess::Write),
        _ => None,
    }
}

fn guest_physical_fault_address(htval: u64, stval: u64, during_guest_page_walk: bool) -> u64 {
    let low_bits = if during_guest_page_walk { 0 } else { stval & 3 };
    (htval << 2) | low_bits
}

/// Applies backend-local virtual-instruction and SBI mechanisms.
///
/// The kernel calls this only while its active-vCPU capability exclusively
/// borrows `context`. The owned exit contains no trap-frame reference.
pub(crate) fn handle_guest_sync(
    context: &mut VcpuContext,
    _vcpu_id: u32,
    _interrupts: &VmInterruptController,
    exit: GuestSyncExit,
) -> GuestSyncAction {
    match exit {
        GuestSyncExit::VirtualInstruction(instruction) => {
            emulate_virtual_instruction(context, instruction)
        }
        GuestSyncExit::SupervisorCall(call) => emulate_sbi(call),
        GuestSyncExit::Unsupported(_) => GuestSyncAction::Stop,
    }
}

fn emulate_virtual_instruction(
    context: &mut VcpuContext,
    instruction: VirtualInstruction,
) -> GuestSyncAction {
    let VirtualInstruction::Csr(instruction) = instruction else {
        return GuestSyncAction::ResumeVirtualInstruction { value: None };
    };
    let old = match instruction.register {
        VirtualCsr::InterruptEnable => read_vsie(),
        VirtualCsr::InterruptPending => read_vsip(),
        VirtualCsr::CounterEnable => context.scounteren,
        VirtualCsr::EnvironmentConfiguration => context.senvcfg,
    };
    let new_value = match instruction.operation {
        CsrOperation::Write => Some(instruction.source),
        CsrOperation::SetBits if instruction.source != 0 => Some(old | instruction.source),
        CsrOperation::ClearBits if instruction.source != 0 => Some(old & !instruction.source),
        CsrOperation::SetBits | CsrOperation::ClearBits => None,
    };
    if let Some(value) = new_value {
        match instruction.register {
            VirtualCsr::InterruptEnable => write_vsie(value),
            VirtualCsr::InterruptPending => write_vsip(value),
            VirtualCsr::CounterEnable => {
                context.scounteren = value;
                write_hcounteren(value);
            }
            VirtualCsr::EnvironmentConfiguration => context.senvcfg = value,
        }
    }
    GuestSyncAction::ResumeVirtualInstruction { value: Some(old) }
}

fn read_vsie() -> u64 {
    let value: u64;
    // SAFETY: VSIE is accessible in HS mode while the guest context is active.
    unsafe { asm!("csrr {value}, vsie", value = out(reg) value, options(nomem, nostack)) };
    value
}

fn write_vsie(value: u64) {
    // SAFETY: VSIE is writable in HS mode while the guest context is active.
    unsafe { asm!("csrw vsie, {value}", value = in(reg) value, options(nostack)) };
}

fn read_vsip() -> u64 {
    let value: u64;
    // SAFETY: VSIP is accessible in HS mode while the guest context is active.
    unsafe { asm!("csrr {value}, vsip", value = out(reg) value, options(nomem, nostack)) };
    value
}

fn write_vsip(value: u64) {
    // SAFETY: VSIP is writable in HS mode while the guest context is active.
    unsafe { asm!("csrw vsip, {value}", value = in(reg) value, options(nostack)) };
}

fn write_hcounteren(value: u64) {
    // SAFETY: HCOUNTEREN is writable in HS mode.
    unsafe { asm!("csrw hcounteren, {value}", value = in(reg) value, options(nostack)) };
}

fn emulate_sbi(call: SupervisorCall) -> GuestSyncAction {
    match call.extension {
        0 => GuestSyncAction::ProgramTimer {
            deadline: call.arguments[0],
            result: SupervisorCallResult::Legacy { value: SBI_SUCCESS },
        },
        // Legacy console output is intercepted by kernel policy before this
        // backend helper. Reaching it means the service violated its contract.
        1 => GuestSyncAction::Stop,
        2 => GuestSyncAction::CompleteSupervisorCall(SupervisorCallResult::Legacy {
            value: u64::MAX,
        }),
        3 => {
            clear_legacy_software_interrupt();
            GuestSyncAction::CompleteSupervisorCall(SupervisorCallResult::Legacy {
                value: SBI_SUCCESS,
            })
        }
        // Legacy remote IPI/fence and shutdown calls have no error return with
        // which to report absence. Do not silently claim their side effects.
        4..=8 => GuestSyncAction::Stop,
        _ => emulate_modern_sbi(call),
    }
}

fn emulate_modern_sbi(call: SupervisorCall) -> GuestSyncAction {
    let (error, value) = match (call.extension, call.function) {
        (SBI_EXT_BASE, 0) => (SBI_SUCCESS, 0x0000_0003),
        (SBI_EXT_BASE, 1 | 2 | 4..=6) => (SBI_SUCCESS, 0),
        (SBI_EXT_BASE, 3) => {
            let available = modern_extension_available(call.arguments[0]);
            (SBI_SUCCESS, u64::from(available))
        }
        (SBI_EXT_TIME, 0) => {
            return GuestSyncAction::ProgramTimer {
                deadline: call.arguments[0],
                result: SupervisorCallResult::Modern {
                    error: SBI_SUCCESS,
                    value: 0,
                },
            };
        }
        _ => (SBI_NOT_SUPPORTED, 0),
    };
    GuestSyncAction::CompleteSupervisorCall(SupervisorCallResult::Modern { error, value })
}

const fn modern_extension_available(extension: u64) -> bool {
    extension == SBI_EXT_TIME
}

fn clear_legacy_software_interrupt() {
    const HVIP_VSSIP: usize = 1 << 2;
    // SAFETY: HVIP is writable in HS mode. Clearing VSSIP implements the only
    // legacy local-IPI operation whose complete side effect is CPU-local.
    unsafe { asm!("csrc hvip, {mask}", mask = in(reg) HVIP_VSSIP, options(nostack)) };
}

fn apply(
    frame: &mut super::exception::TrapFrame,
    captured: CapturedExit,
    action: GuestSyncAction,
) -> bool {
    let CapturedExit::Synchronous { completion, .. } = captured else {
        return false;
    };
    match (completion, action) {
        (
            Completion::VirtualInstruction { destination },
            GuestSyncAction::ResumeVirtualInstruction { value },
        ) => {
            apply_virtual_instruction_result(&mut frame.general, destination, value);
            advance_program_counter(frame);
            true
        }
        (Completion::SupervisorCall(call), GuestSyncAction::CompleteSupervisorCall(result))
            if supervisor_completion_matches(call, result) =>
        {
            apply_supervisor_call_result(&mut frame.general, result);
            advance_program_counter(frame);
            true
        }
        (Completion::SupervisorCall(call), GuestSyncAction::ProgramTimer { deadline, result })
            if supervisor_timer_matches(call, result) =>
        {
            set_timer(deadline);
            apply_supervisor_call_result(&mut frame.general, result);
            advance_program_counter(frame);
            true
        }
        (Completion::Unsupported, GuestSyncAction::Stop) => false,
        _ => false,
    }
}

const fn supervisor_result_matches(call: SupervisorCall, result: SupervisorCallResult) -> bool {
    matches!(
        (call.extension <= 8, result),
        (true, SupervisorCallResult::Legacy { .. }) | (false, SupervisorCallResult::Modern { .. })
    )
}

const fn supervisor_completion_matches(call: SupervisorCall, result: SupervisorCallResult) -> bool {
    !supervisor_call_is_timer(call) && supervisor_result_matches(call, result)
}

const fn supervisor_timer_matches(call: SupervisorCall, result: SupervisorCallResult) -> bool {
    supervisor_call_is_timer(call) && supervisor_result_matches(call, result)
}

const fn supervisor_call_is_timer(call: SupervisorCall) -> bool {
    call.extension == 0 || (call.extension == SBI_EXT_TIME && call.function == 0)
}

fn apply_supervisor_call_result(general: &mut [u64; 32], result: SupervisorCallResult) {
    match result {
        SupervisorCallResult::Legacy { value } => general[10] = value,
        SupervisorCallResult::Modern { error, value } => {
            general[10] = error;
            general[11] = value;
        }
    }
}

fn apply_virtual_instruction_result(
    general: &mut [u64; 32],
    destination: usize,
    value: Option<u64>,
) {
    if destination != 0
        && let Some(value) = value
        && let Some(register) = general.get_mut(destination)
    {
        *register = value;
    }
}

fn advance_program_counter(frame: &mut super::exception::TrapFrame) {
    // ECALL, WFI, and every CSR instruction accepted above are 32-bit SYSTEM
    // encodings; none has a compressed representation in these exit classes.
    advance_program_counter_value(&mut frame.sepc);
}

fn advance_program_counter_value(program_counter: &mut u64) {
    *program_counter = program_counter.wrapping_add(4);
}

fn set_timer(deadline: u64) {
    // SAFETY: These hypervisor CSRs are writable in HS mode while the active
    // vCPU is exclusively owned on this hart.
    unsafe {
        asm!(
            "csrs henvcfg, {stce}",
            "csrw 0x24d, {deadline}",
            "csrc hvip, {mask}",
            stce = in(reg) 1usize << 63,
            deadline = in(reg) deadline,
            mask = in(reg) HVIP_VSTIP,
            options(nostack)
        )
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationError {
    CsrDecoder,
    GuestMemoryFaultDecoder,
    SupervisorCallCompletion,
}

pub(super) fn validate() -> Result<(), ValidationError> {
    if guest_page_walk_access(0x3000) != Some(MemoryAccess::Read)
        || guest_page_walk_access(0x3020) != Some(MemoryAccess::Write)
        || guest_page_walk_access(0x0000_20c3).is_some()
        || guest_physical_fault_address(0x1234, 3, true) != 0x48d0
        || guest_physical_fault_address(0x1234, 3, false) != 0x48d3
        || decode_guest_memory_fault(CAUSE_LOAD_GUEST_PAGE_FAULT, 0, 0, 0).is_some()
    {
        return Err(ValidationError::GuestMemoryFaultDecoder);
    }

    let mut registers = [0u64; 32];
    registers[5] = 0x55aa;
    // csrrw x7, sie, x5
    let csr = (CSR_SIE << 20) | (5 << 15) | (1 << 12) | (7 << 7) | SYSTEM_OPCODE;
    let Some((decoded, destination)) = decode_csr_instruction(csr, &registers) else {
        return Err(ValidationError::CsrDecoder);
    };
    if decoded.register != VirtualCsr::InterruptEnable
        || decoded.operation != CsrOperation::Write
        || decoded.source != 0x55aa
        || destination != 7
    {
        return Err(ValidationError::CsrDecoder);
    }
    apply_virtual_instruction_result(&mut registers, 0, Some(0xffff));
    if registers[0] != 0 {
        return Err(ValidationError::CsrDecoder);
    }
    apply_virtual_instruction_result(&mut registers, destination, Some(0x1234));
    if registers[7] != 0x1234 {
        return Err(ValidationError::CsrDecoder);
    }
    let mut program_counter = u64::MAX - 1;
    advance_program_counter_value(&mut program_counter);
    if program_counter != 2 {
        return Err(ValidationError::CsrDecoder);
    }

    registers[11] = 0xfeed;
    apply_supervisor_call_result(&mut registers, SupervisorCallResult::Legacy { value: 0x12 });
    if registers[10] != 0x12 || registers[11] != 0xfeed {
        return Err(ValidationError::SupervisorCallCompletion);
    }
    apply_supervisor_call_result(
        &mut registers,
        SupervisorCallResult::Modern {
            error: u64::MAX - 1,
            value: 0x34,
        },
    );
    if registers[10] != u64::MAX - 1 || registers[11] != 0x34 {
        return Err(ValidationError::SupervisorCallCompletion);
    }
    let legacy_console = SupervisorCall {
        extension: 1,
        function: 0,
        arguments: [0; 6],
    };
    let modern_timer = SupervisorCall {
        extension: SBI_EXT_TIME,
        function: 0,
        arguments: [0; 6],
    };
    if !supervisor_completion_matches(legacy_console, SupervisorCallResult::Legacy { value: 0 })
        || supervisor_completion_matches(
            legacy_console,
            SupervisorCallResult::Modern { error: 0, value: 0 },
        )
        || supervisor_timer_matches(legacy_console, SupervisorCallResult::Legacy { value: 0 })
        || !supervisor_timer_matches(
            modern_timer,
            SupervisorCallResult::Modern { error: 0, value: 0 },
        )
        || supervisor_completion_matches(
            modern_timer,
            SupervisorCallResult::Modern { error: 0, value: 0 },
        )
        || modern_extension_available(0x0073_5049)
        || modern_extension_available(0x5246_4e43)
        || !modern_extension_available(SBI_EXT_TIME)
    {
        return Err(ValidationError::SupervisorCallCompletion);
    }
    Ok(())
}
