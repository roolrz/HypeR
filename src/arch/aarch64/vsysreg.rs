// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! `AArch64` guest synchronous-trap and virtual system-register handling.

use core::arch::asm;
use hyper::vm::exit::{
    GuestMemoryFault, GuestPhysicalAddress, MemoryAccess, MmioAccess, MmioOperation,
};

use super::VcpuContext;
use super::registers::{self, SystemRegisterEncoding as Encoding};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GuestSyncAction {
    Resume,
    Injected,
    SoftwareInterrupt(u64),
    Unhandled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationError {
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
    Ok(())
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

pub(crate) struct GuestSyncFrame<'a> {
    general: &'a mut [u64; 31],
    program_counter: &'a mut u64,
    processor_state: &'a mut u64,
    syndrome: u64,
    physical_address: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GuestDataAccess {
    pub address: u64,
    pub size: usize,
    pub write: bool,
    pub value: u64,
    target: u8,
    sign_extend: bool,
    register_64_bit: bool,
}

impl<'a> GuestSyncFrame<'a> {
    pub(crate) fn new(
        general: &'a mut [u64; 31],
        program_counter: &'a mut u64,
        processor_state: &'a mut u64,
        syndrome: u64,
        physical_address: u64,
    ) -> Self {
        Self {
            general,
            program_counter,
            processor_state,
            syndrome,
            physical_address,
        }
    }

    fn read_general(&self, index: u8) -> u64 {
        match self.general.get(index as usize) {
            Some(value) => *value,
            None => 0,
        }
    }

    fn write_general(&mut self, index: u8, value: u64) {
        if let Some(target) = self.general.get_mut(index as usize) {
            *target = value;
        }
    }

    fn advance(&mut self) {
        *self.program_counter = self
            .program_counter
            .wrapping_add(registers::AARCH64_INSTRUCTION_SIZE);
    }

    pub(crate) fn data_access(&self) -> Option<GuestDataAccess> {
        if (self.syndrome >> registers::ESR_EC_SHIFT) & registers::ESR_EC_MASK
            != registers::ESR_EC_DATA_ABORT_LOWER
            || self.syndrome & registers::ESR_DATA_ABORT_ISV == 0
        {
            return None;
        }
        let size = 1usize
            << ((self.syndrome >> registers::ESR_DATA_ABORT_SAS_SHIFT)
                & registers::ESR_DATA_ABORT_SAS_MASK);
        let target = ((self.syndrome >> registers::ESR_DATA_ABORT_SRT_SHIFT)
            & registers::ESR_DATA_ABORT_SRT_MASK) as u8;
        let mask = if size == 8 {
            u64::MAX
        } else {
            (1u64 << (size * 8)) - 1
        };
        Some(GuestDataAccess {
            address: self.physical_address,
            size,
            write: self.syndrome & registers::ESR_DATA_ABORT_WNR != 0,
            value: self.read_general(target) & mask,
            target,
            sign_extend: self.syndrome & registers::ESR_DATA_ABORT_SSE != 0,
            register_64_bit: self.syndrome & registers::ESR_DATA_ABORT_SF != 0,
        })
    }

    pub(crate) fn mmio_access(&self) -> Option<MmioAccess> {
        let access = self.data_access()?;
        let operation = if access.write {
            MmioOperation::Write(access.value)
        } else {
            MmioOperation::Read
        };
        Some(MmioAccess::new(
            GuestPhysicalAddress::new(access.address),
            access.size,
            operation,
        ))
    }

    pub(crate) fn guest_memory_fault(&self) -> Option<GuestMemoryFault> {
        decode_guest_memory_fault(self.syndrome, self.physical_address)
    }

    pub(crate) fn complete_data_access(&mut self, access: GuestDataAccess, value: Option<u64>) {
        if let Some(mut value) = value {
            if access.sign_extend {
                let shift = 64 - access.size * 8;
                value = ((value << shift) as i64 >> shift) as u64;
            }
            if !access.register_64_bit {
                value &= u64::from(u32::MAX);
            }
            self.write_general(access.target, value);
        }
        self.advance();
    }

    pub(crate) fn complete_mmio_access(&mut self, access: MmioAccess, value: Option<u64>) -> bool {
        let Some(decoded) = self.data_access() else {
            return false;
        };
        let operation = if decoded.write {
            MmioOperation::Write(decoded.value)
        } else {
            MmioOperation::Read
        };
        if access
            != MmioAccess::new(
                GuestPhysicalAddress::new(decoded.address),
                decoded.size,
                operation,
            )
        {
            return false;
        }
        self.complete_data_access(decoded, value);
        true
    }
}

pub(crate) fn decode_guest_mmio_access(frame: &GuestSyncFrame<'_>) -> Option<MmioAccess> {
    frame.mmio_access()
}

pub(crate) fn complete_guest_mmio_access(
    frame: &mut GuestSyncFrame<'_>,
    access: MmioAccess,
    value: Option<u64>,
) -> bool {
    frame.complete_mmio_access(access, value)
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

pub(crate) fn handle_guest_sync(
    context: &mut VcpuContext,
    vcpu_id: u32,
    frame: &mut GuestSyncFrame<'_>,
) -> GuestSyncAction {
    match (frame.syndrome >> registers::ESR_EC_SHIFT) & registers::ESR_EC_MASK {
        registers::ESR_EC_SYSTEM_REGISTER => emulate_system_register(context, vcpu_id, frame),
        registers::ESR_EC_HVC64 => emulate_hypercall(frame),
        registers::ESR_EC_SMC64 => unsupported_call(frame),
        registers::ESR_EC_WFX => {
            // A scheduler-aware WFI exit can replace this resume policy once
            // vCPU threads gain a blocked state and interrupt wakeup queue.
            frame.advance();
            GuestSyncAction::Resume
        }
        // Lower-EL aborts require stage-2 fault resolution and must not be
        // mistaken for guest Undefined Instruction exceptions.
        registers::ESR_EC_INSTRUCTION_ABORT_LOWER | registers::ESR_EC_DATA_ABORT_LOWER => {
            GuestSyncAction::Unhandled
        }
        _ => inject_undefined(context, frame),
    }
}

fn emulate_hypercall(frame: &mut GuestSyncFrame<'_>) -> GuestSyncAction {
    let function = frame.read_general(0);
    let result = match function {
        registers::SMCCC_VERSION => registers::SMCCC_VERSION_1_1,
        registers::SMCCC_ARCH_FEATURES => registers::SMCCC_NOT_SUPPORTED,
        registers::PSCI_VERSION => registers::PSCI_VERSION_1_0,
        registers::PSCI_MIGRATE_INFO_TYPE => registers::PSCI_TOS_NOT_PRESENT,
        registers::PSCI_FEATURES => match frame.read_general(1) {
            registers::PSCI_VERSION
            | registers::PSCI_FEATURES
            | registers::PSCI_MIGRATE_INFO_TYPE => 0,
            _ => registers::SMCCC_NOT_SUPPORTED,
        },
        _ => registers::SMCCC_NOT_SUPPORTED,
    };
    frame.write_general(0, result);
    // HVC and SMC save the architectural return address in ELR_EL2. Unlike
    // trapped instructions such as WFx and system-register accesses, the
    // exception-generating instruction has already been consumed.
    GuestSyncAction::Resume
}

fn unsupported_call(frame: &mut GuestSyncFrame<'_>) -> GuestSyncAction {
    frame.write_general(0, registers::SMCCC_NOT_SUPPORTED);
    GuestSyncAction::Resume
}

fn emulate_system_register(
    context: &mut VcpuContext,
    vcpu_id: u32,
    frame: &mut GuestSyncFrame<'_>,
) -> GuestSyncAction {
    let access = decode_access(frame.syndrome);
    match access.direction {
        Direction::Read => match read_virtual_register(context, vcpu_id, access.encoding) {
            Some(value) => {
                frame.write_general(access.target, value);
                frame.advance();
                GuestSyncAction::Resume
            }
            None => inject_undefined(context, frame),
        },
        Direction::Write => {
            let value = frame.read_general(access.target);
            if access.encoding == registers::SYSREG_ICC_SGI1R_EL1 {
                frame.advance();
                return GuestSyncAction::SoftwareInterrupt(value);
            }
            if write_virtual_register(access.encoding, value) {
                frame.advance();
                GuestSyncAction::Resume
            } else {
                inject_undefined(context, frame)
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

fn inject_undefined(context: &mut VcpuContext, frame: &mut GuestSyncFrame<'_>) -> GuestSyncAction {
    let saved_pc = *frame.program_counter;
    let saved_pstate = *frame.processor_state;
    let syndrome = frame.syndrome & registers::ESR_IL;
    let vector_offset = match saved_pstate & registers::SPSR_M_MASK {
        registers::SPSR_EL0T => registers::VECTOR_LOWER_EL_AARCH64,
        registers::SPSR_EL1T => registers::VECTOR_CURRENT_EL_SP0,
        _ => registers::VECTOR_CURRENT_EL_SPX,
    };

    context.esr_el1 = syndrome;
    context.far_el1 = 0;
    context.elr_el1 = saved_pc;
    context.spsr_el1 = saved_pstate;
    // SAFETY: The active-vCPU bridge guarantees this is the guest whose EL1
    // bank is live on the current CPU.
    let live_vbar = unsafe { load_undefined_exception(syndrome, saved_pc, saved_pstate) };
    context.vbar_el1 = live_vbar;
    *frame.program_counter = live_vbar.wrapping_add(vector_offset);
    *frame.processor_state =
        (saved_pstate & !registers::SPSR_MODE_AND_DAIF_MASK) | registers::SPSR_EL1H_AND_DAIF;
    GuestSyncAction::Injected
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
