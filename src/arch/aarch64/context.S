/* AAPCS64 cooperative kernel-thread context switch. */

#include "asm_constants.h"

.section .text.context, "ax"

.global aarch64_switch_context
.type aarch64_switch_context, %function
aarch64_switch_context:
    stp     x19, x20, [x0, #(THREAD_CONTEXT_X19_OFFSET + 0)]
    stp     x21, x22, [x0, #(THREAD_CONTEXT_X19_OFFSET + 16)]
    stp     x23, x24, [x0, #(THREAD_CONTEXT_X19_OFFSET + 32)]
    stp     x25, x26, [x0, #(THREAD_CONTEXT_X19_OFFSET + 48)]
    stp     x27, x28, [x0, #(THREAD_CONTEXT_X19_OFFSET + 64)]
    stp     x29, x30, [x0, #THREAD_CONTEXT_X29_OFFSET]
    mov     x2, sp
    str     x2, [x0, #THREAD_CONTEXT_SP_OFFSET]
    stp     d8, d9, [x0, #(THREAD_CONTEXT_D8_OFFSET + 0)]
    stp     d10, d11, [x0, #(THREAD_CONTEXT_D8_OFFSET + 16)]
    stp     d12, d13, [x0, #(THREAD_CONTEXT_D8_OFFSET + 32)]
    stp     d14, d15, [x0, #(THREAD_CONTEXT_D8_OFFSET + 48)]
    mrs     x2, fpcr
    str     x2, [x0, #THREAD_CONTEXT_FPCR_OFFSET]
    mrs     x2, fpsr
    str     x2, [x0, #THREAD_CONTEXT_FPSR_OFFSET]

    ldp     x19, x20, [x1, #(THREAD_CONTEXT_X19_OFFSET + 0)]
    ldp     x21, x22, [x1, #(THREAD_CONTEXT_X19_OFFSET + 16)]
    ldp     x23, x24, [x1, #(THREAD_CONTEXT_X19_OFFSET + 32)]
    ldp     x25, x26, [x1, #(THREAD_CONTEXT_X19_OFFSET + 48)]
    ldp     x27, x28, [x1, #(THREAD_CONTEXT_X19_OFFSET + 64)]
    ldp     x29, x30, [x1, #THREAD_CONTEXT_X29_OFFSET]
    ldr     x2, [x1, #THREAD_CONTEXT_SP_OFFSET]
    mov     sp, x2
    ldp     d8, d9, [x1, #(THREAD_CONTEXT_D8_OFFSET + 0)]
    ldp     d10, d11, [x1, #(THREAD_CONTEXT_D8_OFFSET + 16)]
    ldp     d12, d13, [x1, #(THREAD_CONTEXT_D8_OFFSET + 32)]
    ldp     d14, d15, [x1, #(THREAD_CONTEXT_D8_OFFSET + 48)]
    ldr     x2, [x1, #THREAD_CONTEXT_FPCR_OFFSET]
    msr     fpcr, x2
    ldr     x2, [x1, #THREAD_CONTEXT_FPSR_OFFSET]
    msr     fpsr, x2
    ret
.size aarch64_switch_context, . - aarch64_switch_context

.global aarch64_thread_trampoline
.type aarch64_thread_trampoline, %function
aarch64_thread_trampoline:
    mov     x0, x20
    blr     x19
    bl      kernel_thread_exit
    b       .
.size aarch64_thread_trampoline, . - aarch64_thread_trampoline

/* Captures a best-effort kernel register image for software panic paths. */
.global aarch64_capture_crash_context
.type aarch64_capture_crash_context, %function
aarch64_capture_crash_context:
    stp     x1, x2, [x0, #(CRASH_CONTEXT_X0_OFFSET + 8)]
    stp     x3, x4, [x0, #(CRASH_CONTEXT_X0_OFFSET + 24)]
    stp     x5, x6, [x0, #(CRASH_CONTEXT_X0_OFFSET + 40)]
    stp     x7, x8, [x0, #(CRASH_CONTEXT_X0_OFFSET + 56)]
    stp     x9, x10, [x0, #(CRASH_CONTEXT_X0_OFFSET + 72)]
    stp     x11, x12, [x0, #(CRASH_CONTEXT_X0_OFFSET + 88)]
    stp     x13, x14, [x0, #(CRASH_CONTEXT_X0_OFFSET + 104)]
    stp     x15, x16, [x0, #(CRASH_CONTEXT_X0_OFFSET + 120)]
    stp     x17, x18, [x0, #(CRASH_CONTEXT_X0_OFFSET + 136)]
    stp     x19, x20, [x0, #(CRASH_CONTEXT_X0_OFFSET + 152)]
    stp     x21, x22, [x0, #(CRASH_CONTEXT_X0_OFFSET + 168)]
    stp     x23, x24, [x0, #(CRASH_CONTEXT_X0_OFFSET + 184)]
    stp     x25, x26, [x0, #(CRASH_CONTEXT_X0_OFFSET + 200)]
    stp     x27, x28, [x0, #(CRASH_CONTEXT_X0_OFFSET + 216)]
    stp     x29, x30, [x0, #(CRASH_CONTEXT_X0_OFFSET + 232)]
    mov     x1, sp
    str     x1, [x0, #CRASH_CONTEXT_SP_OFFSET]
    str     x30, [x0, #CRASH_CONTEXT_PC_OFFSET]
    mrs     x1, nzcv
    mrs     x2, daif
    orr     x1, x1, x2
    mrs     x2, CurrentEL
    add     x3, x2, #1
    orr     x1, x1, x3
    str     x1, [x0, #CRASH_CONTEXT_PSTATE_OFFSET]
    mrs     x1, mpidr_el1
    str     x1, [x0, #CRASH_CONTEXT_MPIDR_OFFSET]
    str     x2, [x0, #CRASH_CONTEXT_CURRENT_EL_OFFSET]
    mrs     x1, daif
    str     x1, [x0, #CRASH_CONTEXT_DAIF_OFFSET]
    mrs     x1, sctlr_el2
    str     x1, [x0, #CRASH_CONTEXT_SCTLR_EL2_OFFSET]
    mrs     x1, tcr_el2
    str     x1, [x0, #CRASH_CONTEXT_TCR_EL2_OFFSET]
    mrs     x1, ttbr0_el2
    str     x1, [x0, #CRASH_CONTEXT_TTBR0_EL2_OFFSET]
    mrs     x1, vbar_el2
    str     x1, [x0, #CRASH_CONTEXT_VBAR_EL2_OFFSET]
    mrs     x1, hcr_el2
    str     x1, [x0, #CRASH_CONTEXT_HCR_EL2_OFFSET]
    ret
.size aarch64_capture_crash_context, . - aarch64_capture_crash_context

.global aarch64_enter_guest
.type aarch64_enter_guest, %function
aarch64_enter_guest:
    // Keep the context pointer in x30 until every other guest register is live.
    mov     x30, x0
    ldr     x1, [x30, #VCPU_CONTEXT_PC_OFFSET]
    msr     elr_el2, x1
    ldr     x1, [x30, #VCPU_CONTEXT_PSTATE_OFFSET]
    msr     spsr_el2, x1
    ldp     x0, x1, [x30, #(VCPU_CONTEXT_X0_OFFSET + 0)]
    ldp     x2, x3, [x30, #(VCPU_CONTEXT_X0_OFFSET + 16)]
    ldp     x4, x5, [x30, #(VCPU_CONTEXT_X0_OFFSET + 32)]
    ldp     x6, x7, [x30, #(VCPU_CONTEXT_X0_OFFSET + 48)]
    ldp     x8, x9, [x30, #(VCPU_CONTEXT_X0_OFFSET + 64)]
    ldp     x10, x11, [x30, #(VCPU_CONTEXT_X0_OFFSET + 80)]
    ldp     x12, x13, [x30, #(VCPU_CONTEXT_X0_OFFSET + 96)]
    ldp     x14, x15, [x30, #(VCPU_CONTEXT_X0_OFFSET + 112)]
    ldp     x16, x17, [x30, #(VCPU_CONTEXT_X0_OFFSET + 128)]
    ldp     x18, x19, [x30, #(VCPU_CONTEXT_X0_OFFSET + 144)]
    ldp     x20, x21, [x30, #(VCPU_CONTEXT_X0_OFFSET + 160)]
    ldp     x22, x23, [x30, #(VCPU_CONTEXT_X0_OFFSET + 176)]
    ldp     x24, x25, [x30, #(VCPU_CONTEXT_X0_OFFSET + 192)]
    ldp     x26, x27, [x30, #(VCPU_CONTEXT_X0_OFFSET + 208)]
    ldp     x28, x29, [x30, #(VCPU_CONTEXT_X0_OFFSET + 224)]
    ldr     x30, [x30, #VCPU_CONTEXT_X30_OFFSET]
    eret
.size aarch64_enter_guest, . - aarch64_enter_guest
