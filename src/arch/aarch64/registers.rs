#![allow(dead_code)]

//! `AArch64` architectural register, syndrome, and descriptor definitions.
//!
//! This module is the single source of truth for values defined by the Arm
//! architecture. `build.rs` evaluates [`ASM_CONSTANTS`] on the host and
//! exports them as a generated C-style header for assembly. Architecture code
//! must reuse these definitions rather than restating manual values locally.

macro_rules! define_asm_constants {
    ($($name:ident = $value:expr;)+) => {
        $(pub const $name: u64 = $value;)+

        pub const ASM_CONSTANTS: &[(&str, u64)] = &[
            $((stringify!($name), $name),)+
        ];
    };
}

define_asm_constants! {
    // Linux arm64 image header.
    IMAGE_TEXT_OFFSET = 0x0008_0000;
    IMAGE_FLAGS = 0x0000_000a;
    IMAGE_MAGIC = 0x644d_5241;

    // HypeR virtual-address layout.
    LINEAR_VIRTUAL_BASE = 0x0000_4000_0000_0000;
    KERNEL_VIRTUAL_BASE = 0x0000_ff00_0000_0000;

    // Exception levels and interrupt masks used by entry assembly.
    CURRENT_EL_EL2 = 0x8;
    DAIFSET_ALL = 0xf;
    DAIFCLR_IRQ = 0x2;

    // HCR_EL2.
    HCR_EL2_VM = 1 << 0;
    HCR_EL2_SWIO = 1 << 1;
    HCR_EL2_PTW = 1 << 2;
    HCR_EL2_FMO = 1 << 3;
    HCR_EL2_IMO = 1 << 4;
    HCR_EL2_AMO = 1 << 5;
    HCR_EL2_VF = 1 << 6;
    HCR_EL2_VI = 1 << 7;
    HCR_EL2_VSE = 1 << 8;
    HCR_EL2_FB = 1 << 9;
    HCR_EL2_BSU_MASK = 3 << 10;
    HCR_EL2_DC = 1 << 12;
    HCR_EL2_TWI = 1 << 13;
    HCR_EL2_TWE = 1 << 14;
    HCR_EL2_TID0 = 1 << 15;
    HCR_EL2_TID1 = 1 << 16;
    HCR_EL2_TID2 = 1 << 17;
    HCR_EL2_TID3 = 1 << 18;
    HCR_EL2_TSC = 1 << 19;
    HCR_EL2_TIDCP = 1 << 20;
    HCR_EL2_TACR = 1 << 21;
    HCR_EL2_TSW = 1 << 22;
    HCR_EL2_TPCP = 1 << 23;
    HCR_EL2_TPU = 1 << 24;
    HCR_EL2_TTLB = 1 << 25;
    HCR_EL2_TVM = 1 << 26;
    HCR_EL2_TGE = 1 << 27;
    HCR_EL2_TDZ = 1 << 28;
    HCR_EL2_HCD = 1 << 29;
    HCR_EL2_TRVM = 1 << 30;
    HCR_EL2_RW = 1 << 31;
    HCR_EL2_CD = 1 << 32;
    HCR_EL2_ID = 1 << 33;
    HCR_EL2_E2H = 1 << 34;
    HCR_EL2_BOOT_VALUE = HCR_EL2_RW
        | HCR_EL2_FMO
        | HCR_EL2_IMO
        | HCR_EL2_AMO
        | HCR_EL2_TID3;
    HCR_EL2_VHE_HOST_VALUE = HCR_EL2_BOOT_VALUE | HCR_EL2_E2H | HCR_EL2_TGE;

    // ID_AA64MMFR1_EL1.Virtualization Host Extensions.
    ID_AA64MMFR1_VH_SHIFT = 8;
    ID_AA64MMFR1_VH_MASK = 0xf;
    ID_AA64MMFR1_VH_NONE = 0x0;
    ID_AA64MMFR1_VH_VHE = 0x1;
    ID_AA64MMFR1_VH_FIELD_MASK = ID_AA64MMFR1_VH_MASK << ID_AA64MMFR1_VH_SHIFT;

    // VHE redirects CPACR_EL1 accesses to the EL2 host trap controls.
    CPACR_EL1_FPEN_SHIFT = 20;
    CPACR_EL1_FPEN_ALL = 3 << CPACR_EL1_FPEN_SHIFT;

    // ELF dynamic relocation ABI.
    R_AARCH64_RELATIVE = 1027;

    // ID_AA64MMFR0_EL1.
    ID_AA64MMFR0_PARANGE_SHIFT = 0;
    ID_AA64MMFR0_PARANGE_MASK = 0xf;
    ID_AA64MMFR0_PARANGE_32BIT = 0x0;
    ID_AA64MMFR0_PARANGE_36BIT = 0x1;
    ID_AA64MMFR0_PARANGE_40BIT = 0x2;
    ID_AA64MMFR0_PARANGE_42BIT = 0x3;
    ID_AA64MMFR0_PARANGE_44BIT = 0x4;
    ID_AA64MMFR0_PARANGE_48BIT = 0x5;
    ID_AA64MMFR0_PARANGE_52BIT = 0x6;
    ID_AA64MMFR0_TGRAN4_SHIFT = 28;
    ID_AA64MMFR0_TGRAN4_MASK = 0xf;
    ID_AA64MMFR0_TGRAN4_SUPPORTED = 0x0;
    ID_AA64MMFR0_TGRAN4_UNSUPPORTED = 0xf;

    // ID_AA64ISAR0_EL1.Atomic.
    ID_AA64ISAR0_ATOMIC_SHIFT = 20;
    ID_AA64ISAR0_ATOMIC_MASK = 0xf;
    ID_AA64ISAR0_ATOMIC_NONE = 0x0;
    ID_AA64ISAR0_ATOMIC_LSE = 0x1;
    ID_AA64ISAR0_ATOMIC_LSE128 = 0x2;

    // CTR_EL0 cache-line encodings.
    CTR_EL0_IMINLINE_SHIFT = 0;
    CTR_EL0_DMINLINE_SHIFT = 16;
    CTR_EL0_LINE_SIZE_MASK = 0xf;

    // MPIDR_EL1 affinity fields.
    MPIDR_AFF0_TO_2_MASK = 0x00ff_ffff;
    MPIDR_AFF3_SHIFT = 32;
    MPIDR_AFF3_FROM_LINEAR_ID_SHIFT = 8;
    MPIDR_AFF3_MASK = 0xff;
    GIC_AFF3_SHIFT = 24;

    // Runtime EL2 vector table and frame ABI.
    EXCEPTION_VECTOR_ALIGNMENT = 2048;
    EXCEPTION_VECTOR_SLOT_SIZE = 128;
    EXCEPTION_VECTOR_SLOT_COUNT = 16;
    EXCEPTION_VECTOR_CURRENT_SP0_SYNC = 0;
    EXCEPTION_VECTOR_CURRENT_SP0_IRQ = 1;
    EXCEPTION_VECTOR_CURRENT_SP0_FIQ = 2;
    EXCEPTION_VECTOR_CURRENT_SP0_SERROR = 3;
    EXCEPTION_VECTOR_CURRENT_SPX_SYNC = 4;
    EXCEPTION_VECTOR_CURRENT_SPX_IRQ = 5;
    EXCEPTION_VECTOR_CURRENT_SPX_FIQ = 6;
    EXCEPTION_VECTOR_CURRENT_SPX_SERROR = 7;
    EXCEPTION_VECTOR_LOWER_AARCH64_SYNC = 8;
    EXCEPTION_VECTOR_LOWER_AARCH64_IRQ = 9;
    EXCEPTION_VECTOR_LOWER_AARCH64_FIQ = 10;
    EXCEPTION_VECTOR_LOWER_AARCH64_SERROR = 11;
    EXCEPTION_VECTOR_LOWER_AARCH32_SYNC = 12;
    EXCEPTION_VECTOR_LOWER_AARCH32_IRQ = 13;
    EXCEPTION_VECTOR_LOWER_AARCH32_FIQ = 14;
    EXCEPTION_VECTOR_LOWER_AARCH32_SERROR = 15;
    EXCEPTION_FRAME_X0_OFFSET = 0;
    EXCEPTION_FRAME_X30_OFFSET = 240;
    EXCEPTION_FRAME_ELR_OFFSET = 248;
    EXCEPTION_FRAME_SPSR_OFFSET = 256;
    EXCEPTION_FRAME_ESR_OFFSET = 264;
    EXCEPTION_FRAME_FAR_OFFSET = 272;
    EXCEPTION_FRAME_VECTOR_OFFSET = 280;
    EXCEPTION_FRAME_SP_EL0_OFFSET = 288;
    EXCEPTION_FRAME_SP_EL1_OFFSET = 296;
    EXCEPTION_FRAME_SIMD_OFFSET = 304;
    EXCEPTION_FRAME_FPCR_OFFSET = 816;
    EXCEPTION_FRAME_FPSR_OFFSET = 824;
    EXCEPTION_FRAME_SIZE = 832;

    // CrashContext ABI shared with the panic register-capture assembly.
    CRASH_CONTEXT_X0_OFFSET = 0;
    CRASH_CONTEXT_X30_OFFSET = 240;
    CRASH_CONTEXT_VALID_OFFSET = 248;
    CRASH_CONTEXT_SP_OFFSET = 256;
    CRASH_CONTEXT_PC_OFFSET = 264;
    CRASH_CONTEXT_PSTATE_OFFSET = 272;
    CRASH_CONTEXT_ESR_OFFSET = 280;
    CRASH_CONTEXT_FAR_OFFSET = 288;
    CRASH_CONTEXT_VECTOR_OFFSET = 296;
    CRASH_CONTEXT_MPIDR_OFFSET = 304;
    CRASH_CONTEXT_CURRENT_EL_OFFSET = 312;
    CRASH_CONTEXT_DAIF_OFFSET = 320;
    CRASH_CONTEXT_SCTLR_EL2_OFFSET = 328;
    CRASH_CONTEXT_TCR_EL2_OFFSET = 336;
    CRASH_CONTEXT_TTBR0_EL2_OFFSET = 344;
    CRASH_CONTEXT_VBAR_EL2_OFFSET = 352;
    CRASH_CONTEXT_HCR_EL2_OFFSET = 360;
    CRASH_CONTEXT_SIZE = 368;

    // ICC_SGI1R_EL1 fields used for emergency all-but-self IPIs.
    GIC_CRASH_STOP_SGI = 15;
    ICC_SGI1R_INTID_SHIFT = 24;
    ICC_SGI1R_IRM = 1 << 40;
    GIC_SPURIOUS_INTERRUPT_MIN = 1020;

    // Kernel thread and vCPU context ABIs shared with context.S.
    THREAD_CONTEXT_X19_OFFSET = 0;
    THREAD_CONTEXT_X29_OFFSET = 80;
    THREAD_CONTEXT_X30_OFFSET = 88;
    THREAD_CONTEXT_SP_OFFSET = 96;
    THREAD_CONTEXT_D8_OFFSET = 104;
    THREAD_CONTEXT_FPCR_OFFSET = 168;
    THREAD_CONTEXT_FPSR_OFFSET = 176;
    VCPU_CONTEXT_X0_OFFSET = 0;
    VCPU_CONTEXT_X30_OFFSET = 240;
    VCPU_CONTEXT_PC_OFFSET = 264;
    VCPU_CONTEXT_PSTATE_OFFSET = 272;

    // MAIR_ELx attribute encodings.
    MAIR_ATTR_DEVICE_NGNRNE = 0x00;
    MAIR_ATTR_DEVICE_NGNRE = 0x04;
    MAIR_ATTR_DEVICE_NGRE = 0x08;
    MAIR_ATTR_DEVICE_GRE = 0x0c;
    MAIR_ATTR_NORMAL_WB = 0xff;
    MAIR_EL2_BOOT_VALUE = MAIR_ATTR_DEVICE_NGNRNE | (MAIR_ATTR_NORMAL_WB << 8);

    // TCR_EL2, 4 KiB granule.
    TCR_EL2_T0SZ_MASK = 0x3f;
    TCR_EL2_RES1 = (1 << 31) | (1 << 23);
    TCR_EL2_T0SZ_32 = 32;
    TCR_EL2_T0SZ_48 = 16;
    TCR_EL2_IRGN0_WBWA = 1 << 8;
    TCR_EL2_ORGN0_WBWA = 1 << 10;
    TCR_EL2_SH0_INNER = 3 << 12;
    TCR_EL2_TG0_4K = 0 << 14;
    TCR_EL2_TG0_64K = 1 << 14;
    TCR_EL2_TG0_16K = 2 << 14;
    TCR_EL2_PS_40BIT = 2 << 16;
    TCR_EL2_BOOT_VALUE = TCR_EL2_RES1
        | TCR_EL2_T0SZ_32
        | TCR_EL2_IRGN0_WBWA
        | TCR_EL2_ORGN0_WBWA
        | TCR_EL2_SH0_INNER
        | TCR_EL2_TG0_4K
        | TCR_EL2_PS_40BIT;
    TCR_EL2_FINAL_VALUE = TCR_EL2_RES1
        | TCR_EL2_T0SZ_48
        | TCR_EL2_IRGN0_WBWA
        | TCR_EL2_ORGN0_WBWA
        | TCR_EL2_SH0_INNER
        | TCR_EL2_TG0_4K
        | TCR_EL2_PS_40BIT;

    // SCTLR_EL1/EL2 fields used by the base Armv8-A execution model.
    SCTLR_M = 1 << 0;
    SCTLR_A = 1 << 1;
    SCTLR_C = 1 << 2;
    SCTLR_SA = 1 << 3;
    SCTLR_SA0 = 1 << 4;
    SCTLR_CP15BEN = 1 << 5;
    SCTLR_ITD = 1 << 7;
    SCTLR_SED = 1 << 8;
    SCTLR_UMA = 1 << 9;
    SCTLR_ENRCTX = 1 << 10;
    SCTLR_EOS = 1 << 11;
    SCTLR_I = 1 << 12;
    SCTLR_ENDB = 1 << 13;
    SCTLR_DZE = 1 << 14;
    SCTLR_UCT = 1 << 15;
    SCTLR_NTWI = 1 << 16;
    SCTLR_NTWE = 1 << 18;
    SCTLR_WXN = 1 << 19;
    SCTLR_TSCXT = 1 << 20;
    SCTLR_IESB = 1 << 21;
    SCTLR_EIS = 1 << 22;
    SCTLR_SPAN = 1 << 23;
    SCTLR_E0E = 1 << 24;
    SCTLR_EE = 1 << 25;
    SCTLR_UCI = 1 << 26;
    SCTLR_ENDA = 1 << 27;
    SCTLR_NTLSMD = 1 << 28;
    SCTLR_LSMAOE = 1 << 29;
    SCTLR_EL2_RES1 = SCTLR_SA0
        | SCTLR_CP15BEN
        | SCTLR_EOS
        | SCTLR_NTWI
        | SCTLR_NTWE
        | SCTLR_EIS
        | SCTLR_SPAN
        | SCTLR_NTLSMD
        | SCTLR_LSMAOE;
    SCTLR_EL2_BOOT_VALUE = SCTLR_EL2_RES1 | SCTLR_M | SCTLR_C | SCTLR_SA | SCTLR_I;

    // Stage-1 translation descriptors for the EL2 regime.
    STAGE1_DESC_INVALID = 0x0;
    TRANSLATION_DESC_TYPE_MASK = 0x3;
    STAGE1_DESC_BLOCK = 0x1;
    STAGE1_DESC_TABLE_OR_PAGE = 0x3;
    STAGE1_DESC_ATTR_INDEX_MASK = 7 << 2;
    STAGE1_DESC_ATTR_DEVICE = 0 << 2;
    STAGE1_DESC_ATTR_NORMAL = 1 << 2;
    STAGE1_DESC_NON_SHAREABLE = 0 << 8;
    STAGE1_DESC_OUTER_SHAREABLE = 2 << 8;
    STAGE1_DESC_INNER_SHAREABLE = 3 << 8;
    STAGE1_DESC_AP_READ_ONLY = 1 << 7;
    STAGE1_DESC_ACCESS_FLAG = 1 << 10;
    STAGE1_DESC_NOT_GLOBAL = 1 << 11;
    STAGE1_DESC_CONTIGUOUS = 1 << 52;
    STAGE1_DESC_PXN = 1 << 53;
    STAGE1_DESC_EXECUTE_NEVER = 1 << 54;
    BOOT_DEVICE_BLOCK_FLAGS = STAGE1_DESC_BLOCK
        | STAGE1_DESC_OUTER_SHAREABLE
        | STAGE1_DESC_ACCESS_FLAG
        | STAGE1_DESC_EXECUTE_NEVER;
    BOOT_NORMAL_BLOCK_FLAGS = STAGE1_DESC_BLOCK
        | STAGE1_DESC_ATTR_NORMAL
        | STAGE1_DESC_INNER_SHAREABLE
        | STAGE1_DESC_ACCESS_FLAG;
}

