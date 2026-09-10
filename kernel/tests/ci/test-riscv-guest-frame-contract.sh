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
    mkdir -p "$fixture/src/hal/selected" "$fixture/src/kernel/entry" "$fixture/src/kernel/boot"
    mkdir -p "$fixture/tests/kernel" "$fixture/tests/qemu"
    cp "$root/src/arch/riscv64/guest.S" "$fixture/src/arch/riscv64/guest.S"
    cp "$root/src/arch/riscv64/trap.S" "$fixture/src/arch/riscv64/trap.S"
    cp "$root/src/arch/riscv64/registers.rs" "$fixture/src/arch/riscv64/registers.rs"
    cp "$root/src/arch/riscv64/exception.rs" "$fixture/src/arch/riscv64/exception.rs"
    cp "$root/src/arch/riscv64/context.rs" "$fixture/src/arch/riscv64/context.rs"
    cp "$root/src/arch/riscv64/guest.rs" "$fixture/src/arch/riscv64/guest.rs"
    cp "$root/src/main.rs" "$fixture/src/main.rs"
    cp "$root/src/kernel/boot/mod.rs" "$fixture/src/kernel/boot/mod.rs"
    cp "$root/src/arch/riscv64/mod.rs" "$fixture/src/arch/riscv64/mod.rs"
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
mutate 'runtime acceptance must require completed kernel self-tests' tests/qemu/verify-riscv64.sh \
    'kernel self-tests completed' 'kernel self-tests unavailable'
mutate 'VS interrupt enable must be captured before Rust' src/arch/riscv64/trap.S \
    'sd t1, VCPU_VSIE_OFFSET(a0)' 'nop'
mutate 'trap resume must restore the authoritative VSSTATUS snapshot' src/arch/riscv64/trap.S \
    'ld t1, VCPU_VSSTATUS_OFFSET(t0)' 'li t1, 0'
mutate 'trap entry must disable guest interrupt delivery' src/arch/riscv64/trap.S \
    'csrw vsie, zero' 'nop'
mutate 'guest VU privilege must survive detached IRQ tails' src/arch/riscv64/trap.S \
    'sd t1, VCPU_SUPERVISOR_OFFSET(a0)' 'nop'
mutate 'guest entry must not force VU continuations into VS' src/arch/riscv64/guest.S \
    'ld t0, VCPU_SUPERVISOR_OFFSET(t6)' 'li t0, 1'
mutate 'stopped-run proof cannot cross CPUs' src/arch/riscv64/context.rs \
    'PhantomData<alloc::rc::Rc<()>>' 'PhantomData<()>'
mutate 'stopped-run proof must validate before hardware detach' src/arch/riscv64/vm_vcpu.rs \
    'stopped.validate_for(context)' 'Ok::<(), super::context::GuestRunError>(())'
mutate 'stopped-run proof must be consumed after detach' src/arch/riscv64/vm_vcpu.rs \
    'stopped.consume_for(context)' 'Ok::<(), super::context::GuestRunError>(())'
mutate 'local stage-2 root must detach before VM reclamation' src/arch/riscv64/vm_vcpu.rs \
    '"csrw hgatp, zero"' '"nop"'
mutate 'PLIC reconciliation must preserve pending software interrupts' src/arch/riscv64/vm_vcpu.rs \
    'context.hvip = (context.hvip \& !(1 << 10)) | (u64::from(pending) << 10)' \
    'context.hvip = u64::from(pending) << 10'

mutate 'timer admission must inspect accepted STCE readback' src/arch/riscv64/vm_vcpu.rs \
    'accepted \& (1 << 63) != 0' 'true'
mutate 'timer admission must restore original host environment' src/arch/riscv64/vm_vcpu.rs \
    '"csrw henvcfg, {original}"' '"nop"'
mutate 'primary admission must require the guest timer gate' src/arch/riscv64/mod.rs \
    'vm_vcpu::discover_local_timer()' 'true'
