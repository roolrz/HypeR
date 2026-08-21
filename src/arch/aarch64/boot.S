/*
 * SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 */

/*
 * AArch64 position-independent bootstrap for the Linux kernel boot ABI.
 *
 * This is the only assembly startup path. It establishes deterministic EL2
 * state, a QEMU `virt` bootstrap identity map, and a Rust-compatible stack.
 * All symbol addresses are formed with PC-relative instructions.
 */

#include "asm_constants.h"

.section .text.boot.header, "ax"
.global _start
.type _start, %function

/* Linux AArch64 Image header. code0 branches over the 64-byte header. */
_start:
    b       aarch64_primary_entry
    .long   0
    .quad   IMAGE_TEXT_OFFSET
    .quad   __image_size
    .quad   IMAGE_FLAGS
    .quad   0
    .quad   0
    .quad   0
    .long   IMAGE_MAGIC
    .long   0
.Limage_header_end:
.size _start, .Limage_header_end - _start

.section .text.boot, "ax"
.balign 16
.type aarch64_primary_entry, %function
aarch64_primary_entry:
    msr     daifset, #DAIFSET_ALL
    clrex

    // A hypervisor kernel requires EL2. EL1 fallback is intentionally rejected.
    mrs     x1, CurrentEL
    cmp     x1, #CURRENT_EL_EL2
    b.ne    .Lunsupported_boot

    // Firmware/PSCI should retain secondaries. Park any that enter defensively.
    mrs     x1, mpidr_el1
    ubfx    x2, x1, #32, #8
    and     x1, x1, #MPIDR_AFF0_TO_2_MASK
    orr     x1, x1, x2
    cbnz    x1, .Lsecondary_halt

    // Preserve the physical DTB address until the Rust ABI call.
    mov     x20, x0
    adr     x21, _start

    // Validate 4 KiB translation and retain a <=48-bit bootstrap PS encoding.
    mrs     x1, id_aa64mmfr0_el1
    and     x23, x1, #ID_AA64MMFR0_PARANGE_MASK
    cmp     x23, #ID_AA64MMFR0_PARANGE_52BIT
    b.hi    .Lunsupported_boot
    mov     x2, #ID_AA64MMFR0_PARANGE_48BIT
    cmp     x23, x2
    csel    x23, x23, x2, ls
    lsr     x2, x1, #ID_AA64MMFR0_TGRAN4_SHIFT
    and     x2, x2, #ID_AA64MMFR0_TGRAN4_MASK
    cmp     x2, #ID_AA64MMFR0_TGRAN4_UNSUPPORTED
    b.eq    .Lunsupported_boot

    /*
     * The Linux boot ABI does not require SCTLR_EL2.C to be clear. Normalize
     * the cache/MMU state before the first kernel store so PIE relocations
     * cannot be stranded in a cache that is disabled immediately afterwards.
     * The loader is responsible for cleaning the loaded Image to PoC.
     */
    dsb     sy
    msr     spsel, #1
    ldr     x1, =SCTLR_EL2_RES1
    msr     sctlr_el2, x1
    isb

    bl      aarch64_boot_apply_relocations
    bl      aarch64_boot_initialize_el2
    bl      aarch64_boot_clear_bss
    bl      aarch64_boot_build_page_tables
    bl      aarch64_boot_enable_translation

    adrp    x1, __boot_stack_top
    add     x1, x1, :lo12:__boot_stack_top
    mov     sp, x1
    mov     x29, xzr
    mov     x30, xzr
    mov     x0, x20
    bl      aarch64_bootstrap
.size aarch64_primary_entry, . - aarch64_primary_entry

/* Applies physical-load PIE relocations. x21 is the physical load bias. */
.type aarch64_boot_apply_relocations, %function
aarch64_boot_apply_relocations:
    adrp    x1, __rela_dyn_start
    add     x1, x1, :lo12:__rela_dyn_start
    adrp    x2, __rela_dyn_end
    add     x2, x2, :lo12:__rela_dyn_end