// ESR_ELx common fields and exception classes.
pub const AARCH64_INSTRUCTION_SIZE: u64 = 4;
pub const STACK_ALIGNMENT: u64 = 16;
pub const STACK_ALIGNMENT_MASK: u64 = STACK_ALIGNMENT - 1;
pub const ESR_EC_SHIFT: u64 = 26;
pub const ESR_EC_MASK: u64 = 0x3f;
pub const ESR_IL: u64 = 1 << 25;
pub const ESR_ISS_MASK: u64 = 0x01ff_ffff;
pub const ESR_BRK_COMMENT_MASK: u64 = 0xffff;
pub const ESR_EC_UNKNOWN: u64 = 0x00;
pub const ESR_EC_WFX: u64 = 0x01;
pub const ESR_EC_CP15_RT: u64 = 0x03;
pub const ESR_EC_CP15_RRT: u64 = 0x04;
pub const ESR_EC_CP14_RT: u64 = 0x05;
pub const ESR_EC_CP14_DT: u64 = 0x06;
pub const ESR_EC_FP_ASIMD: u64 = 0x07;
pub const ESR_EC_CP10_ID: u64 = 0x08;
pub const ESR_EC_PAC: u64 = 0x09;
pub const ESR_EC_CP14_RRT: u64 = 0x0c;
pub const ESR_EC_ILLEGAL_STATE: u64 = 0x0e;
pub const ESR_EC_SVC32: u64 = 0x11;
pub const ESR_EC_HVC32: u64 = 0x12;
pub const ESR_EC_SMC32: u64 = 0x13;
pub const ESR_EC_SVC64: u64 = 0x15;
pub const ESR_EC_HVC64: u64 = 0x16;
pub const ESR_EC_SMC64: u64 = 0x17;
pub const ESR_EC_SYSTEM_REGISTER: u64 = 0x18;
pub const ESR_EC_SVE: u64 = 0x19;
pub const ESR_EC_ERET: u64 = 0x1a;
pub const ESR_EC_PAC_FAILURE: u64 = 0x1c;
pub const ESR_EC_SME: u64 = 0x1d;
pub const ESR_EC_INSTRUCTION_ABORT_LOWER: u64 = 0x20;
pub const ESR_EC_INSTRUCTION_ABORT_CURRENT: u64 = 0x21;
pub const ESR_EC_PC_ALIGNMENT: u64 = 0x22;
pub const ESR_EC_DATA_ABORT_LOWER: u64 = 0x24;
pub const ESR_EC_DATA_ABORT_CURRENT: u64 = 0x25;
pub const ESR_EC_SP_ALIGNMENT: u64 = 0x26;
pub const ESR_EC_FP32: u64 = 0x28;
pub const ESR_EC_FP64: u64 = 0x2c;
pub const ESR_EC_SERROR: u64 = 0x2f;
pub const ESR_EC_BREAKPOINT_LOWER: u64 = 0x30;
pub const ESR_EC_BREAKPOINT_CURRENT: u64 = 0x31;
pub const ESR_EC_SOFTWARE_STEP_LOWER: u64 = 0x32;
pub const ESR_EC_SOFTWARE_STEP_CURRENT: u64 = 0x33;
pub const ESR_EC_WATCHPOINT_LOWER: u64 = 0x34;
pub const ESR_EC_WATCHPOINT_CURRENT: u64 = 0x35;
pub const ESR_EC_BKPT32: u64 = 0x38;
pub const ESR_EC_VECTOR_CATCH: u64 = 0x3a;
pub const ESR_EC_BRK64: u64 = 0x3c;

