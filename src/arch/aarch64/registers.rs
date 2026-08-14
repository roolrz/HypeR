#![allow(dead_code)]

//! AArch64 bootstrap register and descriptor definitions.
//!
//! This Rust module is the single source of truth. `build.rs` evaluates the
//! constants on the host and exports them as a generated C-style header for
//! startup assembly. Later Rust MMU and virtualization code must reuse these
//! definitions instead of restating architectural bit positions.

macro_rules! define_asm_constants {
    ($($name:ident = $value:expr;)+) => {
        $(pub const $name: u64 = $value;)+

        pub const ASM_CONSTANTS: &[(&str, u64)] = &[
            $((stringify!($name), $name),)+
        ];
    };
}

define_asm_constants! {
    IMAGE_TEXT_OFFSET = 0x0008_0000;
    IMAGE_FLAGS = 0x0000_000a;
    IMAGE_MAGIC = 0x644d_5241;

    LINEAR_VIRTUAL_BASE = 0x0000_4000_0000_0000;
    KERNEL_VIRTUAL_BASE = 0x0000_ff00_0000_0000;

    CURRENT_EL_EL2 = 0x8;
    HCR_EL2_FMO = 1 << 3;
    HCR_EL2_IMO = 1 << 4;
    HCR_EL2_AMO = 1 << 5;
    HCR_EL2_RW = 1 << 31;
    HCR_EL2_BOOT_VALUE = HCR_EL2_RW | HCR_EL2_FMO | HCR_EL2_IMO | HCR_EL2_AMO;
    R_AARCH64_RELATIVE = 1027;

    ID_AA64MMFR0_PARANGE_MASK = 0xf;
    ID_AA64MMFR0_PARANGE_40BIT = 0x2;
    ID_AA64MMFR0_TGRAN4_SHIFT = 28;
    ID_AA64MMFR0_TGRAN4_MASK = 0xf;
    ID_AA64MMFR0_TGRAN4_UNSUPPORTED = 0xf;

    ID_AA64ISAR0_ATOMIC_SHIFT = 20;
    ID_AA64ISAR0_ATOMIC_MASK = 0xf;
    ID_AA64ISAR0_ATOMIC_LSE = 0x2;

    CTR_EL0_IMINLINE_SHIFT = 0;
    CTR_EL0_DMINLINE_SHIFT = 16;
    CTR_EL0_LINE_SIZE_MASK = 0xf;

    EXCEPTION_FRAME_X0_OFFSET = 0;
    EXCEPTION_FRAME_X30_OFFSET = 240;
    EXCEPTION_FRAME_ELR_OFFSET = 248;
    EXCEPTION_FRAME_SPSR_OFFSET = 256;
    EXCEPTION_FRAME_ESR_OFFSET = 264;
    EXCEPTION_FRAME_FAR_OFFSET = 272;
    EXCEPTION_FRAME_VECTOR_OFFSET = 280;
    EXCEPTION_FRAME_SIMD_OFFSET = 288;
    EXCEPTION_FRAME_FPCR_OFFSET = 800;
    EXCEPTION_FRAME_FPSR_OFFSET = 808;
    EXCEPTION_FRAME_SIZE = 816;

    THREAD_CONTEXT_X19_OFFSET = 0;
    THREAD_CONTEXT_X29_OFFSET = 80;
    THREAD_CONTEXT_X30_OFFSET = 88;
    THREAD_CONTEXT_SP_OFFSET = 96;
    THREAD_CONTEXT_D8_OFFSET = 104;
    THREAD_CONTEXT_FPCR_OFFSET = 168;
    THREAD_CONTEXT_FPSR_OFFSET = 176;

    MAIR_ATTR_DEVICE_NGNRNE = 0x00;
    MAIR_ATTR_NORMAL_WB = 0xff;
    MAIR_EL2_BOOT_VALUE = MAIR_ATTR_DEVICE_NGNRNE | (MAIR_ATTR_NORMAL_WB << 8);

    TCR_EL2_RES1 = (1 << 31) | (1 << 23);
    TCR_EL2_T0SZ_32 = 32;
    TCR_EL2_T0SZ_48 = 16;
    TCR_EL2_IRGN0_WBWA = 1 << 8;
    TCR_EL2_ORGN0_WBWA = 1 << 10;
    TCR_EL2_SH0_INNER = 3 << 12;
    TCR_EL2_PS_40BIT = 2 << 16;
    TCR_EL2_BOOT_VALUE = TCR_EL2_RES1
        | TCR_EL2_T0SZ_32
        | TCR_EL2_IRGN0_WBWA
        | TCR_EL2_ORGN0_WBWA
        | TCR_EL2_SH0_INNER
        | TCR_EL2_PS_40BIT;
    TCR_EL2_FINAL_VALUE = TCR_EL2_RES1
        | TCR_EL2_T0SZ_48
        | TCR_EL2_IRGN0_WBWA
        | TCR_EL2_ORGN0_WBWA
        | TCR_EL2_SH0_INNER
        | TCR_EL2_PS_40BIT;

    SCTLR_EL2_RES1 = (1 << 4)
        | (1 << 5)
        | (1 << 11)
        | (1 << 16)
        | (1 << 18)
        | (1 << 22)
        | (1 << 23)
        | (1 << 28)
        | (1 << 29);
    SCTLR_M = 1 << 0;
    SCTLR_C = 1 << 2;
    SCTLR_SA = 1 << 3;
    SCTLR_I = 1 << 12;
    SCTLR_EL2_BOOT_VALUE = SCTLR_EL2_RES1 | SCTLR_M | SCTLR_C | SCTLR_SA | SCTLR_I;

    STAGE1_DESC_BLOCK = 0x1;
    STAGE1_DESC_ATTR_NORMAL = 1 << 2;
    STAGE1_DESC_OUTER_SHAREABLE = 2 << 8;
    STAGE1_DESC_INNER_SHAREABLE = 3 << 8;
    STAGE1_DESC_ACCESS_FLAG = 1 << 10;
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
