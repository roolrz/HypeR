/*
 * SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 */

#include "asm_constants.h"

.section .text.native, "ax"
.balign 4
.global riscv64_run_native_user
.type riscv64_run_native_user, @function
riscv64_run_native_user:
    # The caller owns a masked CPU and active Sv39 root. The anchor is private
    # kernel storage; only the final U return publishes it through sscratch.
    addi sp, sp, -USER_ANCHOR_SIZE
    sd ra, 0(sp)
    sd s0, 8(sp)
    sd s1, 16(sp)
    sd s2, 24(sp)
    sd s3, 32(sp)
    sd s4, 40(sp)
    sd s5, 48(sp)
    sd s6, 56(sp)
    sd s7, 64(sp)
    sd s8, 72(sp)
    sd s9, 80(sp)
    sd s10, 88(sp)
    sd s11, 96(sp)
    sd gp, USER_ANCHOR_GP_OFFSET(sp)
    sd tp, USER_ANCHOR_TP_OFFSET(sp)
    sd a0, USER_ANCHOR_CONTEXT_OFFSET(sp)
    csrr t0, sstatus
    sd t0, USER_ANCHOR_STATUS_OFFSET(sp)
    csrr t0, stvec
    sd t0, USER_ANCHOR_VECTOR_OFFSET(sp)
    csrr t0, hstatus
    sd t0, USER_ANCHOR_HSTATUS_OFFSET(sp)
    csrr t0, scounteren
    sd t0, USER_ANCHOR_COUNTER_OFFSET(sp)
    csrr t0, senvcfg
    sd t0, USER_ANCHOR_ENVCFG_OFFSET(sp)
    li t0, (3 << 13)
    csrs sstatus, t0
    fsd f0, USER_ANCHOR_FLOATING_OFFSET + 0(sp)
    fsd f1, USER_ANCHOR_FLOATING_OFFSET + 8(sp)
    fsd f2, USER_ANCHOR_FLOATING_OFFSET + 16(sp)
    fsd f3, USER_ANCHOR_FLOATING_OFFSET + 24(sp)
    fsd f4, USER_ANCHOR_FLOATING_OFFSET + 32(sp)
    fsd f5, USER_ANCHOR_FLOATING_OFFSET + 40(sp)
    fsd f6, USER_ANCHOR_FLOATING_OFFSET + 48(sp)
    fsd f7, USER_ANCHOR_FLOATING_OFFSET + 56(sp)
    fsd f8, USER_ANCHOR_FLOATING_OFFSET + 64(sp)
    fsd f9, USER_ANCHOR_FLOATING_OFFSET + 72(sp)
    fsd f10, USER_ANCHOR_FLOATING_OFFSET + 80(sp)
    fsd f11, USER_ANCHOR_FLOATING_OFFSET + 88(sp)
    fsd f12, USER_ANCHOR_FLOATING_OFFSET + 96(sp)
    fsd f13, USER_ANCHOR_FLOATING_OFFSET + 104(sp)
    fsd f14, USER_ANCHOR_FLOATING_OFFSET + 112(sp)
    fsd f15, USER_ANCHOR_FLOATING_OFFSET + 120(sp)
    fsd f16, USER_ANCHOR_FLOATING_OFFSET + 128(sp)
    fsd f17, USER_ANCHOR_FLOATING_OFFSET + 136(sp)
    fsd f18, USER_ANCHOR_FLOATING_OFFSET + 144(sp)
    fsd f19, USER_ANCHOR_FLOATING_OFFSET + 152(sp)
    fsd f20, USER_ANCHOR_FLOATING_OFFSET + 160(sp)
    fsd f21, USER_ANCHOR_FLOATING_OFFSET + 168(sp)
    fsd f22, USER_ANCHOR_FLOATING_OFFSET + 176(sp)
    fsd f23, USER_ANCHOR_FLOATING_OFFSET + 184(sp)
    fsd f24, USER_ANCHOR_FLOATING_OFFSET + 192(sp)
    fsd f25, USER_ANCHOR_FLOATING_OFFSET + 200(sp)
    fsd f26, USER_ANCHOR_FLOATING_OFFSET + 208(sp)
    fsd f27, USER_ANCHOR_FLOATING_OFFSET + 216(sp)
    fsd f28, USER_ANCHOR_FLOATING_OFFSET + 224(sp)
    fsd f29, USER_ANCHOR_FLOATING_OFFSET + 232(sp)
    fsd f30, USER_ANCHOR_FLOATING_OFFSET + 240(sp)
    fsd f31, USER_ANCHOR_FLOATING_OFFSET + 248(sp)
    frcsr t0
    sd t0, USER_ANCHOR_FCSR_OFFSET(sp)
    csrw scounteren, zero
    csrw senvcfg, zero
    # HU must be clear: Native applications have no authority to issue HLV/HSV.
    li t0, HSTATUS_NATIVE_CLEAR
    csrc hstatus, t0
    # Initialize the reusable trap frame from the pinned stopped context.
    mv t0, a0
    addi sp, sp, -USER_FRAME_SIZE
    mv t1, sp
    li t2, USER_FRAME_SIZE