// ESR_ELx ISS layout for trapped system-register accesses.
pub const ESR_SYSREG_DIRECTION_READ: u64 = 1 << 0;
pub const ESR_SYSREG_CRM_SHIFT: u64 = 1;
pub const ESR_SYSREG_CRM_MASK: u64 = 0xf;
pub const ESR_SYSREG_RT_SHIFT: u64 = 5;
pub const ESR_SYSREG_RT_MASK: u64 = 0x1f;
pub const ESR_SYSREG_CRN_SHIFT: u64 = 10;
pub const ESR_SYSREG_CRN_MASK: u64 = 0xf;
pub const ESR_SYSREG_OP1_SHIFT: u64 = 14;
pub const ESR_SYSREG_OP1_MASK: u64 = 0x7;
pub const ESR_SYSREG_OP2_SHIFT: u64 = 17;
pub const ESR_SYSREG_OP2_MASK: u64 = 0x7;
pub const ESR_SYSREG_OP0_SHIFT: u64 = 20;
pub const ESR_SYSREG_OP0_MASK: u64 = 0x3;

// ESR_ELx ISS layout for data aborts with a valid instruction syndrome.
pub const ESR_ABORT_FSC_MASK: u64 = 0x3f;
pub const ESR_ABORT_TRANSLATION_FAULT_LEVEL0: u64 = 0b000100;
pub const ESR_ABORT_TRANSLATION_FAULT_LEVEL3: u64 = 0b000111;
pub const ESR_DATA_ABORT_WNR: u64 = 1 << 6;
pub const ESR_DATA_ABORT_S1PTW: u64 = 1 << 7;
pub const ESR_DATA_ABORT_SF: u64 = 1 << 15;
pub const ESR_DATA_ABORT_SRT_SHIFT: u64 = 16;
pub const ESR_DATA_ABORT_SRT_MASK: u64 = 0x1f;
pub const ESR_DATA_ABORT_SSE: u64 = 1 << 21;
pub const ESR_DATA_ABORT_SAS_SHIFT: u64 = 22;
pub const ESR_DATA_ABORT_SAS_MASK: u64 = 0x3;
pub const ESR_DATA_ABORT_ISV: u64 = 1 << 24;

