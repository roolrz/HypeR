/*
 * SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 */

#include "asm_constants.h"

.section .text.guest, "ax"
.balign 4
.global riscv64_enter_guest
.type riscv64_enter_guest, @function
riscv64_enter_guest:
    mv t6, a0
    sd tp, -8(sp)

    ld t0, VCPU_VSSTATUS_OFFSET(t6)
    csrw vsstatus, t0
    ld t0, VCPU_VSIE_OFFSET(t6)
    csrw vsie, t0
    ld t0, VCPU_VSTVEC_OFFSET(t6)
    csrw vstvec, t0
    ld t0, VCPU_VSSCRATCH_OFFSET(t6)
    csrw vsscratch, t0
    ld t0, VCPU_VSEPC_OFFSET(t6)
    csrw vsepc, t0
    ld t0, VCPU_VSCAUSE_OFFSET(t6)
    csrw vscause, t0
    ld t0, VCPU_VSTVAL_OFFSET(t6)
    csrw vstval, t0
    ld t0, VCPU_VSATP_OFFSET(t6)
    csrw vsatp, t0

    # Delegate exceptions handled by a normal supervisor kernel. Guest
    # supervisor ecalls and guest-page faults remain in HS mode for SBI and
    # stage-2 emulation.
    li t0, HEDELEG_GUEST_VALUE
    csrw hedeleg, t0
    li t0, HIDELEG_GUEST_VALUE
    csrw hideleg, t0
    li t0, HCOUNTEREN_GUEST_VALUE
    csrw hcounteren, t0

    # SPV=1 selects virtualized execution and SPVP=1 enters VS mode.
    csrr t0, hstatus
    # Trap guest WFI so HS retains control of physical idle state. Keep VTSR
    # and VTVM clear, while setting VTW.
    li t1, HSTATUS_GUEST_CLEAR
    not t1, t1
    and t0, t0, t1
    li t1, HSTATUS_GUEST_SET
    or t0, t0, t1
    csrw hstatus, t0
    # SPP selects supervisor privilege for the HS sret; SPV above selects the
    # virtualized execution domain.
    li t0, SSTATUS_GUEST_SPP
    csrs sstatus, t0
    ld t0, VCPU_PC_OFFSET(t6)
    csrw sepc, t0

    # Preserve the HS stack for trap entry before loading guest registers.
    csrw sscratch, sp
    ld ra, 8(t6)
    ld sp, 16(t6)
    ld gp, 24(t6)
    ld tp, 32(t6)
    ld t0, 40(t6)
    ld t1, 48(t6)
    ld t2, 56(t6)
    ld s0, 64(t6)
    ld s1, 72(t6)
    ld a0, 80(t6)
    ld a1, 88(t6)
    ld a2, 96(t6)
    ld a3, 104(t6)
    ld a4, 112(t6)
    ld a5, 120(t6)
    ld a6, 128(t6)
    ld a7, 136(t6)
    ld s2, 144(t6)
    ld s3, 152(t6)
    ld s4, 160(t6)
    ld s5, 168(t6)
    ld s6, 176(t6)
    ld s7, 184(t6)
    ld s8, 192(t6)
    ld s9, 200(t6)
    ld s10, 208(t6)
    ld s11, 216(t6)
    ld t3, 224(t6)
    ld t4, 232(t6)
    ld t5, 240(t6)
    ld t6, 248(t6)
    sret
.size riscv64_enter_guest, . - riscv64_enter_guest

.global riscv64_activate_stage2
.type riscv64_activate_stage2, @function
riscv64_activate_stage2:
    fence rw, rw
    csrw hgatp, a0
    hfence.gvma zero, zero
    ret
.size riscv64_activate_stage2, . - riscv64_activate_stage2

.global riscv64_invalidate_stage2_page
.type riscv64_invalidate_stage2_page, @function
riscv64_invalidate_stage2_page:
    fence rw, rw
    hfence.gvma a0, a1
    ret
.size riscv64_invalidate_stage2_page, . - riscv64_invalidate_stage2_page