1:
    ld t3, 0(t0)
    sd t3, 0(t1)
    addi t0, t0, 8
    addi t1, t1, 8
    addi t2, t2, -8
    bnez t2, 1b
    # Data coherence does not imply instruction-fetch coherence.
    fence.i
    j .Lnative_restore
.size riscv64_run_native_user, . - riscv64_run_native_user

.balign 4
.global riscv64_native_vector
.type riscv64_native_vector, @function
riscv64_native_vector:
    csrrw sp, sscratch, sp
    addi sp, sp, -USER_FRAME_SIZE
    sd zero, 0(sp)
    sd ra, 8(sp)
    sd gp, 24(sp)
    sd tp, 32(sp)
    sd t0, 40(sp)
    sd t1, 48(sp)
    sd t2, 56(sp)
    sd s0, 64(sp)
    sd s1, 72(sp)
    sd a0, 80(sp)
    sd a1, 88(sp)
    sd a2, 96(sp)
    sd a3, 104(sp)
    sd a4, 112(sp)
    sd a5, 120(sp)
    sd a6, 128(sp)
    sd a7, 136(sp)
    sd s2, 144(sp)
    sd s3, 152(sp)
    sd s4, 160(sp)
    sd s5, 168(sp)
    sd s6, 176(sp)
    sd s7, 184(sp)
    sd s8, 192(sp)
    sd s9, 200(sp)
    sd s10, 208(sp)
    sd s11, 216(sp)
    sd t3, 224(sp)
    sd t4, 232(sp)
    sd t5, 240(sp)
    sd t6, 248(sp)
    csrr t0, sscratch
    sd t0, 16(sp)
    addi t0, sp, USER_FRAME_SIZE
    ld gp, USER_ANCHOR_GP_OFFSET(t0)
    ld tp, USER_ANCHOR_TP_OFFSET(t0)
    csrw sscratch, zero
    # Reestablish the host vector before any Rust call or nested exception.
    la t0, riscv64_trap_vector
    csrw stvec, t0
    csrr t0, sepc
    sd t0, USER_FRAME_PC_OFFSET(sp)
    csrr t0, sstatus
    sd t0, USER_FRAME_STATUS_OFFSET(sp)
    csrr t0, scause
    sd t0, USER_FRAME_CAUSE_OFFSET(sp)
    csrr t0, stval
    sd t0, USER_FRAME_FAULT_OFFSET(sp)
    li t0, (3 << 13)
    csrs sstatus, t0
    fsd f0, USER_FRAME_FLOATING_OFFSET + 0(sp)
    fsd f1, USER_FRAME_FLOATING_OFFSET + 8(sp)
    fsd f2, USER_FRAME_FLOATING_OFFSET + 16(sp)
    fsd f3, USER_FRAME_FLOATING_OFFSET + 24(sp)
    fsd f4, USER_FRAME_FLOATING_OFFSET + 32(sp)
    fsd f5, USER_FRAME_FLOATING_OFFSET + 40(sp)
    fsd f6, USER_FRAME_FLOATING_OFFSET + 48(sp)
    fsd f7, USER_FRAME_FLOATING_OFFSET + 56(sp)
    fsd f8, USER_FRAME_FLOATING_OFFSET + 64(sp)
    fsd f9, USER_FRAME_FLOATING_OFFSET + 72(sp)
    fsd f10, USER_FRAME_FLOATING_OFFSET + 80(sp)
    fsd f11, USER_FRAME_FLOATING_OFFSET + 88(sp)
    fsd f12, USER_FRAME_FLOATING_OFFSET + 96(sp)
    fsd f13, USER_FRAME_FLOATING_OFFSET + 104(sp)
    fsd f14, USER_FRAME_FLOATING_OFFSET + 112(sp)
    fsd f15, USER_FRAME_FLOATING_OFFSET + 120(sp)
    fsd f16, USER_FRAME_FLOATING_OFFSET + 128(sp)
    fsd f17, USER_FRAME_FLOATING_OFFSET + 136(sp)
    fsd f18, USER_FRAME_FLOATING_OFFSET + 144(sp)
    fsd f19, USER_FRAME_FLOATING_OFFSET + 152(sp)
    fsd f20, USER_FRAME_FLOATING_OFFSET + 160(sp)
    fsd f21, USER_FRAME_FLOATING_OFFSET + 168(sp)
    fsd f22, USER_FRAME_FLOATING_OFFSET + 176(sp)
    fsd f23, USER_FRAME_FLOATING_OFFSET + 184(sp)
    fsd f24, USER_FRAME_FLOATING_OFFSET + 192(sp)
    fsd f25, USER_FRAME_FLOATING_OFFSET + 200(sp)
    fsd f26, USER_FRAME_FLOATING_OFFSET + 208(sp)
    fsd f27, USER_FRAME_FLOATING_OFFSET + 216(sp)
    fsd f28, USER_FRAME_FLOATING_OFFSET + 224(sp)
    fsd f29, USER_FRAME_FLOATING_OFFSET + 232(sp)
    fsd f30, USER_FRAME_FLOATING_OFFSET + 240(sp)
    fsd f31, USER_FRAME_FLOATING_OFFSET + 248(sp)
    frcsr t0
    sd t0, USER_FRAME_FCSR_OFFSET(sp)
    sd zero, USER_FRAME_FCSR_OFFSET + 8(sp)
    addi t0, sp, USER_FRAME_SIZE
    ld gp, USER_ANCHOR_GP_OFFSET(t0)
    ld tp, USER_ANCHOR_TP_OFFSET(t0)
    fld f0, USER_ANCHOR_FLOATING_OFFSET + 0(t0)
    fld f1, USER_ANCHOR_FLOATING_OFFSET + 8(t0)
    fld f2, USER_ANCHOR_FLOATING_OFFSET + 16(t0)
    fld f3, USER_ANCHOR_FLOATING_OFFSET + 24(t0)
    fld f4, USER_ANCHOR_FLOATING_OFFSET + 32(t0)
    fld f5, USER_ANCHOR_FLOATING_OFFSET + 40(t0)
    fld f6, USER_ANCHOR_FLOATING_OFFSET + 48(t0)
    fld f7, USER_ANCHOR_FLOATING_OFFSET + 56(t0)
    fld f8, USER_ANCHOR_FLOATING_OFFSET + 64(t0)
    fld f9, USER_ANCHOR_FLOATING_OFFSET + 72(t0)
    fld f10, USER_ANCHOR_FLOATING_OFFSET + 80(t0)
    fld f11, USER_ANCHOR_FLOATING_OFFSET + 88(t0)
    fld f12, USER_ANCHOR_FLOATING_OFFSET + 96(t0)
    fld f13, USER_ANCHOR_FLOATING_OFFSET + 104(t0)
    fld f14, USER_ANCHOR_FLOATING_OFFSET + 112(t0)
    fld f15, USER_ANCHOR_FLOATING_OFFSET + 120(t0)
    fld f16, USER_ANCHOR_FLOATING_OFFSET + 128(t0)
    fld f17, USER_ANCHOR_FLOATING_OFFSET + 136(t0)
    fld f18, USER_ANCHOR_FLOATING_OFFSET + 144(t0)
    fld f19, USER_ANCHOR_FLOATING_OFFSET + 152(t0)
    fld f20, USER_ANCHOR_FLOATING_OFFSET + 160(t0)
    fld f21, USER_ANCHOR_FLOATING_OFFSET + 168(t0)
    fld f22, USER_ANCHOR_FLOATING_OFFSET + 176(t0)
    fld f23, USER_ANCHOR_FLOATING_OFFSET + 184(t0)
    fld f24, USER_ANCHOR_FLOATING_OFFSET + 192(t0)
    fld f25, USER_ANCHOR_FLOATING_OFFSET + 200(t0)
    fld f26, USER_ANCHOR_FLOATING_OFFSET + 208(t0)
    fld f27, USER_ANCHOR_FLOATING_OFFSET + 216(t0)
    fld f28, USER_ANCHOR_FLOATING_OFFSET + 224(t0)
    fld f29, USER_ANCHOR_FLOATING_OFFSET + 232(t0)
    fld f30, USER_ANCHOR_FLOATING_OFFSET + 240(t0)
    fld f31, USER_ANCHOR_FLOATING_OFFSET + 248(t0)
    ld t1, USER_ANCHOR_FCSR_OFFSET(t0)
    fscsr t1
    ld t1, USER_ANCHOR_COUNTER_OFFSET(t0)
    csrw scounteren, t1
    ld t1, USER_ANCHOR_ENVCFG_OFFSET(t0)
    csrw senvcfg, t1
    ld t1, USER_ANCHOR_HSTATUS_OFFSET(t0)
    csrw hstatus, t1
    ld t1, USER_ANCHOR_STATUS_OFFSET(t0)
    csrw sstatus, t1
    mv a0, sp
    call riscv64_native_trap_dispatch
    bnez a0, riscv64_unwind_native_user
