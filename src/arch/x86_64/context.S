/*
 * SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 */

#include "asm_constants.h"

    .section .text.context, "ax"
    .code64

    .global x86_64_switch_context
    .type x86_64_switch_context, @function
x86_64_switch_context:
    movq %rbx, THREAD_CONTEXT_RBX_OFFSET(%rdi)
    movq %rbp, THREAD_CONTEXT_RBP_OFFSET(%rdi)
    movq %r12, THREAD_CONTEXT_R12_OFFSET(%rdi)
    movq %r13, THREAD_CONTEXT_R13_OFFSET(%rdi)
    movq %r14, THREAD_CONTEXT_R14_OFFSET(%rdi)
    movq %r15, THREAD_CONTEXT_R15_OFFSET(%rdi)
    movq %rsp, THREAD_CONTEXT_RSP_OFFSET(%rdi)
    leaq .Lswitch_return(%rip), %rax
    movq %rax, THREAD_CONTEXT_RIP_OFFSET(%rdi)

    movq THREAD_CONTEXT_RBX_OFFSET(%rsi), %rbx
    movq THREAD_CONTEXT_RBP_OFFSET(%rsi), %rbp
    movq THREAD_CONTEXT_R12_OFFSET(%rsi), %r12
    movq THREAD_CONTEXT_R13_OFFSET(%rsi), %r13
    movq THREAD_CONTEXT_R14_OFFSET(%rsi), %r14
    movq THREAD_CONTEXT_R15_OFFSET(%rsi), %r15
    movq THREAD_CONTEXT_RSP_OFFSET(%rsi), %rsp
    jmp *THREAD_CONTEXT_RIP_OFFSET(%rsi)
.Lswitch_return:
    ret
    .size x86_64_switch_context, . - x86_64_switch_context

    .global x86_64_thread_trampoline
    .type x86_64_thread_trampoline, @function
x86_64_thread_trampoline:
    movq %r13, %rdi
    call *%r12
    call kernel_thread_exit
1:  cli
    hlt
    jmp 1b
    .size x86_64_thread_trampoline, . - x86_64_thread_trampoline

    .global x86_64_reset_stack_and_enter
    .type x86_64_reset_stack_and_enter, @function
x86_64_reset_stack_and_enter:
    movq %rsi, %rsp
    xorq %rbp, %rbp
    movq %rcx, (%rdi)
    addq $8, %rdi
1:  cmpq %rsi, %rdi
    jae 2f
    movq %rdx, (%rdi)
    addq $8, %rdi
    jmp 1b
2:  # Enter Rust with the stack shape produced by a normal SysV call.
    subq $8, %rsp
    movq $0, (%rsp)
    movq %r9, %rdi
    jmp *%r8
    .size x86_64_reset_stack_and_enter, . - x86_64_reset_stack_and_enter

    .global x86_64_run_on_stack
    .type x86_64_run_on_stack, @function
x86_64_run_on_stack:
    movq %rdi, %rsp
    andq $-16, %rsp
    # The callback never returns, but its compiler-generated prologue still
    # requires the SysV entry alignment that follows a pushed return address.
    subq $8, %rsp
    movq $0, (%rsp)
    xorq %rbp, %rbp
    movq %rsi, %rax
    movq %rdx, %rdi
    jmp *%rax
    .size x86_64_run_on_stack, . - x86_64_run_on_stack
