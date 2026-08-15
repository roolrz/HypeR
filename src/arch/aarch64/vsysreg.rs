//! AArch64 guest synchronous-trap and virtual system-register handling.

use core::arch::asm;

use super::VcpuContext;

const EC_SHIFT: u64 = 26;
const EC_MASK: u64 = 0x3f;
const EC_WFX: u64 = 0x01;
const EC_HVC64: u64 = 0x16;
const EC_SMC64: u64 = 0x17;
const EC_SYSTEM_REGISTER: u64 = 0x18;
const INSTRUCTION_SIZE: u64 = 4;
const SMCCC_NOT_SUPPORTED: u64 = u64::MAX;
const SMCCC_VERSION: u64 = 0x8000_0000;
const SMCCC_ARCH_FEATURES: u64 = 0x8000_0001;
const PSCI_VERSION: u64 = 0x8400_0000;
const PSCI_MIGRATE_INFO_TYPE: u64 = 0x8400_0006;
const PSCI_FEATURES: u64 = 0x8400_000a;
const PSCI_VERSION_1_0: u64 = 0x0001_0000;
const SMCCC_VERSION_1_1: u64 = 0x0001_0001;
const PSCI_TOS_NOT_PRESENT: u64 = 2;
const SPSR_MODE_MASK: u64 = 0xf;
const SPSR_EL0T: u64 = 0x0;
const SPSR_EL1T: u64 = 0x4;
const SPSR_MODE_AND_DAIF_MASK: u64 = 0x3cf;
const SPSR_EL1H_AND_DAIF: u64 = 0x3c5;
const VECTOR_CURRENT_EL_SP0: u64 = 0x000;
const VECTOR_CURRENT_EL_SPX: u64 = 0x200;
const VECTOR_LOWER_EL_AARCH64: u64 = 0x400;
const ESR_IL: u64 = 1 << 25;

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
    let syndrome =
        (EC_SYSTEM_REGISTER << EC_SHIFT) | ESR_IL | (3 << 20) | (5 << 17) | (17 << 5) | 1;
    let access = decode_access(syndrome);
    if access.encoding != MPIDR_EL1 || access.target != 17 || access.direction != Direction::Read {
        return Err(ValidationError::InvalidDecoder);
    }
    if virtual_mpidr(0x1234_5678) != 0x0000_0012_0034_5678 {
        return Err(ValidationError::InvalidTopology);
    }
    if sanitize_pfr0(u64::MAX) != 0x11 {
        return Err(ValidationError::UnsafeFeatureExposure);
    }
    Ok(())
}

pub(crate) struct GuestSyncFrame<'a> {
    general: &'a mut [u64; 31],
    program_counter: &'a mut u64,
    processor_state: &'a mut u64,
    syndrome: u64,
    fault_address: u64,
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
        fault_address: u64,
        physical_address: u64,
    ) -> Self {
        Self {
            general,
            program_counter,
            processor_state,
            syndrome,
            fault_address,
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
        *self.program_counter = self.program_counter.wrapping_add(INSTRUCTION_SIZE);
    }

    pub(crate) fn data_access(&self) -> Option<GuestDataAccess> {
        if (self.syndrome >> EC_SHIFT) & EC_MASK != 0x24 || self.syndrome & (1 << 24) == 0 {
            return None;
        }
        let size = 1usize << ((self.syndrome >> 22) & 0x3);
        let target = ((self.syndrome >> 16) & 0x1f) as u8;
        let mask = if size == 8 {
            u64::MAX
        } else {
            (1u64 << (size * 8)) - 1
        };
        Some(GuestDataAccess {
            address: self.physical_address,
            size,
            write: self.syndrome & (1 << 6) != 0,
            value: self.read_general(target) & mask,
            target,
            sign_extend: self.syndrome & (1 << 21) != 0,
            register_64_bit: self.syndrome & (1 << 15) != 0,
        })
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

    pub(crate) const fn fault_address(&self) -> u64 {
        self.fault_address
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Direction {
    Write,
    Read,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Encoding {
    op0: u8,
    op1: u8,
    crn: u8,
    crm: u8,
    op2: u8,
}

impl Encoding {
    const fn new(op0: u8, op1: u8, crn: u8, crm: u8, op2: u8) -> Self {
        Self {
            op0,
            op1,
            crn,
            crm,
            op2,
        }
    }
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
    match (frame.syndrome >> EC_SHIFT) & EC_MASK {
        EC_SYSTEM_REGISTER => emulate_system_register(context, vcpu_id, frame),
        EC_HVC64 => emulate_hypercall(frame),
        EC_SMC64 => unsupported_call(frame),
        EC_WFX => {
            // A scheduler-aware WFI exit can replace this resume policy once
            // vCPU threads gain a blocked state and interrupt wakeup queue.
            frame.advance();
            GuestSyncAction::Resume
        }
        // Lower-EL aborts require stage-2 fault resolution and must not be
        // mistaken for guest Undefined Instruction exceptions.
        0x20 | 0x24 => GuestSyncAction::Unhandled,
        _ => inject_undefined(context, frame),
    }
}

fn emulate_hypercall(frame: &mut GuestSyncFrame<'_>) -> GuestSyncAction {
    let function = frame.read_general(0);
    let result = match function {
        SMCCC_VERSION => SMCCC_VERSION_1_1,
        SMCCC_ARCH_FEATURES => SMCCC_NOT_SUPPORTED,
        PSCI_VERSION => PSCI_VERSION_1_0,
        PSCI_MIGRATE_INFO_TYPE => PSCI_TOS_NOT_PRESENT,
        PSCI_FEATURES => match frame.read_general(1) {
            PSCI_VERSION | PSCI_FEATURES | PSCI_MIGRATE_INFO_TYPE => 0,
            _ => SMCCC_NOT_SUPPORTED,
        },
        _ => SMCCC_NOT_SUPPORTED,
    };
    frame.write_general(0, result);
    // HVC and SMC save the architectural return address in ELR_EL2. Unlike
    // trapped instructions such as WFx and system-register accesses, the
    // exception-generating instruction has already been consumed.
    GuestSyncAction::Resume
}

fn unsupported_call(frame: &mut GuestSyncFrame<'_>) -> GuestSyncAction {
    frame.write_general(0, SMCCC_NOT_SUPPORTED);
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
            if access.encoding == ICC_SGI1R_EL1 {
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
        encoding: Encoding::new(
            ((esr >> 20) & 0x3) as u8,
            ((esr >> 14) & 0x7) as u8,
            ((esr >> 10) & 0xf) as u8,
            ((esr >> 1) & 0xf) as u8,
            ((esr >> 17) & 0x7) as u8,
        ),
        target: ((esr >> 5) & 0x1f) as u8,
        direction: if esr & 1 == 0 {
            Direction::Write
        } else {
            Direction::Read
        },
    }
}

fn read_virtual_register(_context: &VcpuContext, vcpu_id: u32, encoding: Encoding) -> Option<u64> {
    match encoding {
        MIDR_EL1 => Some(read_midr_el1()),
        MPIDR_EL1 => Some(virtual_mpidr(vcpu_id)),
        REVIDR_EL1 => Some(read_revidr_el1()),
        ID_AA64PFR0_EL1 => Some(sanitize_pfr0(read_id_aa64pfr0_el1())),
        ID_AA64PFR1_EL1 | ID_AA64PFR2_EL1 | ID_AA64FPFR0_EL1 | ID_AA64DFR1_EL1
        | ID_AA64AFR0_EL1 | ID_AA64AFR1_EL1 | ID_AA64ISAR3_EL1 | ID_AA64MMFR3_EL1
        | ID_AA64MMFR4_EL1 | ID_AA64ZFR0_EL1 | ID_AA64SMFR0_EL1 => Some(0),
        ID_AA64DFR0_EL1 => Some(0x0000_0000_0000_0f0f),
        ID_AA64ISAR0_EL1 => Some(sanitize_isar0(read_id_aa64isar0_el1())),
        ID_AA64ISAR1_EL1 => Some(sanitize_isar1(read_id_aa64isar1_el1())),
        ID_AA64ISAR2_EL1 => Some(sanitize_isar2(read_id_aa64isar2_el1())),
        ID_AA64MMFR0_EL1 => Some(read_id_aa64mmfr0_el1()),
        ID_AA64MMFR1_EL1 => Some(read_id_aa64mmfr1_el1()),
        ID_AA64MMFR2_EL1 => Some(read_id_aa64mmfr2_el1()),
        CTR_EL0 => Some(read_ctr_el0()),
        DCZID_EL0 => Some(read_dczid_el0()),
        CNTFRQ_EL0 => Some(read_cntfrq_el0()),
        CNTPCT_EL0 => Some(read_cntvct_el0()),
        ACTLR_EL1 => Some(0),
        _ => None,
    }
}

fn write_virtual_register(encoding: Encoding, _value: u64) -> bool {
    // ACTLR_EL1 is architecturally implementation-defined. RAZ/WI prevents a
    // guest from depending on host-specific auxiliary controls.
    encoding == ACTLR_EL1
}

fn inject_undefined(context: &mut VcpuContext, frame: &mut GuestSyncFrame<'_>) -> GuestSyncAction {
    let saved_pc = *frame.program_counter;
    let saved_pstate = *frame.processor_state;
    let syndrome = frame.syndrome & ESR_IL;
    let vector_offset = match saved_pstate & SPSR_MODE_MASK {
        SPSR_EL0T => VECTOR_LOWER_EL_AARCH64,
        SPSR_EL1T => VECTOR_CURRENT_EL_SP0,
        _ => VECTOR_CURRENT_EL_SPX,
    };

    context.esr_el1 = syndrome;
    context.far_el1 = 0;
    context.elr_el1 = saved_pc;
    context.spsr_el1 = saved_pstate;
    // SAFETY: The active-vCPU bridge guarantees this is the guest whose EL1
    // bank is live on the current CPU. These writes prepare an architecturally
    // normal EL1 Undefined Instruction exception before EL2 returns.
    let live_vbar: u64;
    unsafe {
        asm!(
            "mrs {vbar}, VBAR_EL1",
            "msr ESR_EL1, {esr}",
            "msr FAR_EL1, xzr",
            "msr ELR_EL1, {elr}",
            "msr SPSR_EL1, {spsr}",
            vbar = out(reg) live_vbar,
            esr = in(reg) syndrome,
            elr = in(reg) saved_pc,
            spsr = in(reg) saved_pstate,
            options(nostack, preserves_flags)
        );
    }
    context.vbar_el1 = live_vbar;
    *frame.program_counter = live_vbar.wrapping_add(vector_offset);
    *frame.processor_state = (saved_pstate & !SPSR_MODE_AND_DAIF_MASK) | SPSR_EL1H_AND_DAIF;
    GuestSyncAction::Injected
}

const fn virtual_mpidr(vcpu_id: u32) -> u64 {
    let id = vcpu_id as u64;
    (id & 0x00ff_ffff) | ((id & 0xff00_0000) << 8)
}

fn sanitize_pfr0(_value: u64) -> u64 {
    // Present a deliberately small, internally coherent CPU contract:
    // AArch64 EL0 and EL1 plus the base FP/Advanced-SIMD implementation.
    // Optional fields use feature-specific absence encodings, so copying a
    // common all-ones mask across them would incorrectly advertise SVE.
    0x11
}

fn sanitize_isar1(value: u64) -> u64 {
    const POINTER_AUTHENTICATION: u64 = (0xf << 4) | (0xf << 8) | (0xf << 24) | (0xf << 28);
    value & !POINTER_AUTHENTICATION
}

fn sanitize_isar0(value: u64) -> u64 {
    // Transactional Memory state is not part of the vCPU context contract.
    value & !(0xf << 52)
}

fn sanitize_isar2(_value: u64) -> u64 {
    // Do not advertise newer pointer-authentication algorithms until their
    // key registers are part of the vCPU context-switch contract. ISAR2 only
    // reports optional extensions, so zero is a conservative coherent model.
    0
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

const MIDR_EL1: Encoding = Encoding::new(3, 0, 0, 0, 0);
const MPIDR_EL1: Encoding = Encoding::new(3, 0, 0, 0, 5);
const REVIDR_EL1: Encoding = Encoding::new(3, 0, 0, 0, 6);
const ID_AA64PFR0_EL1: Encoding = Encoding::new(3, 0, 0, 4, 0);
const ID_AA64PFR1_EL1: Encoding = Encoding::new(3, 0, 0, 4, 1);
const ID_AA64PFR2_EL1: Encoding = Encoding::new(3, 0, 0, 4, 2);
const ID_AA64ZFR0_EL1: Encoding = Encoding::new(3, 0, 0, 4, 4);
const ID_AA64SMFR0_EL1: Encoding = Encoding::new(3, 0, 0, 4, 5);
const ID_AA64FPFR0_EL1: Encoding = Encoding::new(3, 0, 0, 4, 7);
const ID_AA64DFR0_EL1: Encoding = Encoding::new(3, 0, 0, 5, 0);
const ID_AA64DFR1_EL1: Encoding = Encoding::new(3, 0, 0, 5, 1);
const ID_AA64AFR0_EL1: Encoding = Encoding::new(3, 0, 0, 5, 4);
const ID_AA64AFR1_EL1: Encoding = Encoding::new(3, 0, 0, 5, 5);
const ID_AA64ISAR0_EL1: Encoding = Encoding::new(3, 0, 0, 6, 0);
const ID_AA64ISAR1_EL1: Encoding = Encoding::new(3, 0, 0, 6, 1);
const ID_AA64ISAR2_EL1: Encoding = Encoding::new(3, 0, 0, 6, 2);
const ID_AA64ISAR3_EL1: Encoding = Encoding::new(3, 0, 0, 6, 3);
const ID_AA64MMFR0_EL1: Encoding = Encoding::new(3, 0, 0, 7, 0);
const ID_AA64MMFR1_EL1: Encoding = Encoding::new(3, 0, 0, 7, 1);
const ID_AA64MMFR2_EL1: Encoding = Encoding::new(3, 0, 0, 7, 2);
const ID_AA64MMFR3_EL1: Encoding = Encoding::new(3, 0, 0, 7, 3);
const ID_AA64MMFR4_EL1: Encoding = Encoding::new(3, 0, 0, 7, 4);
const CTR_EL0: Encoding = Encoding::new(3, 3, 0, 0, 1);
const DCZID_EL0: Encoding = Encoding::new(3, 3, 0, 0, 7);
const CNTFRQ_EL0: Encoding = Encoding::new(3, 3, 14, 0, 0);
const CNTPCT_EL0: Encoding = Encoding::new(3, 3, 14, 0, 1);
const ACTLR_EL1: Encoding = Encoding::new(3, 0, 1, 0, 1);
const ICC_SGI1R_EL1: Encoding = Encoding::new(3, 0, 12, 11, 5);