.Lnative_restore:
    # Only masked assembly installs a Native return regime. Rust always sees
    # the host vector, zero sscratch, and privileged access protection.
    li t0, (3 << 13)
    csrs sstatus, t0
    fld f0, USER_FRAME_FLOATING_OFFSET + 0(sp)
    fld f1, USER_FRAME_FLOATING_OFFSET + 8(sp)
    fld f2, USER_FRAME_FLOATING_OFFSET + 16(sp)
    fld f3, USER_FRAME_FLOATING_OFFSET + 24(sp)
    fld f4, USER_FRAME_FLOATING_OFFSET + 32(sp)
    fld f5, USER_FRAME_FLOATING_OFFSET + 40(sp)
    fld f6, USER_FRAME_FLOATING_OFFSET + 48(sp)
    fld f7, USER_FRAME_FLOATING_OFFSET + 56(sp)
    fld f8, USER_FRAME_FLOATING_OFFSET + 64(sp)
    fld f9, USER_FRAME_FLOATING_OFFSET + 72(sp)
    fld f10, USER_FRAME_FLOATING_OFFSET + 80(sp)
    fld f11, USER_FRAME_FLOATING_OFFSET + 88(sp)
    fld f12, USER_FRAME_FLOATING_OFFSET + 96(sp)
    fld f13, USER_FRAME_FLOATING_OFFSET + 104(sp)
    fld f14, USER_FRAME_FLOATING_OFFSET + 112(sp)
    fld f15, USER_FRAME_FLOATING_OFFSET + 120(sp)
    fld f16, USER_FRAME_FLOATING_OFFSET + 128(sp)
    fld f17, USER_FRAME_FLOATING_OFFSET + 136(sp)
    fld f18, USER_FRAME_FLOATING_OFFSET + 144(sp)
    fld f19, USER_FRAME_FLOATING_OFFSET + 152(sp)
    fld f20, USER_FRAME_FLOATING_OFFSET + 160(sp)
    fld f21, USER_FRAME_FLOATING_OFFSET + 168(sp)
    fld f22, USER_FRAME_FLOATING_OFFSET + 176(sp)
    fld f23, USER_FRAME_FLOATING_OFFSET + 184(sp)
    fld f24, USER_FRAME_FLOATING_OFFSET + 192(sp)
    fld f25, USER_FRAME_FLOATING_OFFSET + 200(sp)
    fld f26, USER_FRAME_FLOATING_OFFSET + 208(sp)
    fld f27, USER_FRAME_FLOATING_OFFSET + 216(sp)
    fld f28, USER_FRAME_FLOATING_OFFSET + 224(sp)
    fld f29, USER_FRAME_FLOATING_OFFSET + 232(sp)
    fld f30, USER_FRAME_FLOATING_OFFSET + 240(sp)
    fld f31, USER_FRAME_FLOATING_OFFSET + 248(sp)
    ld t0, USER_FRAME_FCSR_OFFSET(sp)
    fscsr t0
    ld t0, USER_FRAME_PC_OFFSET(sp)
    csrw sepc, t0
    ld t0, USER_FRAME_STATUS_OFFSET(sp)
    csrw sstatus, t0
    li t0, HSTATUS_NATIVE_CLEAR
    csrc hstatus, t0
    csrw scounteren, zero
    csrw senvcfg, zero
    la t0, riscv64_native_vector
    csrw stvec, t0
    addi t0, sp, USER_FRAME_SIZE
    csrw sscratch, t0
    ld ra, 8(sp)
    ld gp, 24(sp)
    ld tp, 32(sp)
    ld t0, 40(sp)
    ld t1, 48(sp)
    ld t2, 56(sp)
    ld s0, 64(sp)
    ld s1, 72(sp)
    ld a0, 80(sp)
    ld a1, 88(sp)
    ld a2, 96(sp)
    ld a3, 104(sp)
    ld a4, 112(sp)
    ld a5, 120(sp)
    ld a6, 128(sp)
    ld a7, 136(sp)
    ld s2, 144(sp)
    ld s3, 152(sp)
    ld s4, 160(sp)
    ld s5, 168(sp)
    ld s6, 176(sp)
    ld s7, 184(sp)
    ld s8, 192(sp)
    ld s9, 200(sp)
    ld s10, 208(sp)
    ld s11, 216(sp)
    ld t3, 224(sp)
    ld t4, 232(sp)
    ld t5, 240(sp)
    ld t6, 248(sp)
    ld sp, 16(sp)
    sret