// SPSR_ELx/PSTATE mode and mask fields.
pub const SPSR_M_MASK: u64 = 0xf;
pub const SPSR_AARCH32_M_MASK: u64 = 0x1f;
pub const SPSR_EL0T: u64 = 0x0;
pub const SPSR_EL1T: u64 = 0x4;
pub const SPSR_EL1H: u64 = 0x5;
pub const SPSR_EL2T: u64 = 0x8;
pub const SPSR_EL2H: u64 = 0x9;
pub const SPSR_AARCH32_USR: u64 = 0x10;
pub const SPSR_AARCH32_SYS: u64 = 0x1f;
pub const SPSR_F: u64 = 1 << 6;
pub const SPSR_I: u64 = 1 << 7;
pub const SPSR_A: u64 = 1 << 8;
pub const SPSR_D: u64 = 1 << 9;
pub const SPSR_DAIF_MASK: u64 = SPSR_D | SPSR_A | SPSR_I | SPSR_F;
pub const SPSR_MODE_AND_DAIF_MASK: u64 = SPSR_M_MASK | SPSR_DAIF_MASK;
pub const SPSR_EL1H_AND_DAIF: u64 = SPSR_EL1H | SPSR_DAIF_MASK;

// Exception-vector offsets relative to VBAR_ELx.
pub const VECTOR_CURRENT_EL_SP0: u64 = 0x000;
pub const VECTOR_CURRENT_EL_SPX: u64 = 0x200;
pub const VECTOR_LOWER_EL_AARCH64: u64 = 0x400;
pub const VECTOR_LOWER_EL_AARCH32: u64 = 0x600;

