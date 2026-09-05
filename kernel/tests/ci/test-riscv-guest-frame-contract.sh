#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Prove the RISC-V guest-frame source contract rejects representative faults.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/hyper-riscv-frame-test.XXXXXX")
trap 'rm -rf "$fixture"' EXIT HUP INT TERM

copy_sources() {
    rm -rf "$fixture/src"
    mkdir -p "$fixture/src/arch/riscv64"
    mkdir -p "$fixture/src/hal/selected" "$fixture/src/kernel/entry"
    mkdir -p "$fixture/tests/kernel" "$fixture/tests/qemu"
    cp "$root/src/arch/riscv64/guest.S" "$fixture/src/arch/riscv64/guest.S"
    cp "$root/src/arch/riscv64/trap.S" "$fixture/src/arch/riscv64/trap.S"
    cp "$root/src/arch/riscv64/registers.rs" "$fixture/src/arch/riscv64/registers.rs"
    cp "$root/src/arch/riscv64/exception.rs" "$fixture/src/arch/riscv64/exception.rs"
    cp "$root/src/arch/riscv64/context.rs" "$fixture/src/arch/riscv64/context.rs"
    cp "$root/src/arch/riscv64/guest.rs" "$fixture/src/arch/riscv64/guest.rs"
    cp "$root/src/arch/riscv64/platform.rs" "$fixture/src/arch/riscv64/platform.rs"
    cp "$root/src/arch/riscv64/vm_vcpu.rs" "$fixture/src/arch/riscv64/vm_vcpu.rs"
    cp "$root/src/hal/selected/exception.rs" "$fixture/src/hal/selected/exception.rs"
    cp "$root/src/kernel/entry/irq.rs" "$fixture/src/kernel/entry/irq.rs"
    cp "$root/tests/kernel/mod.rs" "$fixture/tests/kernel/mod.rs"
    cp "$root/tests/qemu/verify-riscv64.sh" "$fixture/tests/qemu/verify-riscv64.sh"
}

check() {
    HYPER_RISCV_GUEST_FRAME_ROOT="$fixture" \
        sh "$root/tests/ci/check-riscv-guest-frame-contract.sh"
}

mutate() {
    description=$1
    file=$2
    pattern=$3
    replacement=$4
    copy_sources
    sed "s/$pattern/$replacement/" "$fixture/$file" >"$fixture/mutated"
    mv "$fixture/mutated" "$fixture/$file"
    if check >/dev/null 2>&1; then
        echo "$description" >&2
        exit 1
    fi
}

copy_sources
check
mutate 'guest transition must mask SIE first' src/arch/riscv64/guest.S \
    'csrci sstatus, SSTATUS_SIE' 'nop'
mutate 'host state must follow an owned stack reservation' src/arch/riscv64/guest.S \
    'addi sp, sp, -GUEST_HS_ANCHOR_SIZE' 'nop'
mutate 'stores below the current sp must remain forbidden' src/arch/riscv64/guest.S \
    'sd tp, GUEST_HS_ANCHOR_TP_OFFSET(sp)' 'sd tp, -8(sp)'
mutate 'guest traps must restore the host gp' src/arch/riscv64/trap.S \
    'ld gp, GUEST_HS_ANCHOR_GP_OFFSET(t0)' 'ld gp, GUEST_HS_ANCHOR_TP_OFFSET(t0)'
mutate 'trap return must use exact guest-anchor state' src/arch/riscv64/trap.S \
    'ld t0, TRAP_FRAME_GUEST_ANCHOR_RETURN_OFFSET(sp)' 'ld t0, TRAP_FRAME_GUEST_ORIGIN_OFFSET(sp)'
mutate 'trap return must not clobber an already-restored register' src/arch/riscv64/trap.S \
    'ld t0, TRAP_FRAME_GUEST_ANCHOR_RETURN_OFFSET(sp)' 'ld t1, TRAP_FRAME_GUEST_ANCHOR_RETURN_OFFSET(sp)'
mutate 'both trap paths must initialize the CPU field' src/arch/riscv64/trap.S \
    'sd tp, TRAP_FRAME_HOST_CPU_INDEX_OFFSET(sp)' 'sd zero, TRAP_FRAME_HOST_CPU_INDEX_OFFSET(sp)'
mutate 'both trap paths must initialize architectural x0' src/arch/riscv64/trap.S \
    'sd zero, TRAP_FRAME_GENERAL_OFFSET(sp)' 'nop'