.size riscv64_native_vector, . - riscv64_native_vector

.global riscv64_unwind_native_user
.type riscv64_unwind_native_user, @function
riscv64_unwind_native_user:
    # The dispatcher copied all owned state and closed publication. No raw
    # frame or borrowed service crosses the restored host continuation.
    addi sp, sp, USER_FRAME_SIZE
    csrw sscratch, zero
    ld t0, USER_ANCHOR_VECTOR_OFFSET(sp)
    csrw stvec, t0
    ld ra, 0(sp)
    ld s0, 8(sp)
    ld s1, 16(sp)
    ld s2, 24(sp)
    ld s3, 32(sp)
    ld s4, 40(sp)
    ld s5, 48(sp)
    ld s6, 56(sp)
    ld s7, 64(sp)
    ld s8, 72(sp)
    ld s9, 80(sp)
    ld s10, 88(sp)
    ld s11, 96(sp)
    ld gp, USER_ANCHOR_GP_OFFSET(sp)
    ld tp, USER_ANCHOR_TP_OFFSET(sp)
    addi sp, sp, USER_ANCHOR_SIZE
    ret
.size riscv64_unwind_native_user, . - riscv64_unwind_native_user

.global riscv64_call_native_irq_on_stack
.type riscv64_call_native_irq_on_stack, @function
riscv64_call_native_irq_on_stack:
    # Native dispatch returns one scalar; the host/guest trap-stack helper has
    # a separate two-register action ABI and cannot share its Rust declaration.
    addi a1, a1, -16
    sd sp, 0(a1)
    sd ra, 8(a1)
    mv sp, a1
    jalr a2
    ld t0, 0(sp)
    ld ra, 8(sp)
    mv sp, t0
    ret
