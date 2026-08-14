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
    b       .Lprimary_entry
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

.section .text.boot, "ax"
.balign 16
.Lprimary_entry:
    msr     daifset, #0xf
    clrex

    // A hypervisor kernel requires EL2. EL1 fallback is intentionally rejected.
    mrs     x1, CurrentEL
    cmp     x1, #CURRENT_EL_EL2
    b.ne    .Lunsupported_boot

    // Firmware/PSCI should retain secondaries. Park any that enter defensively.
    mrs     x1, mpidr_el1
    ubfx    x2, x1, #32, #8
    and     x1, x1, #0x00ffffff
    orr     x1, x1, x2
    cbnz    x1, .Lsecondary_halt

    // Preserve the physical DTB address until the Rust ABI call.
    mov     x20, x0
    adr     x21, _start

    // Validate the fixed 4 KiB granule and 40-bit physical-address policy.
    mrs     x1, id_aa64mmfr0_el1
    and     x2, x1, #ID_AA64MMFR0_PARANGE_MASK
    cmp     x2, #ID_AA64MMFR0_PARANGE_40BIT
    b.lo    .Lunsupported_boot
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

    /*
     * Apply static PIE relative relocations before using Rust static data.
     * Every accepted relocation writes `load_bias + addend` to
     * `load_bias + offset`. Any non-relative relocation is a build defect.
     */
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

    // Initialize EL2 controls before installing translation state.
    ldr     x1, =HCR_EL2_BOOT_VALUE
    msr     hcr_el2, x1
    msr     cptr_el2, xzr
    msr     hstr_el2, xzr
    msr     mdcr_el2, xzr
    msr     vttbr_el2, xzr
    msr     tpidr_el2, xzr
    msr     cntvoff_el2, xzr
    msr     cnthctl_el2, xzr
    msr     cnthp_ctl_el2, xzr
    isb

    // Clear all Rust zero-initialized static storage.
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
    // Install exception vectors before any operation that can fault.
    adrp    x1, .Lexception_vectors
    add     x1, x1, :lo12:.Lexception_vectors
    msr     vbar_el2, x1
    isb

    // Build L0 and L1 tables for a 48-bit identity-mapped bootstrap VA.
    adrp    x4, __boot_l0_table
    add     x4, x4, :lo12:__boot_l0_table
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

    // L0[0] covers the low 512 GiB through the bootstrap L1 table.
    orr     x1, x5, #0x3
    str     x1, [x4]

    // 0..1 GiB: QEMU `virt` MMIO, Device-nGnRnE, execute-never.
    ldr     x1, =BOOT_DEVICE_BLOCK_FLAGS
    str     x1, [x5]

    // 1..4 GiB: QEMU RAM bootstrap window, Normal WBWA and executable.
    mov     x1, #1
    ldr     x2, =BOOT_NORMAL_BLOCK_FLAGS
.Lmap_ram:
    lsl     x3, x1, #30
    orr     x3, x3, x2
    str     x3, [x5, x1, lsl #3]
    add     x1, x1, #1
    cmp     x1, #4
    b.lo    .Lmap_ram

    dsb     sy
    ldr     x1, =MAIR_EL2_BOOT_VALUE
    msr     mair_el2, x1
    ldr     x1, =TCR_EL2_FINAL_VALUE
    msr     tcr_el2, x1
    msr     ttbr0_el2, x4
    isb
    tlbi    alle2
    dsb     sy
    isb

    // Enable the identity map and caches. Execution continues at the same VA.
    ldr     x1, =SCTLR_EL2_BOOT_VALUE
    msr     sctlr_el2, x1
    isb

    adrp    x1, __boot_stack_top
    add     x1, x1, :lo12:__boot_stack_top
    mov     sp, x1
    mov     x29, xzr
    mov     x30, xzr
    mov     x0, x20
    bl      aarch64_rust_entry

/*
 * Switches from the bootstrap root to the final 48-bit address space and
 * enters Rust through the high kernel alias. This function never returns.
 *
 * x0 = final TTBR0_EL2 physical address
 * x1 = virtual load base of the kernel image
 * x2 = virtual top of the final boot stack
 */
.global aarch64_activate_final_address_space
.type aarch64_activate_final_address_space, %function
aarch64_activate_final_address_space:
    msr     daifset, #0xf
    mov     x19, x0
    mov     x20, x1
    mov     x21, x2
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
    dsb     ishst

    // TCR and MAIR are unchanged; replacing TTBR is therefore atomic here.
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
    adr     x3, aarch64_virtual_entry
    sub     x3, x3, x22
    add     x3, x20, x3
    br      x3

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
 */
.balign 16
.global aarch64_secondary_entry
.type aarch64_secondary_entry, %function
aarch64_secondary_entry:
    msr     daifset, #0xf
    clrex
    mov     x19, x0

    mrs     x1, CurrentEL
    cmp     x1, #CURRENT_EL_EL2
    b.ne    .Lsecondary_halt
    msr     spsel, #1
    ldr     x1, [x19, #8]
    mov     sp, x1

    ldr     x1, =SCTLR_EL2_RES1
    msr     sctlr_el2, x1
    ldr     x1, =HCR_EL2_BOOT_VALUE
    msr     hcr_el2, x1
    msr     cptr_el2, xzr
    msr     hstr_el2, xzr
    msr     mdcr_el2, xzr
    msr     vttbr_el2, xzr
    msr     cntvoff_el2, xzr
    msr     cnthctl_el2, xzr
    msr     cnthp_ctl_el2, xzr
    isb

    ldr     x1, =MAIR_EL2_BOOT_VALUE
    msr     mair_el2, x1
    ldr     x1, =TCR_EL2_FINAL_VALUE
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