// Runtime vector self-test contract.
pub const EXCEPTION_VECTOR_TEST_IMMEDIATE: u64 = 0x4859;
pub const EXCEPTION_VECTOR_TEST_INVALID_SP: u64 = 0x1000;
pub const EXCEPTION_VECTOR_TEST_NONE: u64 = u64::MAX;

// HPFAR_EL2.FIPA and the 4 KiB page offset used to reconstruct an IPA.
pub const HPFAR_EL2_FIPA_MASK: u64 = 0x0000_00ff_ffff_fff0;
pub const HPFAR_EL2_FIPA_TO_IPA_SHIFT: u32 = 8;
pub const PAGE_OFFSET_MASK_4K: u64 = 0xfff;

// Generic timer control fields.
pub const CNT_CTL_ENABLE: u64 = 1 << 0;
pub const CNT_CTL_IMASK: u64 = 1 << 1;
pub const CNT_CTL_ISTATUS: u64 = 1 << 2;

// GICv3 physical and virtual CPU-interface fields.
pub const ICC_SRE_EL2_SRE: u64 = 1 << 0;
pub const ICC_CTLR_EL1_EOI_MODE: u64 = 1 << 1;
pub const ICC_IGRPEN1_ENABLE: u64 = 1 << 0;
pub const ICC_IAR1_INTID_MASK: u64 = 0x00ff_ffff;
pub const ICC_PMR_ALLOW_ALL: u64 = 0xff;
pub const ICH_VTR_LIST_REGISTERS_MASK: u64 = 0x1f;
pub const ICH_VTR_ID_BITS_SHIFT: u64 = 23;
pub const ICH_VTR_ID_BITS_MASK: u64 = 0x7;
pub const ICH_VTR_PREEMPTION_BITS_SHIFT: u64 = 26;
pub const ICH_VTR_PRIORITY_BITS_SHIFT: u64 = 29;
pub const ICH_VTR_BITS_MASK: u64 = 0x7;
pub const ICH_HCR_ENABLE: u64 = 1 << 0;
pub const ICH_VMCR_ENABLE_GROUP0: u64 = 1 << 0;
pub const ICH_VMCR_ENABLE_GROUP1: u64 = 1 << 1;
pub const ICH_VMCR_ACK_CONTROL: u64 = 1 << 2;
pub const ICH_VMCR_EOI_MODE: u64 = 1 << 9;
pub const ICH_VMCR_PRIORITY_MASK_SHIFT: u64 = 24;
pub const ICH_VMCR_PRIORITY_MASK: u64 = 0xff << ICH_VMCR_PRIORITY_MASK_SHIFT;
pub const ICH_VMCR_PRIORITY_MASK_ALLOW_ALL: u64 = ICH_VMCR_PRIORITY_MASK;