.size riscv64_call_native_irq_on_stack, . - riscv64_call_native_irq_on_stack

/* Raw user payload copied into an RX VMO by kernel self-tests. Every branch
 * is relative, so no kernel address or linker relocation enters userspace. */
.section .text.native_register_test, "ax"
.balign 4
.global riscv64_native_register_test_start
.global riscv64_native_register_test_end
riscv64_native_register_test_start:
    ori s0, a0, 256
    addi gp, s0, 17
    addi tp, s0, 23
    addi t0, s0, 31
    addi s2, s0, 40
    addi s3, s0, 41
    addi s4, s0, 42
    addi s5, s0, 43
    addi s6, s0, 44
    addi s7, s0, 45
    addi s8, s0, 46
    addi s9, s0, 47
    addi s10, s0, 48
    addi s11, s0, 49
    addi a3, s0, 50
    addi a4, s0, 51
    addi a5, s0, 52
    addi a6, s0, 53
    li t1, 0x61
    fscsr t1
    addi t1, s0, 128
    fmv.d.x f0, t1
    addi t1, t1, 1
    fmv.d.x f1, t1
    addi t1, t1, 1
    fmv.d.x f2, t1
    addi t1, t1, 1
    fmv.d.x f3, t1
    addi t1, t1, 1
    fmv.d.x f4, t1
    addi t1, t1, 1
    fmv.d.x f5, t1
    addi t1, t1, 1
    fmv.d.x f6, t1
    addi t1, t1, 1
    fmv.d.x f7, t1
    addi t1, t1, 1
    fmv.d.x f8, t1
    addi t1, t1, 1
    fmv.d.x f9, t1
    addi t1, t1, 1
    fmv.d.x f10, t1
    addi t1, t1, 1
    fmv.d.x f11, t1
    addi t1, t1, 1
    fmv.d.x f12, t1
    addi t1, t1, 1
    fmv.d.x f13, t1
    addi t1, t1, 1
    fmv.d.x f14, t1
    addi t1, t1, 1
    fmv.d.x f15, t1
    addi t1, t1, 1
    fmv.d.x f16, t1
    addi t1, t1, 1
    fmv.d.x f17, t1
    addi t1, t1, 1
    fmv.d.x f18, t1
    addi t1, t1, 1
    fmv.d.x f19, t1
    addi t1, t1, 1
    fmv.d.x f20, t1
    addi t1, t1, 1
    fmv.d.x f21, t1
    addi t1, t1, 1
    fmv.d.x f22, t1
    addi t1, t1, 1
    fmv.d.x f23, t1
    addi t1, t1, 1
    fmv.d.x f24, t1
    addi t1, t1, 1
    fmv.d.x f25, t1
    addi t1, t1, 1
    fmv.d.x f26, t1
    addi t1, t1, 1
    fmv.d.x f27, t1
    addi t1, t1, 1
    fmv.d.x f28, t1
    addi t1, t1, 1
    fmv.d.x f29, t1
    addi t1, t1, 1
    fmv.d.x f30, t1
    addi t1, t1, 1
    fmv.d.x f31, t1
    addi t1, t1, 1
    li s1, 64
