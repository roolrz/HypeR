// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! `AArch64` guest synchronous-trap and virtual system-register handling.

use core::arch::asm;
use hyper::vm::exit::{
    AccessWidth, GuestMemoryFault, GuestPhysicalAddress, MemoryAccess, MmioAccess, MmioAction,
    MmioOperation,
};

use super::registers::{self, SystemRegisterEncoding as Encoding};
use super::{VcpuContext, VmInterruptController};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[must_use]
pub(crate) enum GuestSyncAction {
    /// Consume one trapped `AArch64` instruction.
    Advance,
    /// Write one guest register and optionally consume the instruction.
    WriteRegister {
        register: u8,
        value: u64,
        advance: bool,
    },
    /// Enter the guest's Undefined Instruction vector.
    InjectUndefined {
        program_counter: u64,
        processor_state: u64,
    },
    /// The exit cannot safely return to the guest.
    Stop,
}

/// Owned `AArch64` synchronous-exit facts consumed by active-vCPU emulation.
///
/// Register-file references and raw exception frames remain in exception
/// entry. This value is fixed-width and may cross the architecture-to-policy
/// callback without extending a stack-frame borrow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GuestSyncExit {
    SystemRegister(SystemRegisterExit),
    HypervisorCall { function: u64, argument: u64 },
    SecureMonitorCall,
    Wait,
    Undefined(UndefinedExit),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SystemRegisterExit {
    encoding: Encoding,
    target: u8,
    direction: Direction,
    value: u64,
    undefined: UndefinedExit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct UndefinedExit {
    program_counter: u64,
    processor_state: u64,
    syndrome: u64,
}

const _: () = {
    // Entry copies only the operands required by policy. Keep accidental raw
    // register-file snapshots from silently entering this hot-path contract.
    assert!(core::mem::size_of::<GuestSyncExit>() <= 64);
    assert!(core::mem::size_of::<GuestSyncAction>() <= 32);
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationError {
    InvalidCompletion,
    InvalidDecoder,
    InvalidTopology,
    UnsafeFeatureExposure,
}

pub fn validate() -> Result<(), ValidationError> {
    let syndrome = (registers::ESR_EC_SYSTEM_REGISTER << registers::ESR_EC_SHIFT)
        | registers::ESR_IL
        | (3 << registers::ESR_SYSREG_OP0_SHIFT)
        | (5 << registers::ESR_SYSREG_OP2_SHIFT)
        | (17 << registers::ESR_SYSREG_RT_SHIFT)
        | registers::ESR_SYSREG_DIRECTION_READ;
    let access = decode_access(syndrome);
    if access.encoding != registers::SYSREG_MPIDR_EL1
        || access.target != 17
        || access.direction != Direction::Read
    {
        return Err(ValidationError::InvalidDecoder);
    }
    if virtual_mpidr(0x1234_5678) != 0x0000_0012_0034_5678 {
        return Err(ValidationError::InvalidTopology);
    }
    if sanitize_pfr0(u64::MAX) != registers::ID_AA64PFR0_GUEST_BASE {
        return Err(ValidationError::UnsafeFeatureExposure);
    }
    if sanitize_mmfr1(u64::MAX) & registers::ID_AA64MMFR1_VH_FIELD_MASK != 0 {
        return Err(ValidationError::UnsafeFeatureExposure);
    }
    if !validate_guest_memory_fault_decoder() {
        return Err(ValidationError::InvalidDecoder);
    }
    if !validate_owned_exit_completion() {
        return Err(ValidationError::InvalidCompletion);
    }
    Ok(())
}

fn validate_owned_exit_completion() -> bool {
    let mut general = [0u64; 31];
    general[3] = 0x12cd;
    let write_syndrome = (registers::ESR_EC_DATA_ABORT_LOWER << registers::ESR_EC_SHIFT)
        | registers::ESR_DATA_ABORT_ISV
        | registers::ESR_DATA_ABORT_WNR
        | (3 << registers::ESR_DATA_ABORT_SRT_SHIFT);
    let Some((write, write_completion)) =
        decode_guest_mmio_access(write_syndrome, 0x9000_1000, &general)
    else {
        return false;
    };
    if write.width() != AccessWidth::Byte || write.operation() != MmioOperation::Write(0xcd) {
        return false;
    }
    let mut mismatch_pc = 0x8000;
    if write_completion.apply(&mut general, &mut mismatch_pc, MmioAction::CompleteRead(1))
        || mismatch_pc != 0x8000
    {
        return false;
    }
    let mut write_pc = 0x8000;
    if !write_completion.apply(&mut general, &mut write_pc, MmioAction::CompleteWrite)
        || write_pc != 0x8004
        || general[3] != 0x12cd
    {
        return false;
    }

    let read_syndrome = (registers::ESR_EC_DATA_ABORT_LOWER << registers::ESR_EC_SHIFT)
        | registers::ESR_DATA_ABORT_ISV
        | registers::ESR_DATA_ABORT_SSE
        | (4 << registers::ESR_DATA_ABORT_SRT_SHIFT);
    let Some((read, read_completion)) =
        decode_guest_mmio_access(read_syndrome, 0x9000_2000, &general)
    else {
        return false;
    };
    let mut read_pc = 0x8100;
    if read.operation() != MmioOperation::Read
        || !read_completion.apply(&mut general, &mut read_pc, MmioAction::CompleteRead(0x80))
        || read_pc != 0x8104
        || general[4] != 0xffff_ff80
    {
        return false;
    }

    general[0] = registers::SMCCC_VERSION;
    general[1] = 0xfeed;
    let hvc_syndrome = registers::ESR_EC_HVC64 << registers::ESR_EC_SHIFT;
    let Some(GuestSyncExit::HypervisorCall { function, argument }) = decode_guest_sync(
        hvc_syndrome,
        &general,
        0x8200,
        registers::SPSR_EL1H_AND_DAIF,
    ) else {
        return false;
    };
    let hvc_exit = GuestSyncExit::HypervisorCall { function, argument };
    let mut mismatch_pc = 0x8200;
    let mut mismatch_pstate = registers::SPSR_EL1H_AND_DAIF;
    if apply_guest_sync_action(
        hvc_exit,
        &mut general,
        &mut mismatch_pc,
        &mut mismatch_pstate,
        GuestSyncAction::WriteRegister {
            register: 1,
            value: 0,
            advance: false,
        },
    ) || mismatch_pc != 0x8200
    {
        return false;
    }
    let mut hvc_pc = 0x8200;
    let mut hvc_pstate = registers::SPSR_EL1H_AND_DAIF;
    apply_guest_sync_action(
        hvc_exit,
        &mut general,
        &mut hvc_pc,
        &mut hvc_pstate,
        emulate_hypercall(function, argument),
    ) && general[0] == registers::SMCCC_VERSION_1_1
        && hvc_pc == 0x8200
        && hvc_pstate == registers::SPSR_EL1H_AND_DAIF
}

fn validate_guest_memory_fault_decoder() -> bool {
    let write_syndrome = (registers::ESR_EC_DATA_ABORT_LOWER << registers::ESR_EC_SHIFT)
        | registers::ESR_ABORT_TRANSLATION_FAULT_LEVEL3
        | registers::ESR_DATA_ABORT_WNR
        | registers::ESR_DATA_ABORT_S1PTW;
    let write_valid = decode_guest_memory_fault(write_syndrome, 0x4321_0abc)
        == Some(GuestMemoryFault::new(
            GuestPhysicalAddress::new(0x4321_0abc),
            MemoryAccess::Write,
            true,
        ));
    let execute_syndrome = (registers::ESR_EC_INSTRUCTION_ABORT_LOWER << registers::ESR_EC_SHIFT)
        | registers::ESR_ABORT_TRANSLATION_FAULT_LEVEL0;
    let execute_valid = decode_guest_memory_fault(execute_syndrome, 0x8000_1000)
        == Some(GuestMemoryFault::new(
            GuestPhysicalAddress::new(0x8000_1000),
            MemoryAccess::Execute,
            false,
        ));
    let read_syndrome = (registers::ESR_EC_DATA_ABORT_LOWER << registers::ESR_EC_SHIFT)
        | registers::ESR_ABORT_TRANSLATION_FAULT_LEVEL0;
    let read_valid = decode_guest_memory_fault(read_syndrome, 0x9000_2000)
        == Some(GuestMemoryFault::new(
            GuestPhysicalAddress::new(0x9000_2000),
            MemoryAccess::Read,
            false,
        ));
    let unrelated_rejected = decode_guest_memory_fault(
        registers::ESR_EC_SYSTEM_REGISTER << registers::ESR_EC_SHIFT,
        0,
    )
    .is_none();
    let non_translation_rejected = decode_guest_memory_fault(
        registers::ESR_EC_DATA_ABORT_LOWER << registers::ESR_EC_SHIFT,
        0,
    )
    .is_none();
    write_valid && execute_valid && read_valid && unrelated_rejected && non_translation_rejected
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GuestMmioCompletion {
    operation: MmioOperation,
    target: u8,
    width: AccessWidth,
    sign_extend: bool,
    register_64_bit: bool,
}

impl GuestMmioCompletion {
    pub(crate) fn apply(
        self,
        general: &mut [u64; 31],
        program_counter: &mut u64,
        action: MmioAction,
    ) -> bool {
        let value = match (self.operation, action) {
            (MmioOperation::Read, MmioAction::CompleteRead(value)) => Some(value),
            (MmioOperation::Write(_), MmioAction::CompleteWrite) => None,
            (_, MmioAction::Unhandled | MmioAction::Stop)
            | (MmioOperation::Read, MmioAction::CompleteWrite)
            | (MmioOperation::Write(_), MmioAction::CompleteRead(_)) => return false,
        };
        if let Some(mut value) = value {
            let bits = self.width.bytes() * 8;
            if bits < u64::BITS as usize {
                value &= (1u64 << bits) - 1;
            }
            if self.sign_extend {
                let shift = 64 - bits;
                value = ((value << shift) as i64 >> shift) as u64;
            }
            if !self.register_64_bit {
                value &= u64::from(u32::MAX);
            }
            write_general(general, self.target, value);
        }
        advance(program_counter);
        true
    }
}

fn read_general(general: &[u64; 31], index: u8) -> u64 {
    match general.get(index as usize) {
        Some(value) => *value,
        None => 0,
    }
}

pub(crate) fn write_general(general: &mut [u64; 31], index: u8, value: u64) {
    if let Some(target) = general.get_mut(index as usize) {
        *target = value;
    }
}

pub(crate) fn advance(program_counter: &mut u64) {
    *program_counter = program_counter.wrapping_add(registers::AARCH64_INSTRUCTION_SIZE);
}

pub(crate) fn apply_guest_sync_action(
    exit: GuestSyncExit,
    general: &mut [u64; 31],
    program_counter: &mut u64,
    processor_state: &mut u64,
    action: GuestSyncAction,
) -> bool {
    if !guest_sync_action_matches(exit, action) {
        return false;
    }
    match action {
        GuestSyncAction::Advance => {
            advance(program_counter);
            true
        }
        GuestSyncAction::WriteRegister {
            register,
            value,
            advance: should_advance,
        } => {
            write_general(general, register, value);
            if should_advance {
                advance(program_counter);
            }
            true
        }
        GuestSyncAction::InjectUndefined {
            program_counter: next_program_counter,
            processor_state: next_processor_state,
        } => {
            *program_counter = next_program_counter;
            *processor_state = next_processor_state;
            true
        }
        GuestSyncAction::Stop => false,
    }
}

fn guest_sync_action_matches(exit: GuestSyncExit, action: GuestSyncAction) -> bool {
    if action == GuestSyncAction::Stop {
        return true;
    }
    match (exit, action) {
        (
            GuestSyncExit::SystemRegister(SystemRegisterExit {
                target,
                direction: Direction::Read,
                ..
            }),
            GuestSyncAction::WriteRegister {
                register,
                advance: true,
                ..
            },
        ) => register == target,
        (
            GuestSyncExit::SystemRegister(SystemRegisterExit {
                direction: Direction::Write,
                ..
            }),
            GuestSyncAction::Advance | GuestSyncAction::InjectUndefined { .. },
        )
        | (
            GuestSyncExit::SystemRegister(SystemRegisterExit {
                direction: Direction::Read,
                ..
            }),
            GuestSyncAction::InjectUndefined { .. },
        )
        | (GuestSyncExit::Wait, GuestSyncAction::Advance)
        | (GuestSyncExit::Undefined(_), GuestSyncAction::InjectUndefined { .. }) => true,
        (
            GuestSyncExit::HypervisorCall { .. } | GuestSyncExit::SecureMonitorCall,
            GuestSyncAction::WriteRegister {
                register: 0,
                advance: false,
                ..
            },
        ) => true,
        _ => false,
    }
}

pub(crate) fn decode_guest_mmio_access(
    syndrome: u64,
    physical_address: u64,
    general: &[u64; 31],
) -> Option<(MmioAccess, GuestMmioCompletion)> {
    if (syndrome >> registers::ESR_EC_SHIFT) & registers::ESR_EC_MASK
        != registers::ESR_EC_DATA_ABORT_LOWER
        || syndrome & registers::ESR_DATA_ABORT_ISV == 0
        || syndrome & registers::ESR_DATA_ABORT_S1PTW != 0
    {
        return None;
    }
    let bytes = 1usize
        << ((syndrome >> registers::ESR_DATA_ABORT_SAS_SHIFT) & registers::ESR_DATA_ABORT_SAS_MASK);
    let width = AccessWidth::from_bytes(bytes)?;
    let target = ((syndrome >> registers::ESR_DATA_ABORT_SRT_SHIFT)
        & registers::ESR_DATA_ABORT_SRT_MASK) as u8;
    let mask = if bytes == 8 {
        u64::MAX
    } else {
        (1u64 << (bytes * 8)) - 1
    };
    let operation = if syndrome & registers::ESR_DATA_ABORT_WNR != 0 {
        MmioOperation::Write(read_general(general, target) & mask)
    } else {
        MmioOperation::Read
    };
    let access = MmioAccess::new(
        GuestPhysicalAddress::new(physical_address),
        width,
        operation,
    );
    Some((
        access,
        GuestMmioCompletion {
            operation,
            target,
            width,
            sign_extend: syndrome & registers::ESR_DATA_ABORT_SSE != 0,
            register_64_bit: syndrome & registers::ESR_DATA_ABORT_SF != 0,
        },
    ))
}

/// Decodes an owned stage-2 translation-fault event from an `AArch64` syndrome.
///
/// The returned value carries no reference to the exception frame. Raw ESR and
/// HPFAR encodings remain private to the `AArch64` backend.
pub(crate) fn decode_guest_memory_fault(
    syndrome: u64,
    physical_address: u64,
) -> Option<GuestMemoryFault> {
    let exception_class = (syndrome >> registers::ESR_EC_SHIFT) & registers::ESR_EC_MASK;
    if !matches!(
        exception_class,
        registers::ESR_EC_INSTRUCTION_ABORT_LOWER | registers::ESR_EC_DATA_ABORT_LOWER
    ) {
        return None;
    }
    let fault_status = syndrome & registers::ESR_ABORT_FSC_MASK;
    if !(registers::ESR_ABORT_TRANSLATION_FAULT_LEVEL0
        ..=registers::ESR_ABORT_TRANSLATION_FAULT_LEVEL3)
        .contains(&fault_status)
    {
        return None;
    }
    let access = if exception_class == registers::ESR_EC_INSTRUCTION_ABORT_LOWER {
        MemoryAccess::Execute
    } else if syndrome & registers::ESR_DATA_ABORT_WNR != 0 {
        MemoryAccess::Write
    } else {
        MemoryAccess::Read
    };
    Some(GuestMemoryFault::new(
        GuestPhysicalAddress::new(physical_address),
        access,
        exception_class == registers::ESR_EC_DATA_ABORT_LOWER
            && syndrome & registers::ESR_DATA_ABORT_S1PTW != 0,
    ))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Direction {
    Write,
    Read,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Access {
    encoding: Encoding,
    target: u8,
    direction: Direction,
}

pub(crate) fn decode_guest_sync(
    syndrome: u64,
    general: &[u64; 31],
    program_counter: u64,
    processor_state: u64,
) -> Option<GuestSyncExit> {
    let undefined = UndefinedExit {
        program_counter,
        processor_state,
        syndrome,
    };
    Some(
        match (syndrome >> registers::ESR_EC_SHIFT) & registers::ESR_EC_MASK {
            registers::ESR_EC_SYSTEM_REGISTER => {
                let access = decode_access(syndrome);
                GuestSyncExit::SystemRegister(SystemRegisterExit {
                    encoding: access.encoding,
                    target: access.target,
                    direction: access.direction,
                    value: match access.direction {
                        Direction::Write => read_general(general, access.target),
                        Direction::Read => 0,
                    },
                    undefined,
                })
            }
            registers::ESR_EC_HVC64 => GuestSyncExit::HypervisorCall {
                function: read_general(general, 0),
                argument: read_general(general, 1),
            },
            registers::ESR_EC_SMC64 => GuestSyncExit::SecureMonitorCall,
            registers::ESR_EC_WFX => GuestSyncExit::Wait,
            // Abort classes are completed only through memory or MMIO policy.
            registers::ESR_EC_INSTRUCTION_ABORT_LOWER | registers::ESR_EC_DATA_ABORT_LOWER => {
                return None;
            }
            _ => GuestSyncExit::Undefined(undefined),
        },
    )
}

pub(crate) fn handle_guest_sync(
    context: &mut VcpuContext,
    vcpu_id: u32,
    interrupts: &VmInterruptController,
    exit: GuestSyncExit,
) -> GuestSyncAction {
    match exit {
        GuestSyncExit::SystemRegister(exit) => {
            emulate_system_register(context, vcpu_id, interrupts, exit)
        }
        GuestSyncExit::HypervisorCall { function, argument } => {
            emulate_hypercall(function, argument)
        }
        GuestSyncExit::SecureMonitorCall => GuestSyncAction::WriteRegister {
            register: 0,
            value: registers::SMCCC_NOT_SUPPORTED,
            advance: false,
        },
        GuestSyncExit::Wait => {
            // A scheduler-aware WFI exit can replace this completion once vCPU
            // Threads acquire a device-interrupt wakeup wait queue.
            GuestSyncAction::Advance
        }
        GuestSyncExit::Undefined(exit) => inject_undefined(context, exit),
    }
}

fn emulate_hypercall(function: u64, argument: u64) -> GuestSyncAction {
    let result = match function {
        registers::SMCCC_VERSION => registers::SMCCC_VERSION_1_1,
        registers::SMCCC_ARCH_FEATURES => registers::SMCCC_NOT_SUPPORTED,
        registers::PSCI_VERSION => registers::PSCI_VERSION_1_0,
        registers::PSCI_MIGRATE_INFO_TYPE => registers::PSCI_TOS_NOT_PRESENT,
        registers::PSCI_FEATURES => match argument {
            registers::PSCI_VERSION
            | registers::PSCI_FEATURES
            | registers::PSCI_MIGRATE_INFO_TYPE => 0,
            _ => registers::SMCCC_NOT_SUPPORTED,
        },
        _ => registers::SMCCC_NOT_SUPPORTED,
    };
    // HVC and SMC save the architectural return address in ELR_EL2. Unlike
    // trapped instructions such as WFx and system-register accesses, the
    // exception-generating instruction has already been consumed.
    GuestSyncAction::WriteRegister {
        register: 0,
        value: result,
        advance: false,
    }
}

fn emulate_system_register(
    context: &mut VcpuContext,
    vcpu_id: u32,
    interrupts: &VmInterruptController,
    exit: SystemRegisterExit,
) -> GuestSyncAction {
    match exit.direction {
        Direction::Read => match read_virtual_register(context, vcpu_id, exit.encoding) {
            Some(value) => GuestSyncAction::WriteRegister {
                register: exit.target,
                value,
                advance: true,
            },
            None => inject_undefined(context, exit.undefined),
        },
        Direction::Write => {
            if exit.encoding == registers::SYSREG_ICC_SGI1R_EL1 {
                return match super::vm_vcpu::deliver_software_interrupt(
                    context, vcpu_id, interrupts, exit.value,
                ) {
                    Ok(()) => GuestSyncAction::Advance,
                    Err(_) => GuestSyncAction::Stop,
                };
            }
            if write_virtual_register(exit.encoding, exit.value) {
                GuestSyncAction::Advance
            } else {
                inject_undefined(context, exit.undefined)
            }
        }
    }
}

fn decode_access(esr: u64) -> Access {
    Access {
        encoding: Encoding::from_esr(esr),
        target: ((esr >> registers::ESR_SYSREG_RT_SHIFT) & registers::ESR_SYSREG_RT_MASK) as u8,
        direction: if esr & registers::ESR_SYSREG_DIRECTION_READ == 0 {
            Direction::Write
        } else {
            Direction::Read
        },
    }
}

fn read_virtual_register(_context: &VcpuContext, vcpu_id: u32, encoding: Encoding) -> Option<u64> {
    match encoding {
        registers::SYSREG_MIDR_EL1 => Some(read_midr_el1()),
        registers::SYSREG_MPIDR_EL1 => Some(virtual_mpidr(vcpu_id)),
        registers::SYSREG_REVIDR_EL1 => Some(read_revidr_el1()),
        registers::SYSREG_ID_AA64PFR0_EL1 => Some(sanitize_pfr0(read_id_aa64pfr0_el1())),
        registers::SYSREG_ID_AA64PFR1_EL1
        | registers::SYSREG_ID_AA64PFR2_EL1
        | registers::SYSREG_ID_AA64FPFR0_EL1
        | registers::SYSREG_ID_AA64DFR1_EL1
        | registers::SYSREG_ID_AA64AFR0_EL1
        | registers::SYSREG_ID_AA64AFR1_EL1
        | registers::SYSREG_ID_AA64ISAR3_EL1
        | registers::SYSREG_ID_AA64MMFR3_EL1
        | registers::SYSREG_ID_AA64MMFR4_EL1
        | registers::SYSREG_ID_AA64ZFR0_EL1
        | registers::SYSREG_ID_AA64SMFR0_EL1 => Some(0),
        registers::SYSREG_ID_AA64DFR0_EL1 => Some(registers::ID_AA64DFR0_GUEST_BASE),
        registers::SYSREG_ID_AA64ISAR0_EL1 => Some(sanitize_isar0(read_id_aa64isar0_el1())),
        registers::SYSREG_ID_AA64ISAR1_EL1 => Some(sanitize_isar1(read_id_aa64isar1_el1())),
        registers::SYSREG_ID_AA64ISAR2_EL1 => Some(sanitize_isar2(read_id_aa64isar2_el1())),
        registers::SYSREG_ID_AA64MMFR0_EL1 => Some(read_id_aa64mmfr0_el1()),
        registers::SYSREG_ID_AA64MMFR1_EL1 => Some(sanitize_mmfr1(read_id_aa64mmfr1_el1())),
        registers::SYSREG_ID_AA64MMFR2_EL1 => Some(read_id_aa64mmfr2_el1()),
        registers::SYSREG_CTR_EL0 => Some(read_ctr_el0()),
        registers::SYSREG_DCZID_EL0 => Some(read_dczid_el0()),
        registers::SYSREG_CNTFRQ_EL0 => Some(read_cntfrq_el0()),
        registers::SYSREG_CNTPCT_EL0 => Some(read_cntvct_el0()),
        registers::SYSREG_ACTLR_EL1 => Some(0),
        _ => None,
    }
}

fn write_virtual_register(encoding: Encoding, _value: u64) -> bool {
    // ACTLR_EL1 is architecturally implementation-defined. RAZ/WI prevents a
    // guest from depending on host-specific auxiliary controls.
    encoding == registers::SYSREG_ACTLR_EL1
}

fn inject_undefined(context: &mut VcpuContext, exit: UndefinedExit) -> GuestSyncAction {
    let syndrome = exit.syndrome & registers::ESR_IL;
    let vector_offset = match exit.processor_state & registers::SPSR_M_MASK {
        registers::SPSR_EL0T => registers::VECTOR_LOWER_EL_AARCH64,
        registers::SPSR_EL1T => registers::VECTOR_CURRENT_EL_SP0,
        _ => registers::VECTOR_CURRENT_EL_SPX,
    };

    context.esr_el1 = syndrome;
    context.far_el1 = 0;
    context.elr_el1 = exit.program_counter;
    context.spsr_el1 = exit.processor_state;
    // SAFETY: The active-vCPU bridge guarantees this is the guest whose EL1
    // bank is live on the current CPU.
    let live_vbar =
        unsafe { load_undefined_exception(syndrome, exit.program_counter, exit.processor_state) };
    context.vbar_el1 = live_vbar;
    GuestSyncAction::InjectUndefined {
        program_counter: live_vbar.wrapping_add(vector_offset),
        processor_state: (exit.processor_state & !registers::SPSR_MODE_AND_DAIF_MASK)
            | registers::SPSR_EL1H_AND_DAIF,
    }
}

unsafe fn load_undefined_exception(syndrome: u64, elr: u64, spsr: u64) -> u64 {
    if super::host::is_vhe() {
        // SAFETY: The caller guarantees the active guest owns the live EL12
        // exception register bank.
        unsafe { load_undefined_exception_vhe(syndrome, elr, spsr) }
    } else {
        // SAFETY: The caller guarantees the active guest owns the live EL1
        // exception register bank.
        unsafe { load_undefined_exception_nvhe(syndrome, elr, spsr) }
    }
}

unsafe fn load_undefined_exception_nvhe(syndrome: u64, elr: u64, spsr: u64) -> u64 {
    let vbar: u64;
    // SAFETY: Guest execution is stopped and the caller guarantees the live
    // nVHE EL1 exception bank belongs to the active vCPU.
    unsafe {
        asm!(
            "mrs {vbar}, VBAR_EL1",
            "msr ESR_EL1, {esr}",
            "msr FAR_EL1, xzr",
            "msr ELR_EL1, {elr}",
            "msr SPSR_EL1, {spsr}",
            vbar = out(reg) vbar,
            esr = in(reg) syndrome,
            elr = in(reg) elr,
            spsr = in(reg) spsr,
            options(nostack, preserves_flags)
        );
    }
    vbar
}

unsafe fn load_undefined_exception_vhe(syndrome: u64, elr: u64, spsr: u64) -> u64 {
    let vbar: u64;
    // SAFETY: Guest execution is stopped and the caller guarantees the live
    // EL12 exception bank belongs to the active vCPU.
    unsafe {
        asm!(
            "mrs {vbar}, S3_5_C12_C0_0",
            "msr S3_5_C5_C2_0, {esr}",
            "msr S3_5_C6_C0_0, xzr",
            "msr S3_5_C4_C0_1, {elr}",
            "msr S3_5_C4_C0_0, {spsr}",
            vbar = out(reg) vbar,
            esr = in(reg) syndrome,
            elr = in(reg) elr,
            spsr = in(reg) spsr,
            options(nostack, preserves_flags)
        );
    }
    vbar
}

const fn virtual_mpidr(vcpu_id: u32) -> u64 {
    let id = vcpu_id as u64;
    (id & registers::MPIDR_AFF0_TO_2_MASK)
        | ((id & registers::MPIDR_LINEAR_AFF3_MASK) << registers::MPIDR_AFF3_FROM_LINEAR_ID_SHIFT)
}

fn sanitize_pfr0(_value: u64) -> u64 {
    // Present a deliberately small, internally coherent CPU contract:
    // AArch64 EL0 and EL1 plus the base FP/Advanced-SIMD implementation.
    // Optional fields use feature-specific absence encodings, so copying a
    // common all-ones mask across them would incorrectly advertise SVE.
    registers::ID_AA64PFR0_GUEST_BASE
}

fn sanitize_isar1(value: u64) -> u64 {
    value & !registers::ID_AA64ISAR1_POINTER_AUTH_MASK
}

fn sanitize_isar0(value: u64) -> u64 {
    // Transactional Memory state is not part of the vCPU context contract.
    value & !registers::ID_AA64ISAR0_TME_MASK
}

fn sanitize_isar2(_value: u64) -> u64 {
    // Do not advertise newer pointer-authentication algorithms until their
    // key registers are part of the vCPU context-switch contract. ISAR2 only
    // reports optional extensions, so zero is a conservative coherent model.
    0
}

const fn sanitize_mmfr1(value: u64) -> u64 {
    // Nested virtualization is not part of the guest CPU contract. VHE is an
    // EL2 implementation feature and must not be advertised to an EL1 guest.
    value & !registers::ID_AA64MMFR1_VH_FIELD_MASK
}

macro_rules! read_register {
    ($function:ident, $register:literal) => {
        fn $function() -> u64 {
            let value: u64;
            // SAFETY: The named register is readable at EL2 and the operation
            // has no memory side effects.
            unsafe {
                asm!(
                    concat!("mrs {value}, ", $register),
                    value = out(reg) value,
                    options(nomem, nostack, preserves_flags)
                );
            }
            value
        }
    };
}

read_register!(read_midr_el1, "MIDR_EL1");
read_register!(read_revidr_el1, "REVIDR_EL1");
read_register!(read_id_aa64pfr0_el1, "ID_AA64PFR0_EL1");
read_register!(read_id_aa64isar0_el1, "ID_AA64ISAR0_EL1");
read_register!(read_id_aa64isar1_el1, "ID_AA64ISAR1_EL1");
read_register!(read_id_aa64isar2_el1, "ID_AA64ISAR2_EL1");
read_register!(read_id_aa64mmfr0_el1, "ID_AA64MMFR0_EL1");
read_register!(read_id_aa64mmfr1_el1, "ID_AA64MMFR1_EL1");
read_register!(read_id_aa64mmfr2_el1, "ID_AA64MMFR2_EL1");
read_register!(read_ctr_el0, "CTR_EL0");
read_register!(read_dczid_el0, "DCZID_EL0");
read_register!(read_cntfrq_el0, "CNTFRQ_EL0");
read_register!(read_cntvct_el0, "CNTVCT_EL0");