// Stage-2 translation control and descriptor fields, 4 KiB granule.
pub const VTCR_EL2_T0SZ_MASK: u64 = 0x3f;
pub const VTCR_EL2_T0SZ_39BIT: u64 = 25;
pub const VTCR_EL2_SL0_LEVEL2: u64 = 0 << 6;
pub const VTCR_EL2_SL0_LEVEL1: u64 = 1 << 6;
pub const VTCR_EL2_IRGN0_WBWA: u64 = 1 << 8;
pub const VTCR_EL2_ORGN0_WBWA: u64 = 1 << 10;
pub const VTCR_EL2_SH0_INNER: u64 = 3 << 12;
pub const VTCR_EL2_TG0_4K: u64 = 0 << 14;
pub const VTCR_EL2_PS_40BIT: u64 = 2 << 16;
pub const VTCR_EL2_RES1: u64 = 1 << 31;
pub const VTCR_EL2_GUEST_VALUE: u64 = VTCR_EL2_RES1
    | VTCR_EL2_T0SZ_39BIT
    | VTCR_EL2_SL0_LEVEL1
    | VTCR_EL2_IRGN0_WBWA
    | VTCR_EL2_ORGN0_WBWA
    | VTCR_EL2_SH0_INNER
    | VTCR_EL2_TG0_4K
    | VTCR_EL2_PS_40BIT;