.Lregister_direct:
    li a7, 0
    ecall
    bnez a0, .Lregister_failure
    bnez a1, .Lregister_failure
    li t1, 1
    bne a2, t1, .Lregister_failure
    jal ra, .Lregister_check
    addi s1, s1, -1
    bnez s1, .Lregister_direct
    li a7, -1
    ecall
    li t1, -4
    bne a0, t1, .Lregister_failure
    jal ra, .Lregister_check
    li s1, 8
.Lregister_schedule:
    li a7, 6
    ecall
    bnez a0, .Lregister_failure
    li t2, 5000000
.Lregister_spin:
    addi t2, t2, -1
    bnez t2, .Lregister_spin
    jal ra, .Lregister_check
    addi s1, s1, -1
    bnez s1, .Lregister_schedule
    li a0, 0
    j .Lregister_exit
.Lregister_failure:
    li a0, -99
.Lregister_exit:
    li a7, 7
    ecall
    ebreak
.Lregister_check:
    addi t1, s0, 17
    bne gp, t1, .Lregister_failure
    addi t1, s0, 23
    bne tp, t1, .Lregister_failure
    addi t1, s0, 31
    bne t0, t1, .Lregister_failure
    addi t1, s0, 40
    bne s2, t1, .Lregister_failure
    addi t1, s0, 41
    bne s3, t1, .Lregister_failure
    addi t1, s0, 42
    bne s4, t1, .Lregister_failure
    addi t1, s0, 43
    bne s5, t1, .Lregister_failure
    addi t1, s0, 44
    bne s6, t1, .Lregister_failure
    addi t1, s0, 45
    bne s7, t1, .Lregister_failure
    addi t1, s0, 46
    bne s8, t1, .Lregister_failure
    addi t1, s0, 47
    bne s9, t1, .Lregister_failure
    addi t1, s0, 48
    bne s10, t1, .Lregister_failure
    addi t1, s0, 49
    bne s11, t1, .Lregister_failure
    addi t1, s0, 50
    bne a3, t1, .Lregister_failure
    addi t1, s0, 51
    bne a4, t1, .Lregister_failure
    addi t1, s0, 52
    bne a5, t1, .Lregister_failure
    addi t1, s0, 53
    bne a6, t1, .Lregister_failure
    frcsr t1
    li t2, 0x61
    bne t1, t2, .Lregister_failure
    addi t2, s0, 128
    fmv.x.d t1, f0
    bne t1, t2, .Lregister_failure
    addi t2, t2, 1
    fmv.x.d t1, f1
    bne t1, t2, .Lregister_failure
    addi t2, t2, 1
    fmv.x.d t1, f2
    bne t1, t2, .Lregister_failure
    addi t2, t2, 1
    fmv.x.d t1, f3
    bne t1, t2, .Lregister_failure
    addi t2, t2, 1
    fmv.x.d t1, f4
    bne t1, t2, .Lregister_failure
    addi t2, t2, 1
    fmv.x.d t1, f5
    bne t1, t2, .Lregister_failure
    addi t2, t2, 1
    fmv.x.d t1, f6
    bne t1, t2, .Lregister_failure
    addi t2, t2, 1
    fmv.x.d t1, f7
    bne t1, t2, .Lregister_failure
    addi t2, t2, 1
    fmv.x.d t1, f8
    bne t1, t2, .Lregister_failure
    addi t2, t2, 1
    fmv.x.d t1, f9
    bne t1, t2, .Lregister_failure
    addi t2, t2, 1
    fmv.x.d t1, f10
    bne t1, t2, .Lregister_failure
    addi t2, t2, 1
    fmv.x.d t1, f11
    bne t1, t2, .Lregister_failure
    addi t2, t2, 1
    fmv.x.d t1, f12
    bne t1, t2, .Lregister_failure
    addi t2, t2, 1
    fmv.x.d t1, f13
    bne t1, t2, .Lregister_failure
    addi t2, t2, 1
    fmv.x.d t1, f14
    bne t1, t2, .Lregister_failure
    addi t2, t2, 1
    fmv.x.d t1, f15
    bne t1, t2, .Lregister_failure
    addi t2, t2, 1
    fmv.x.d t1, f16
    bne t1, t2, .Lregister_failure
    addi t2, t2, 1
    fmv.x.d t1, f17
    bne t1, t2, .Lregister_failure
    addi t2, t2, 1
    fmv.x.d t1, f18
    bne t1, t2, .Lregister_failure
    addi t2, t2, 1
    fmv.x.d t1, f19
    bne t1, t2, .Lregister_failure
    addi t2, t2, 1
    fmv.x.d t1, f20
    bne t1, t2, .Lregister_failure
    addi t2, t2, 1
    fmv.x.d t1, f21
    bne t1, t2, .Lregister_failure
    addi t2, t2, 1
    fmv.x.d t1, f22
    bne t1, t2, .Lregister_failure
    addi t2, t2, 1
    fmv.x.d t1, f23
    bne t1, t2, .Lregister_failure
    addi t2, t2, 1
    fmv.x.d t1, f24
    bne t1, t2, .Lregister_failure
    addi t2, t2, 1
    fmv.x.d t1, f25
    bne t1, t2, .Lregister_failure
    addi t2, t2, 1
    fmv.x.d t1, f26
    bne t1, t2, .Lregister_failure
    addi t2, t2, 1
    fmv.x.d t1, f27
    bne t1, t2, .Lregister_failure
    addi t2, t2, 1
    fmv.x.d t1, f28
    bne t1, t2, .Lregister_failure
    addi t2, t2, 1
    fmv.x.d t1, f29
    bne t1, t2, .Lregister_failure
    addi t2, t2, 1
    fmv.x.d t1, f30
    bne t1, t2, .Lregister_failure
    addi t2, t2, 1
    fmv.x.d t1, f31
    bne t1, t2, .Lregister_failure
    addi t2, t2, 1
    ret
riscv64_native_register_test_end:
