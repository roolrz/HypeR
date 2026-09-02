/*
 * SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 */

#include "asm_constants.h"

.macro SAVE_FLOATING base, floating_offset, fcsr_offset
    fsd f0, \floating_offset + 0(\base)
    fsd f1, \floating_offset + 8(\base)
    fsd f2, \floating_offset + 16(\base)
    fsd f3, \floating_offset + 24(\base)
    fsd f4, \floating_offset + 32(\base)
    fsd f5, \floating_offset + 40(\base)
    fsd f6, \floating_offset + 48(\base)
    fsd f7, \floating_offset + 56(\base)
    fsd f8, \floating_offset + 64(\base)
    fsd f9, \floating_offset + 72(\base)
    fsd f10, \floating_offset + 80(\base)
    fsd f11, \floating_offset + 88(\base)
    fsd f12, \floating_offset + 96(\base)
    fsd f13, \floating_offset + 104(\base)
    fsd f14, \floating_offset + 112(\base)
    fsd f15, \floating_offset + 120(\base)
    fsd f16, \floating_offset + 128(\base)
    fsd f17, \floating_offset + 136(\base)
    fsd f18, \floating_offset + 144(\base)
    fsd f19, \floating_offset + 152(\base)
    fsd f20, \floating_offset + 160(\base)
    fsd f21, \floating_offset + 168(\base)
    fsd f22, \floating_offset + 176(\base)
    fsd f23, \floating_offset + 184(\base)
    fsd f24, \floating_offset + 192(\base)
    fsd f25, \floating_offset + 200(\base)
    fsd f26, \floating_offset + 208(\base)
    fsd f27, \floating_offset + 216(\base)
    fsd f28, \floating_offset + 224(\base)
    fsd f29, \floating_offset + 232(\base)
    fsd f30, \floating_offset + 240(\base)
    fsd f31, \floating_offset + 248(\base)
    frcsr t0
    sw t0, \fcsr_offset(\base)
.endm

.macro LOAD_FLOATING base, floating_offset, fcsr_offset
    lw t0, \fcsr_offset(\base)
    fscsr t0
    fld f0, \floating_offset + 0(\base)
    fld f1, \floating_offset + 8(\base)
    fld f2, \floating_offset + 16(\base)
    fld f3, \floating_offset + 24(\base)
    fld f4, \floating_offset + 32(\base)
    fld f5, \floating_offset + 40(\base)
    fld f6, \floating_offset + 48(\base)
    fld f7, \floating_offset + 56(\base)
    fld f8, \floating_offset + 64(\base)
    fld f9, \floating_offset + 72(\base)
    fld f10, \floating_offset + 80(\base)
    fld f11, \floating_offset + 88(\base)
    fld f12, \floating_offset + 96(\base)
    fld f13, \floating_offset + 104(\base)
    fld f14, \floating_offset + 112(\base)
    fld f15, \floating_offset + 120(\base)
    fld f16, \floating_offset + 128(\base)
    fld f17, \floating_offset + 136(\base)
    fld f18, \floating_offset + 144(\base)
    fld f19, \floating_offset + 152(\base)
    fld f20, \floating_offset + 160(\base)
    fld f21, \floating_offset + 168(\base)
    fld f22, \floating_offset + 176(\base)
    fld f23, \floating_offset + 184(\base)
    fld f24, \floating_offset + 192(\base)
    fld f25, \floating_offset + 200(\base)
    fld f26, \floating_offset + 208(\base)
    fld f27, \floating_offset + 216(\base)
    fld f28, \floating_offset + 224(\base)
    fld f29, \floating_offset + 232(\base)
    fld f30, \floating_offset + 240(\base)
    fld f31, \floating_offset + 248(\base)
.endm