pub const VTTBR_EL2_VMID_SHIFT: u32 = 48;
pub const TLBI_IPAS2E1_IPA_SHIFT: u32 = 12;
pub const STAGE2_DESC_INVALID: u64 = 0b00;
pub const STAGE2_DESC_BLOCK: u64 = 0b01;
pub const STAGE2_DESC_TABLE_OR_PAGE: u64 = 0b11;
pub const STAGE2_DESC_MEMATTR_DEVICE_NGNRE: u64 = 0x1 << 2;
pub const STAGE2_DESC_MEMATTR_NORMAL_WB: u64 = 0xf << 2;
pub const STAGE2_DESC_READ_ONLY: u64 = 0x1 << 6;
pub const STAGE2_DESC_WRITE_ONLY: u64 = 0x2 << 6;
pub const STAGE2_DESC_READ_WRITE: u64 = 0x3 << 6;
pub const STAGE2_DESC_INNER_SHAREABLE: u64 = 0x3 << 8;
pub const STAGE2_DESC_ACCESS_FLAG: u64 = 1 << 10;
pub const TRANSLATION_DESC_ADDRESS_MASK_48BIT: u64 = 0x0000_ffff_ffff_f000;
pub const TRANSLATION_GRANULE_4K: u64 = 4096;
pub const TRANSLATION_TABLE_ENTRY_COUNT_4K: usize = 512;
pub const STAGE1_VA_BITS: u32 = 48;
pub const STAGE1_VA_LIMIT: u64 = 1 << STAGE1_VA_BITS;
pub const PHYSICAL_ADDRESS_BITS: u32 = 40;
pub const PHYSICAL_ADDRESS_LIMIT: u64 = 1 << PHYSICAL_ADDRESS_BITS;
pub const STAGE1_LEVEL_SHIFTS_4K: [u32; 4] = [39, 30, 21, 12];
pub const STAGE1_LEVEL_SIZES_4K: [u64; 4] = [1 << 39, 1 << 30, 1 << 21, 1 << 12];
pub const STAGE2_IPA_BITS: u32 = 39;
pub const STAGE2_IPA_LIMIT: u64 = 1 << STAGE2_IPA_BITS;
pub const STAGE2_LEVEL_SHIFTS_4K: [u32; 3] = [30, 21, 12];
pub const STAGE2_LEVEL_SIZES_4K: [u64; 3] = [1 << 30, 1 << 21, 1 << 12];

// Identification-register fields filtered by the guest CPU model.
pub const ID_AA64PFR0_GUEST_BASE: u64 = 0x11;
pub const ID_AA64DFR0_GUEST_BASE: u64 = 0x0000_0000_0000_0f0f;
pub const ID_AA64ISAR0_TME_MASK: u64 = 0xf << 52;
pub const ID_AA64ISAR1_POINTER_AUTH_MASK: u64 = (0xf << 4) | (0xf << 8) | (0xf << 24) | (0xf << 28);
pub const MPIDR_LINEAR_AFF3_MASK: u64 = 0xff00_0000;

// GICv3 permits up to sixteen virtual list registers.
pub const ICH_MAX_LIST_REGISTERS: usize = 16;

/// Architecturally required `SCTLR_EL1` reset policy for an `AArch64` guest.
pub const SCTLR_EL1_GUEST_RESET_VALUE: u64 =
    SCTLR_LSMAOE | SCTLR_NTLSMD | SCTLR_SPAN | SCTLR_EIS | SCTLR_TSCXT | SCTLR_EOS;

// SMCCC and PSCI architectural calling-convention constants.
pub const SMCCC_NOT_SUPPORTED: u64 = u64::MAX;
pub const SMCCC_VERSION: u64 = 0x8000_0000;
pub const SMCCC_ARCH_FEATURES: u64 = 0x8000_0001;
pub const SMCCC_VERSION_1_1: u64 = 0x0001_0001;
pub const PSCI_VERSION: u64 = 0x8400_0000;
pub const PSCI_MIGRATE_INFO_TYPE: u64 = 0x8400_0006;
pub const PSCI_FEATURES: u64 = 0x8400_000a;
pub const PSCI_VERSION_1_0: u64 = 0x0001_0000;
pub const PSCI_TOS_NOT_PRESENT: u64 = 2;

/// Encoding of an `AArch64` system register in an `MRS`/`MSR` instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SystemRegisterEncoding {
    pub op0: u8,
    pub op1: u8,
    pub crn: u8,
    pub crm: u8,
    pub op2: u8,
}

impl SystemRegisterEncoding {
    pub const fn new(op0: u8, op1: u8, crn: u8, crm: u8, op2: u8) -> Self {
        Self {
            op0,
            op1,
            crn,
            crm,
            op2,
        }
    }

    pub const fn from_esr(esr: u64) -> Self {
        Self::new(
            ((esr >> ESR_SYSREG_OP0_SHIFT) & ESR_SYSREG_OP0_MASK) as u8,
            ((esr >> ESR_SYSREG_OP1_SHIFT) & ESR_SYSREG_OP1_MASK) as u8,
            ((esr >> ESR_SYSREG_CRN_SHIFT) & ESR_SYSREG_CRN_MASK) as u8,
            ((esr >> ESR_SYSREG_CRM_SHIFT) & ESR_SYSREG_CRM_MASK) as u8,
            ((esr >> ESR_SYSREG_OP2_SHIFT) & ESR_SYSREG_OP2_MASK) as u8,
        )
    }
}

