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
