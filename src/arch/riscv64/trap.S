/*
 * SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 */

.equ TRAP_FRAME_SIZE, 320

.section .text.trap, "ax"
.balign 4
.global riscv64_trap_vector
.type riscv64_trap_vector, @function
riscv64_trap_vector:
    # Guest entry leaves the reserved HS stack in sscratch. Host execution
    # keeps sscratch zero, so the same non-clobbering exchange distinguishes
    # both origins before any interrupted GPR is touched.
    csrrw sp, sscratch, sp
    bnez sp, .Lguest_entry
    csrrw sp, sscratch, sp
    addi sp, sp, -TRAP_FRAME_SIZE
    sd zero, 0(sp)
    sd ra, 8(sp)
    sd t0, 40(sp)
    addi t0, sp, TRAP_FRAME_SIZE
    sd t0, 16(sp)
    j .Lsave_remaining
.Lguest_entry:
    addi sp, sp, -TRAP_FRAME_SIZE
    sd zero, 0(sp)
    sd ra, 8(sp)
    sd t0, 40(sp)
    csrr t0, sscratch
    sd t0, 16(sp)
.Lsave_remaining:
    sd gp, 24(sp)
    sd tp, 32(sp)
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
    csrr t0, sepc
    sd t0, 256(sp)
    csrr t0, sstatus
    sd t0, 264(sp)
    csrr t0, scause
    sd t0, 272(sp)
    csrr t0, stval
    sd t0, 280(sp)
    csrr t0, htval
    sd t0, 288(sp)
    csrr t0, htinst
    sd t0, 296(sp)
    csrr t0, hstatus
    andi t0, t0, 0x80
    sd t0, 304(sp)
    beqz t0, .Ldispatch
    # guest.S saved the HS logical CPU index just below the reserved stack.
    ld tp, 312(sp)
.Ldispatch:
    mv a0, sp
    call riscv64_trap_dispatch
    ld t0, 256(sp)
    csrw sepc, t0
    ld t0, 264(sp)
    csrw sstatus, t0
    ld ra, 8(sp)
    ld gp, 24(sp)
    ld tp, 32(sp)
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
    ld t0, 304(sp)
    bnez t0, .Lguest_return
    ld t0, 40(sp)
    ld sp, 16(sp)
    sret
.Lguest_return:
    ld t0, 40(sp)
    addi sp, sp, TRAP_FRAME_SIZE
    csrrw sp, sscratch, sp
    sret
.size riscv64_trap_vector, . - riscv64_trap_vector

.global riscv64_call_trap_on_stack
.type riscv64_call_trap_on_stack, @function
riscv64_call_trap_on_stack:
    # a0 retains the trap-frame pointer, a1 is the aligned IRQ-stack top, and
    # a2 is an extern "C" callback that returns after dispatching the IRQ.
    addi a1, a1, -16
    sd sp, 0(a1)
    sd ra, 8(a1)
    mv sp, a1
    jalr a2
    ld t0, 0(sp)
    ld ra, 8(sp)
    mv sp, t0
    ret
.size riscv64_call_trap_on_stack, . - riscv64_call_trap_on_stack