mutate 'secondary admission must apply the same hardware contract' src/arch/riscv64/mod.rs \
    '    prepare_primary_cpu_admission()' '    true'
mutate 'boot must fail closed on incompatible primary admission' src/kernel/boot/mod.rs \
    'if !crate::hal::cpu::prepare_primary_admission()' 'if false'
mutate 'secondary startup must fail closed on incompatible admission' src/main.rs \
    'if !crate::hal::cpu::secondary_is_compatible()' 'if false'
mutate 'activation must inspect committed STCE readback' src/arch/riscv64/vm_vcpu.rs \
    'environment \& (1 << 63) != 0' 'true'

# The unmodified fixture above is the positive masked-entry case. These
# mutations reproduce the unmask-before-anchor bug and remove each fail-closed
# boundary independently, so masking only inside assembly is insufficient.
mutate 'guest preparation must not unmask before the returning anchor exists' src/arch/riscv64/mod.rs \
    'pub const fn prepare_interrupts_for_guest_entry() {}' \
    'pub fn prepare_interrupts_for_guest_entry() { enable_local_irq(); }'
mutate 'run entry must read the actual IRQ mask' src/arch/riscv64/context.rs \
    '"csrr {status}, sstatus"' '"li {status}, 0"'
mutate 'run entry must read the actual anchor publication' src/arch/riscv64/context.rs \
    '"csrr {scratch}, sscratch"' '"li {scratch}, 0"'
mutate 'run entry must reject enabled local interrupts' src/arch/riscv64/context.rs \
    'if status \& registers::SSTATUS_SIE != 0 || scratch != 0' 'if scratch != 0'
mutate 'run entry must reject an existing anchor' src/arch/riscv64/context.rs \
    'if status \& registers::SSTATUS_SIE != 0 || scratch != 0' 'if status \& registers::SSTATUS_SIE != 0'

copy_sources
# Reproduce the actual migration regression with both CSR writes retained,
# merely installing delegation too late for the saved VSIE write to take effect.
awk '
    /csrw hideleg, t0/ { next }
    { print }
    /csrw vsie, t0/ {
        print "    li t0, HIDELEG_GUEST_VALUE"
        print "    csrw hideleg, t0"
    }
' "$fixture/src/arch/riscv64/guest.S" >"$fixture/mutated"
mv "$fixture/mutated" "$fixture/src/arch/riscv64/guest.S"
if check >/dev/null 2>&1; then
    echo 'delegation after VSIE restore must be rejected even when both writes exist' >&2
    exit 1
fi

mutate 'guest activation must inspect the actual stage-2 selection' src/arch/riscv64/vm_vcpu.rs \
    '"csrr {value}, hgatp"' '"li {value}, 0"'
mutate 'guest activation must reject Bare translation despite cached residency' src/arch/riscv64/vm_vcpu.rs \
    'if hgatp >> 60 != 8 || root == 0 || root \& 0x3fff != 0' 'if root == 0 || root \& 0x3fff != 0'
mutate 'guest activation must reject a null stage-2 root' src/arch/riscv64/vm_vcpu.rs \
    'if hgatp >> 60 != 8 || root == 0 || root \& 0x3fff != 0' 'if hgatp >> 60 != 8 || root \& 0x3fff != 0'

mutate 'VU virtual instructions must not acquire supervisor CSR emulation' src/arch/riscv64/guest.rs \
    'return illegal_instruction(frame)' 'return capture_virtual_instruction(frame)'
mutate 'forwarded guest illegal trap must retain the faulting PC' src/arch/riscv64/guest.rs \
    'context.vsepc = exit.program_counter;' 'context.vsepc = exit.program_counter.wrapping_add(4);'
mutate 'synchronous guest trap must ignore VSTVEC vector-mode bits' src/arch/riscv64/guest.rs \
    'program_counter: context.vstvec \& !3' 'program_counter: context.vstvec'