.section .text.guest, "ax"
.balign 4
.global riscv64_enter_guest
.type riscv64_enter_guest, @function
riscv64_enter_guest:
    csrci sstatus, SSTATUS_SIE
    mv t6, a0
    addi sp, sp, -GUEST_HS_ANCHOR_SIZE
    sd ra, GUEST_HS_ANCHOR_RA_OFFSET(sp)
    sd s0, GUEST_HS_ANCHOR_S0_OFFSET + 0(sp)
    sd s1, GUEST_HS_ANCHOR_S0_OFFSET + 8(sp)
    sd s2, GUEST_HS_ANCHOR_S0_OFFSET + 16(sp)
    sd s3, GUEST_HS_ANCHOR_S0_OFFSET + 24(sp)
    sd s4, GUEST_HS_ANCHOR_S0_OFFSET + 32(sp)
    sd s5, GUEST_HS_ANCHOR_S0_OFFSET + 40(sp)
    sd s6, GUEST_HS_ANCHOR_S0_OFFSET + 48(sp)
    sd s7, GUEST_HS_ANCHOR_S0_OFFSET + 56(sp)
    sd s8, GUEST_HS_ANCHOR_S0_OFFSET + 64(sp)
    sd s9, GUEST_HS_ANCHOR_S0_OFFSET + 72(sp)
    sd s10, GUEST_HS_ANCHOR_S0_OFFSET + 80(sp)
    sd s11, GUEST_HS_ANCHOR_S0_OFFSET + 88(sp)
    sd gp, GUEST_HS_ANCHOR_GP_OFFSET(sp)
    sd tp, GUEST_HS_ANCHOR_TP_OFFSET(sp)
    sd t6, GUEST_HS_ANCHOR_CONTEXT_OFFSET(sp)
    csrr t0, sstatus
    li t1, (3 << 13)
    and t0, t0, t1
    sd t0, GUEST_HS_ANCHOR_FS_OFFSET(sp)
    csrs sstatus, t1
    SAVE_FLOATING sp, GUEST_HS_ANCHOR_FLOATING_OFFSET, GUEST_HS_ANCHOR_FCSR_OFFSET
    csrr t0, scounteren
    sd t0, GUEST_HS_ANCHOR_SCOUNTEREN_OFFSET(sp)
    csrr t0, senvcfg
    sd t0, GUEST_HS_ANCHOR_SENVCFG_OFFSET(sp)

    # Guest floating state is loaded only after the complete host ABI image is
    # recoverable from the anchor. Trap entry performs the inverse transition
    # before calling Rust policy.
    ld t6, GUEST_HS_ANCHOR_CONTEXT_OFFSET(sp)
    LOAD_FLOATING t6, VCPU_FLOATING_OFFSET, VCPU_FCSR_OFFSET

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
    li t0, HVIP_GUEST_MASK
    csrc hvip, t0
    ld t1, VCPU_HVIP_OFFSET(t6)
    and t1, t1, t0
    csrs hvip, t1
    ld t0, VCPU_VSATP_OFFSET(t6)
    csrw vsatp, t0
    hfence.vvma zero, zero
    ld t0, VCPU_SCOUNTEREN_OFFSET(t6)
    csrw scounteren, t0
    ld t0, VCPU_SENVCFG_OFFSET(t6)
    csrw senvcfg, t0
    li t0, (1 << 63)
    csrs henvcfg, t0
    ld t0, VCPU_VSTIMECMP_OFFSET(t6)
    csrw 0x24d, t0

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

    # Preserve the reserved HS anchor for trap entry before loading guest
    # registers. SPIE carries the desired HS interrupt-enabled state across the
    # initial sret into VS mode.
    csrw sscratch, sp
    li t0, SSTATUS_SPIE
    csrs sstatus, t0
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

.global riscv64_restore_host_floating_point
.type riscv64_restore_host_floating_point, @function
riscv64_restore_host_floating_point:
    li t0, (3 << 13)
    csrs sstatus, t0
    LOAD_FLOATING a0, GUEST_HS_ANCHOR_FLOATING_OFFSET, GUEST_HS_ANCHOR_FCSR_OFFSET
    ret
.size riscv64_restore_host_floating_point, . - riscv64_restore_host_floating_point

.global riscv64_activate_stage2
.type riscv64_activate_stage2, @function
riscv64_activate_stage2:
    # The old VS root must not remain live while HGATP changes: speculative
    # VS-stage walks could otherwise populate translations under the new VMID.
    csrw vsatp, zero
    hfence.vvma zero, zero
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