.Lrelocate:
    cmp     x1, x2
    b.hs    .Lrelocation_done
    ldr     x3, [x1]
    ldr     x4, [x1, #8]
    ldr     x5, [x1, #16]
    cmp     w4, #R_AARCH64_RELATIVE
    b.ne    .Lunsupported_boot
    add     x3, x21, x3
    add     x5, x21, x5
    str     x5, [x3]
    add     x1, x1, #24
    b       .Lrelocate
.Lrelocation_done:
    adrp    x1, __relr_dyn_start
    add     x1, x1, :lo12:__relr_dyn_start
    adrp    x2, __relr_dyn_end
    add     x2, x2, :lo12:__relr_dyn_end
    mov     x4, xzr
.Lrelr_load_loop:
    cmp     x1, x2
    b.hs    .Lrelr_load_done
    ldr     x3, [x1], #8
    tbnz    x3, #0, .Lrelr_load_bitmap
    add     x4, x21, x3
    ldr     x5, [x4]
    add     x5, x5, x21
    str     x5, [x4], #8
    b       .Lrelr_load_loop
.Lrelr_load_bitmap:
    cbz     x4, .Lunsupported_boot
    lsr     x3, x3, #1
    mov     x6, #63
.Lrelr_load_bitmap_loop:
    tbz     x3, #0, .Lrelr_load_bitmap_next
    ldr     x5, [x4]
    add     x5, x5, x21
    str     x5, [x4]
.Lrelr_load_bitmap_next:
    add     x4, x4, #8
    lsr     x3, x3, #1
    subs    x6, x6, #1
    b.ne    .Lrelr_load_bitmap_loop
    b       .Lrelr_load_loop
.Lrelr_load_done:
    ret
.size aarch64_boot_apply_relocations, . - aarch64_boot_apply_relocations

/* Establishes deterministic EL2 controls before enabling translation. */
.type aarch64_boot_initialize_el2, %function
aarch64_boot_initialize_el2:
    ldr     x1, =HCR_EL2_BOOT_VALUE
    mrs     x2, id_aa64mmfr1_el1
    lsr     x2, x2, #ID_AA64MMFR1_VH_SHIFT
    and     x2, x2, #ID_AA64MMFR1_VH_MASK
    cbz     x2, .Linitialize_el2_hcr
    ldr     x1, =HCR_EL2_VHE_HOST_VALUE
.Linitialize_el2_hcr:
    msr     hcr_el2, x1
    tst     x1, #HCR_EL2_E2H
    b.eq    .Linitialize_el2_nvhe_cptr
    ldr     x2, =CPACR_EL1_FPEN_ALL
    msr     cpacr_el1, x2
    b       .Linitialize_el2_cptr_done
.Linitialize_el2_nvhe_cptr:
    msr     cptr_el2, xzr
.Linitialize_el2_cptr_done:
    msr     hstr_el2, xzr
    msr     mdcr_el2, xzr
    msr     vttbr_el2, xzr
    msr     tpidr_el2, xzr
    msr     cntvoff_el2, xzr
    msr     cnthctl_el2, xzr
    msr     cnthp_ctl_el2, xzr
    isb
    ret
.size aarch64_boot_initialize_el2, . - aarch64_boot_initialize_el2

/* Clears Rust static storage and installs the early fault vectors. */
.type aarch64_boot_clear_bss, %function
aarch64_boot_clear_bss:
    adrp    x1, __bss_start
    add     x1, x1, :lo12:__bss_start
    adrp    x2, __bss_end
    add     x2, x2, :lo12:__bss_end
.Lbss_loop:
    cmp     x1, x2
    b.hs    .Lbss_done
    str     xzr, [x1], #8
    b       .Lbss_loop
.Lbss_done:
    adrp    x1, .Lexception_vectors
    add     x1, x1, :lo12:.Lexception_vectors
    msr     vbar_el2, x1
    isb
    ret
.size aarch64_boot_clear_bss, . - aarch64_boot_clear_bss

/* Builds the low identity map and returns its root in x0. */
.type aarch64_boot_build_page_tables, %function
aarch64_boot_build_page_tables:
    adrp    x4, __boot_l1_table
    add     x5, x4, :lo12:__boot_l1_table
    adrp    x4, __boot_l0_table
    add     x4, x4, :lo12:__boot_l0_table
    mov     x1, x4
    mov     x2, #1024
.Lclear_table:
    str     xzr, [x1], #8
    subs    x2, x2, #1
    b.ne    .Lclear_table
    orr     x1, x5, #STAGE1_DESC_TABLE_OR_PAGE
    str     x1, [x4]
    ldr     x1, =BOOT_DEVICE_BLOCK_FLAGS
    str     x1, [x5]
    mov     x1, #1
    ldr     x2, =BOOT_NORMAL_BLOCK_FLAGS
.Lmap_ram:
    lsl     x3, x1, #30
    orr     x3, x3, x2
    str     x3, [x5, x1, lsl #3]
    add     x1, x1, #1
    cmp     x1, #4
    b.lo    .Lmap_ram
    mov     x0, x4
    ret
.size aarch64_boot_build_page_tables, . - aarch64_boot_build_page_tables

/* Activates the bootstrap identity map passed in x0. */
.type aarch64_boot_enable_translation, %function
aarch64_boot_enable_translation:
    dsb     sy
    ldr     x1, =MAIR_EL2_BOOT_VALUE
    msr     mair_el2, x1
    ldr     x1, =TCR_EL2_BOOT_BASE
    orr     x1, x1, x23, lsl #TCR_EL2_PS_SHIFT
    msr     tcr_el2, x1
    msr     ttbr0_el2, x0
    isb
    tlbi    alle2
    dsb     sy
    isb
    ldr     x1, =SCTLR_EL2_BOOT_VALUE
    msr     sctlr_el2, x1
    isb
    ret
.size aarch64_boot_enable_translation, . - aarch64_boot_enable_translation

/*
 * Switches from the bootstrap root to the final 48-bit address space and
 * enters Rust through the high kernel alias. This function never returns.
 *
 * x0 = final TTBR0_EL2 physical address
 * x1 = virtual load base of the kernel image
 * x2 = virtual top of the final boot stack
 * x3 = selected final TCR_EL2 value
 */
.global aarch64_activate_final_address_space
.type aarch64_activate_final_address_space, %function
aarch64_activate_final_address_space:
    msr     daifset, #DAIFSET_ALL
    mov     x19, x0
    mov     x20, x1
    mov     x21, x2
    mov     x23, x3
    adr     x22, _start

    // Rewrite static PIE pointers for the high kernel alias.
    adrp    x3, __rela_dyn_start
    add     x3, x3, :lo12:__rela_dyn_start
    adrp    x4, __rela_dyn_end
    add     x4, x4, :lo12:__rela_dyn_end
.Lvirtual_relocate:
    cmp     x3, x4
    b.hs    .Lvirtual_relocation_done
    ldr     x5, [x3]
    ldr     x6, [x3, #8]
    ldr     x7, [x3, #16]
    cmp     w6, #R_AARCH64_RELATIVE
    b.ne    .Lunsupported_boot
    add     x5, x22, x5
    add     x7, x20, x7
    str     x7, [x5]
    add     x3, x3, #24
    b       .Lvirtual_relocate
.Lvirtual_relocation_done:
    // RELR locations contain physical pointers after the load-bias pass.
    sub     x8, x20, x22
    adrp    x3, __relr_dyn_start
    add     x3, x3, :lo12:__relr_dyn_start
    adrp    x4, __relr_dyn_end
    add     x4, x4, :lo12:__relr_dyn_end
    mov     x5, xzr
.Lvirtual_relr_loop:
    cmp     x3, x4
    b.hs    .Lvirtual_relr_done
    ldr     x6, [x3], #8
    tbnz    x6, #0, .Lvirtual_relr_bitmap
    add     x5, x22, x6
    ldr     x7, [x5]
    add     x7, x7, x8
    str     x7, [x5], #8
    b       .Lvirtual_relr_loop
.Lvirtual_relr_bitmap:
    cbz     x5, .Lunsupported_boot
    lsr     x6, x6, #1
    mov     x9, #63
.Lvirtual_relr_bitmap_loop:
    tbz     x6, #0, .Lvirtual_relr_bitmap_next
    ldr     x7, [x5]
    add     x7, x7, x8
    str     x7, [x5]
.Lvirtual_relr_bitmap_next:
    add     x5, x5, #8
    lsr     x6, x6, #1
    subs    x9, x9, #1
    b.ne    .Lvirtual_relr_bitmap_loop
    b       .Lvirtual_relr_loop
.Lvirtual_relr_done:
    dsb     ishst

    // Install the configured VA width and boot-CPU-selected PA width with TTBR.
    msr     tcr_el2, x23
    msr     ttbr0_el2, x19
    isb
    tlbi    alle2
    dsb     ish
    isb

    // Move vectors, stack, and execution to their high kernel aliases.
    adr     x3, .Lexception_vectors
    sub     x3, x3, x22
    add     x3, x20, x3
    msr     vbar_el2, x3
    isb
    mov     sp, x21
    mov     x29, xzr
    mov     x30, xzr
    adr     x3, start_kernel
    sub     x3, x3, x22
    add     x3, x20, x3
    br      x3
.size aarch64_activate_final_address_space, . - aarch64_activate_final_address_space

.Lunsupported_boot:
.Lsecondary_halt:
    wfe
    b       .Lsecondary_halt

/*
 * PSCI secondary-CPU trampoline.
 *
 * x0 points to a physically addressed SecondaryBootParameters record:
 *   +0  final TTBR0_EL2 root
 *   +8  physical stack top used during the MMU transition
 *   +16 virtual stack top
 *   +24 logical CPU index
 *   +32 high Rust entry address
 *   +40 high runtime vector address
 *   +48 selected TCR_EL2 value
 */
.balign 16
.global aarch64_secondary_entry
.type aarch64_secondary_entry, %function
aarch64_secondary_entry:
    msr     daifset, #DAIFSET_ALL
    clrex
    mov     x19, x0

    mrs     x1, CurrentEL
    cmp     x1, #CURRENT_EL_EL2
    b.ne    .Lsecondary_halt

    // Reject an incompatible secondary before enabling its translation regime.
    ldr     x2, [x19, #48]
    ubfx    x3, x2, #TCR_EL2_PS_SHIFT, #3
    mrs     x4, id_aa64mmfr0_el1
    and     x5, x4, #ID_AA64MMFR0_PARANGE_MASK
    cmp     x5, x3
    b.lo    .Lsecondary_halt
    lsr     x5, x4, #ID_AA64MMFR0_TGRAN4_SHIFT
    and     x5, x5, #ID_AA64MMFR0_TGRAN4_MASK
    cmp     x5, #ID_AA64MMFR0_TGRAN4_UNSUPPORTED
    b.eq    .Lsecondary_halt
    msr     spsel, #1
    ldr     x1, [x19, #8]
    mov     sp, x1

    ldr     x1, =SCTLR_EL2_RES1
    msr     sctlr_el2, x1
    bl      aarch64_boot_initialize_el2

    ldr     x1, =MAIR_EL2_BOOT_VALUE
    msr     mair_el2, x1
    ldr     x1, [x19, #48]
    msr     tcr_el2, x1
    ldr     x1, [x19]
    msr     ttbr0_el2, x1
    isb
    tlbi    alle2
    dsb     sy
    isb
    ldr     x1, =SCTLR_EL2_BOOT_VALUE
    msr     sctlr_el2, x1
    isb

    ldr     x0, [x19, #24]
    msr     tpidr_el2, x0
    ldr     x1, [x19, #40]
    msr     vbar_el2, x1
    isb
    ldr     x1, [x19, #16]
    mov     sp, x1
    mov     x29, xzr
    mov     x30, xzr
    ldr     x1, [x19, #32]
    br      x1
.size aarch64_secondary_entry, . - aarch64_secondary_entry

/* Save early synchronous fault state for debugger inspection. */
.macro EARLY_VECTOR
    b       .Learly_exception
    .space  124
.endm

.balign 2048
.Lexception_vectors:
    .rept   16
    EARLY_VECTOR
    .endr

.Learly_exception:
    mrs     x0, esr_el2
    mrs     x1, elr_el2
    mrs     x2, far_el2
    mrs     x3, spsr_el2
    adrp    x4, __early_exception_state
    add     x4, x4, :lo12:__early_exception_state
    stp     x0, x1, [x4]
    stp     x2, x3, [x4, #16]
    dsb     sy
    b       .Lsecondary_halt

.section .bss.early_exception, "aw", %nobits
.balign 16
.global __early_exception_state
__early_exception_state:
    .skip   32