pub const SYSREG_MIDR_EL1: SystemRegisterEncoding = SystemRegisterEncoding::new(3, 0, 0, 0, 0);
pub const SYSREG_MPIDR_EL1: SystemRegisterEncoding = SystemRegisterEncoding::new(3, 0, 0, 0, 5);
pub const SYSREG_REVIDR_EL1: SystemRegisterEncoding = SystemRegisterEncoding::new(3, 0, 0, 0, 6);
pub const SYSREG_ID_AA64PFR0_EL1: SystemRegisterEncoding =
    SystemRegisterEncoding::new(3, 0, 0, 4, 0);
pub const SYSREG_ID_AA64PFR1_EL1: SystemRegisterEncoding =
    SystemRegisterEncoding::new(3, 0, 0, 4, 1);
pub const SYSREG_ID_AA64PFR2_EL1: SystemRegisterEncoding =
    SystemRegisterEncoding::new(3, 0, 0, 4, 2);
pub const SYSREG_ID_AA64ZFR0_EL1: SystemRegisterEncoding =
    SystemRegisterEncoding::new(3, 0, 0, 4, 4);
pub const SYSREG_ID_AA64SMFR0_EL1: SystemRegisterEncoding =
    SystemRegisterEncoding::new(3, 0, 0, 4, 5);
pub const SYSREG_ID_AA64FPFR0_EL1: SystemRegisterEncoding =
    SystemRegisterEncoding::new(3, 0, 0, 4, 7);
pub const SYSREG_ID_AA64DFR0_EL1: SystemRegisterEncoding =
    SystemRegisterEncoding::new(3, 0, 0, 5, 0);
pub const SYSREG_ID_AA64DFR1_EL1: SystemRegisterEncoding =
    SystemRegisterEncoding::new(3, 0, 0, 5, 1);
pub const SYSREG_ID_AA64AFR0_EL1: SystemRegisterEncoding =
    SystemRegisterEncoding::new(3, 0, 0, 5, 4);
pub const SYSREG_ID_AA64AFR1_EL1: SystemRegisterEncoding =
    SystemRegisterEncoding::new(3, 0, 0, 5, 5);
pub const SYSREG_ID_AA64ISAR0_EL1: SystemRegisterEncoding =
    SystemRegisterEncoding::new(3, 0, 0, 6, 0);
pub const SYSREG_ID_AA64ISAR1_EL1: SystemRegisterEncoding =
    SystemRegisterEncoding::new(3, 0, 0, 6, 1);
pub const SYSREG_ID_AA64ISAR2_EL1: SystemRegisterEncoding =
    SystemRegisterEncoding::new(3, 0, 0, 6, 2);
pub const SYSREG_ID_AA64ISAR3_EL1: SystemRegisterEncoding =
    SystemRegisterEncoding::new(3, 0, 0, 6, 3);
pub const SYSREG_ID_AA64MMFR0_EL1: SystemRegisterEncoding =
    SystemRegisterEncoding::new(3, 0, 0, 7, 0);
pub const SYSREG_ID_AA64MMFR1_EL1: SystemRegisterEncoding =
    SystemRegisterEncoding::new(3, 0, 0, 7, 1);
pub const SYSREG_ID_AA64MMFR2_EL1: SystemRegisterEncoding =
    SystemRegisterEncoding::new(3, 0, 0, 7, 2);
pub const SYSREG_ID_AA64MMFR3_EL1: SystemRegisterEncoding =
    SystemRegisterEncoding::new(3, 0, 0, 7, 3);
pub const SYSREG_ID_AA64MMFR4_EL1: SystemRegisterEncoding =
    SystemRegisterEncoding::new(3, 0, 0, 7, 4);
pub const SYSREG_CTR_EL0: SystemRegisterEncoding = SystemRegisterEncoding::new(3, 3, 0, 0, 1);
pub const SYSREG_DCZID_EL0: SystemRegisterEncoding = SystemRegisterEncoding::new(3, 3, 0, 0, 7);
pub const SYSREG_CNTFRQ_EL0: SystemRegisterEncoding = SystemRegisterEncoding::new(3, 3, 14, 0, 0);
pub const SYSREG_CNTPCT_EL0: SystemRegisterEncoding = SystemRegisterEncoding::new(3, 3, 14, 0, 1);
pub const SYSREG_CNTVCT_EL0: SystemRegisterEncoding = SystemRegisterEncoding::new(3, 3, 14, 0, 2);
pub const SYSREG_ACTLR_EL1: SystemRegisterEncoding = SystemRegisterEncoding::new(3, 0, 1, 0, 1);
pub const SYSREG_ICC_SGI1R_EL1: SystemRegisterEncoding =
    SystemRegisterEncoding::new(3, 0, 12, 11, 5);