.global riscv64_load_guest_floating_point
.type riscv64_load_guest_floating_point, @function
riscv64_load_guest_floating_point:
    li t0, (3 << 13)
    csrs sstatus, t0
    lw t0, VCPU_FCSR_OFFSET(a0)
    fscsr t0
    fld f0, VCPU_FLOATING_OFFSET + 0(a0)
    fld f1, VCPU_FLOATING_OFFSET + 8(a0)
    fld f2, VCPU_FLOATING_OFFSET + 16(a0)
    fld f3, VCPU_FLOATING_OFFSET + 24(a0)
    fld f4, VCPU_FLOATING_OFFSET + 32(a0)
    fld f5, VCPU_FLOATING_OFFSET + 40(a0)
    fld f6, VCPU_FLOATING_OFFSET + 48(a0)
    fld f7, VCPU_FLOATING_OFFSET + 56(a0)
    fld f8, VCPU_FLOATING_OFFSET + 64(a0)
    fld f9, VCPU_FLOATING_OFFSET + 72(a0)
    fld f10, VCPU_FLOATING_OFFSET + 80(a0)
    fld f11, VCPU_FLOATING_OFFSET + 88(a0)
    fld f12, VCPU_FLOATING_OFFSET + 96(a0)
    fld f13, VCPU_FLOATING_OFFSET + 104(a0)
    fld f14, VCPU_FLOATING_OFFSET + 112(a0)
    fld f15, VCPU_FLOATING_OFFSET + 120(a0)
    fld f16, VCPU_FLOATING_OFFSET + 128(a0)
    fld f17, VCPU_FLOATING_OFFSET + 136(a0)
    fld f18, VCPU_FLOATING_OFFSET + 144(a0)
    fld f19, VCPU_FLOATING_OFFSET + 152(a0)
    fld f20, VCPU_FLOATING_OFFSET + 160(a0)
    fld f21, VCPU_FLOATING_OFFSET + 168(a0)
    fld f22, VCPU_FLOATING_OFFSET + 176(a0)
    fld f23, VCPU_FLOATING_OFFSET + 184(a0)
    fld f24, VCPU_FLOATING_OFFSET + 192(a0)
    fld f25, VCPU_FLOATING_OFFSET + 200(a0)
    fld f26, VCPU_FLOATING_OFFSET + 208(a0)
    fld f27, VCPU_FLOATING_OFFSET + 216(a0)
    fld f28, VCPU_FLOATING_OFFSET + 224(a0)
    fld f29, VCPU_FLOATING_OFFSET + 232(a0)
    fld f30, VCPU_FLOATING_OFFSET + 240(a0)
    fld f31, VCPU_FLOATING_OFFSET + 248(a0)
    ret
.size riscv64_load_guest_floating_point, . - riscv64_load_guest_floating_point

.global riscv64_save_guest_floating_point
.type riscv64_save_guest_floating_point, @function
riscv64_save_guest_floating_point:
    fsd f0, VCPU_FLOATING_OFFSET + 0(a0)
    fsd f1, VCPU_FLOATING_OFFSET + 8(a0)
    fsd f2, VCPU_FLOATING_OFFSET + 16(a0)
    fsd f3, VCPU_FLOATING_OFFSET + 24(a0)
    fsd f4, VCPU_FLOATING_OFFSET + 32(a0)
    fsd f5, VCPU_FLOATING_OFFSET + 40(a0)
    fsd f6, VCPU_FLOATING_OFFSET + 48(a0)
    fsd f7, VCPU_FLOATING_OFFSET + 56(a0)
    fsd f8, VCPU_FLOATING_OFFSET + 64(a0)
    fsd f9, VCPU_FLOATING_OFFSET + 72(a0)
    fsd f10, VCPU_FLOATING_OFFSET + 80(a0)
    fsd f11, VCPU_FLOATING_OFFSET + 88(a0)
    fsd f12, VCPU_FLOATING_OFFSET + 96(a0)
    fsd f13, VCPU_FLOATING_OFFSET + 104(a0)
    fsd f14, VCPU_FLOATING_OFFSET + 112(a0)
    fsd f15, VCPU_FLOATING_OFFSET + 120(a0)
    fsd f16, VCPU_FLOATING_OFFSET + 128(a0)
    fsd f17, VCPU_FLOATING_OFFSET + 136(a0)
    fsd f18, VCPU_FLOATING_OFFSET + 144(a0)
    fsd f19, VCPU_FLOATING_OFFSET + 152(a0)
    fsd f20, VCPU_FLOATING_OFFSET + 160(a0)
    fsd f21, VCPU_FLOATING_OFFSET + 168(a0)
    fsd f22, VCPU_FLOATING_OFFSET + 176(a0)
    fsd f23, VCPU_FLOATING_OFFSET + 184(a0)
    fsd f24, VCPU_FLOATING_OFFSET + 192(a0)
    fsd f25, VCPU_FLOATING_OFFSET + 200(a0)
    fsd f26, VCPU_FLOATING_OFFSET + 208(a0)
    fsd f27, VCPU_FLOATING_OFFSET + 216(a0)
    fsd f28, VCPU_FLOATING_OFFSET + 224(a0)
    fsd f29, VCPU_FLOATING_OFFSET + 232(a0)
    fsd f30, VCPU_FLOATING_OFFSET + 240(a0)
    fsd f31, VCPU_FLOATING_OFFSET + 248(a0)
    frcsr t0
    sw t0, VCPU_FCSR_OFFSET(a0)
    ret
.size riscv64_save_guest_floating_point, . - riscv64_save_guest_floating_point