mutate 'TrapFrame must compiler-check the guest-anchor return field' src/arch/riscv64/exception.rs \
    'offset_of!(TrapFrame, guest_anchor_return)' 'offset_of!(TrapFrame, guest_origin)'
mutate 'TrapFrame size must remain compiler checked' src/arch/riscv64/exception.rs \
    'assert!(size_of::<TrapFrame>() == super::registers::TRAP_FRAME_SIZE as usize);' \
    'assert!(size_of::<TrapFrame>() != super::registers::TRAP_FRAME_SIZE as usize);'
mutate 'typed unwind must clear guest-origin publication first' src/arch/riscv64/trap.S \
    'csrw sscratch, zero' 'nop'
mutate 'invalid trap actions must not recursively trap' src/arch/riscv64/trap.S \
    'call riscv64_invalid_trap_action' 'ebreak'
mutate 'guest floating state must be saved before host restoration' src/arch/riscv64/trap.S \
    'call riscv64_save_guest_floating_point' 'nop'
mutate 'guest return must keep HS floating state enabled' src/arch/riscv64/trap.S \
    'li t0, (3 << 13)' 'li t0, 0'
mutate 'guest traps must capture manually swapped SCOUNTEREN' src/arch/riscv64/trap.S \
    'sd t1, VCPU_SCOUNTEREN_OFFSET(a0)' 'nop'
mutate 'guest traps must restore host SENVCFG' src/arch/riscv64/trap.S \
    'ld t1, GUEST_HS_ANCHOR_SENVCFG_OFFSET(t0)' 'ld t1, GUEST_HS_ANCHOR_SCOUNTEREN_OFFSET(t0)'
mutate 'guest traps must quiesce VSATP before host policy' src/arch/riscv64/trap.S \
    'csrw vsatp, zero' 'nop'
mutate 'guest VS-stage translations must be fenced on install' src/arch/riscv64/guest.S \
    'hfence.vvma zero, zero' 'nop'
mutate 'guest pending interrupts must be captured before quiescing' src/arch/riscv64/trap.S \
    'sd t1, VCPU_HVIP_OFFSET(a0)' 'nop'
mutate 'legacy clear-IPI must update saved VSSIP' src/arch/riscv64/guest.rs \
    'context.hvip &= !HVIP_VSSIP' 'context.hvip |= HVIP_VSSIP'
mutate 'guest timer compare must be captured for migration' src/arch/riscv64/trap.S \
    'sd t1, VCPU_VSTIMECMP_OFFSET(a0)' 'nop'
mutate 'guest scounteren must not overwrite hcounteren policy' src/arch/riscv64/guest.S \
    'csrw scounteren, t0' 'csrw hcounteren, t0'
mutate 'RISC-V guest timer state requires Sstc' src/arch/riscv64/platform.rs \
    'return Err(Error::MissingSstc);' 'return Ok(candidate);'
mutate 'every hart must validate firmware STCE enablement' src/arch/riscv64/vm_vcpu.rs \
    'if !enable_supervisor_timer_compare()' 'if false'
mutate 'guest anchors must retain their exact context' src/arch/riscv64/guest.S \
    'sd t6, GUEST_HS_ANCHOR_CONTEXT_OFFSET(sp)' 'nop'
mutate 'guest anchor returns must be consumed exactly once' src/arch/riscv64/context.rs \
    'fn consume_irq_tail' 'fn discard_irq_tail'
mutate 'HTIMEDELTA must use additive offset direction' src/arch/riscv64/context.rs \
    'value.wrapping_sub(physical)' 'physical.wrapping_sub(value)'
mutate 'RISC-V must retain a qualified IRQ-tail capability' src/hal/selected/exception.rs \
    'any(CONFIG_ARCH_AARCH64, CONFIG_ARCH_RISCV64)' 'CONFIG_ARCH_AARCH64'
mutate 'SSIP must use formal IRQ accounting' src/kernel/entry/irq.rs \
    'dispatch_kernel_rpc_entry(origin: InterruptOrigin)' 'dispatch_kernel_rpc_entry()'
mutate 'runtime acceptance must require RISC-V guest preemption' tests/qemu/verify-riscv64.sh \
    'RISC-V IRQ-tail Fair vCPU preemption passed' 'RISC-V IRQ-tail probe unavailable'
