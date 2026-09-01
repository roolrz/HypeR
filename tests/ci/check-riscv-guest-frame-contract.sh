#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Protect the RISC-V HS guest-run anchor and trap-frame ABI.
set -eu

root=${HYPER_RISCV_GUEST_FRAME_ROOT:-$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)}
cd "$root"

guest=src/arch/riscv64/guest.S
trap=src/arch/riscv64/trap.S
registers=src/arch/riscv64/registers.rs
exception=src/arch/riscv64/exception.rs
context=src/arch/riscv64/context.rs
guest_rust=src/arch/riscv64/guest.rs
platform=src/arch/riscv64/platform.rs
vm_vcpu=src/arch/riscv64/vm_vcpu.rs
selected_exception=src/hal/selected/exception.rs
kernel_irq=src/kernel/entry/irq.rs
kernel_tests=tests/kernel/mod.rs
qemu_verify=tests/qemu/verify-riscv64.sh

entry=$(mktemp "${TMPDIR:-/tmp}/hyper-riscv-entry.XXXXXX")
trap_body=$(mktemp "${TMPDIR:-/tmp}/hyper-riscv-trap.XXXXXX")
anchor_exit=$(mktemp "${TMPDIR:-/tmp}/hyper-riscv-anchor-exit.XXXXXX")
guest_return=$(mktemp "${TMPDIR:-/tmp}/hyper-riscv-guest-return.XXXXXX")
invalid_action=$(mktemp "${TMPDIR:-/tmp}/hyper-riscv-invalid-action.XXXXXX")
trap 'rm -f "$entry" "$trap_body" "$anchor_exit" "$guest_return" "$invalid_action"' EXIT HUP INT TERM
sed -n '/^riscv64_enter_guest:/,/^\.size riscv64_enter_guest/p' "$guest" >"$entry"
sed -n '/^riscv64_trap_vector:/,/^\.size riscv64_trap_vector/p' "$trap" >"$trap_body"
sed -n '/^\.Lanchor_irq_tail:/,/^[.]Lresume_trap:/p' "$trap" >"$anchor_exit"
sed -n '/^\.Lguest_return:/,/^\.size riscv64_trap_vector/p' "$trap" >"$guest_return"
sed -n '/^\.Linvalid_trap_action:/,/^[.]Lrun_postlude:/p' "$trap" >"$invalid_action"

line_first() {
    rg -n "$2" "$1" | sed -n '1s/:.*//p'
}

require_order() {
    first=$(line_first "$1" "$2")
    second=$(line_first "$1" "$3")
    if [ -z "$first" ] || [ -z "$second" ] || [ "$first" -ge "$second" ]; then
        echo "$4" >&2
        exit 1
    fi
}

require_order "$entry" 'csrci[[:space:]]+sstatus,[[:space:]]+SSTATUS_SIE' \
    'addi[[:space:]]+sp,[[:space:]]+sp,[[:space:]]+-GUEST_HS_ANCHOR_SIZE' \
    'guest entry must mask SIE before mutating its run state'
require_order "$entry" 'addi[[:space:]]+sp,[[:space:]]+sp,[[:space:]]+-GUEST_HS_ANCHOR_SIZE' \
    'sd[[:space:]]+ra,[[:space:]]+GUEST_HS_ANCHOR_RA_OFFSET\(sp\)' \
    'guest entry must reserve its anchor before saving the host return ABI'
require_order "$entry" 'sd[[:space:]]+t6,[[:space:]]+GUEST_HS_ANCHOR_CONTEXT_OFFSET\(sp\)' \
    'csrw[[:space:]]+sscratch,[[:space:]]+sp' \
    'sscratch must publish only the complete aligned anchor'
if rg -q 's[dw][[:space:]]+[^,]+,[[:space:]]*-[0-9]+\(sp\)' "$entry"; then
    echo 'guest entry must not store below its current stack pointer' >&2
    exit 1
fi
rg -q 'li[[:space:]]+t0,[[:space:]]+SSTATUS_SPIE' "$entry" || {
    echo 'initial sret must restore the intended HS interrupt state from SPIE' >&2
    exit 1
}

require_order "$trap_body" 'sd[[:space:]]+gp,[[:space:]]+TRAP_FRAME_GP_OFFSET\(sp\)' \
    'ld[[:space:]]+gp,[[:space:]]+GUEST_HS_ANCHOR_GP_OFFSET\(t0\)' \
    'trap entry must save guest gp before restoring host gp'
require_order "$trap_body" 'sd[[:space:]]+tp,[[:space:]]+TRAP_FRAME_TP_OFFSET\(sp\)' \
    'ld[[:space:]]+tp,[[:space:]]+GUEST_HS_ANCHOR_TP_OFFSET\(t0\)' \
    'trap entry must save guest tp before restoring host tp'
require_order "$trap_body" 'call[[:space:]]+riscv64_save_guest_floating_point' \
    'call[[:space:]]+riscv64_restore_host_floating_point' \
    'trap entry must save guest floating state before restoring host state'
require_order "$trap_body" 'beqz[[:space:]]+t0,[[:space:]]+\.Lhost_origin' \
    'and[[:space:]]+t1,[[:space:]]+t0,[[:space:]]+t1' \
    'guest origin must require both an anchor entry and hardware SPV'
require_order "$trap_body" 'sd[[:space:]]+t1,[[:space:]]+TRAP_FRAME_GUEST_ANCHOR_RETURN_OFFSET\(sp\)' \
    'ld[[:space:]]+t0,[[:space:]]+TRAP_FRAME_GUEST_ANCHOR_RETURN_OFFSET\(sp\)' \
    'guest-anchor return state must be materialized in the trap frame'
require_order "$trap_body" 'ld[[:space:]]+t0,[[:space:]]+TRAP_FRAME_GUEST_ANCHOR_RETURN_OFFSET\(sp\)' \
    'bnez[[:space:]]+t0,[[:space:]]+\.Lguest_return' \
    'trap return must use the explicit guest-anchor predicate'
require_order "$trap_body" 'bnez[[:space:]]+t0,[[:space:]]+\.Lguest_return' \
    'ld[[:space:]]+t0,[[:space:]]+TRAP_FRAME_T0_OFFSET\(sp\)' \
    'the selected epilogue must reload the only return-predicate scratch register'

host_cpu_stores=$(rg -c 'sd[[:space:]]+tp,[[:space:]]+TRAP_FRAME_HOST_CPU_INDEX_OFFSET\(sp\)' "$trap_body")
if [ "$host_cpu_stores" -ne 2 ]; then
    echo 'both host and guest trap paths must initialize host_cpu_index' >&2
    exit 1
fi

zero_register_stores=$(rg -c 'sd[[:space:]]+zero,[[:space:]]+TRAP_FRAME_GENERAL_OFFSET\(sp\)' "$trap_body")
if [ "$zero_register_stores" -ne 2 ]; then
    echo 'both trap paths must initialize the complete Rust TrapFrame value' >&2
    exit 1
fi

rg -q 'pub const GUEST_HS_ANCHOR_SIZE: u64 = 416;' "$registers" &&
    rg -q '"GUEST_HS_ANCHOR_RA_OFFSET"' "$registers" &&
    rg -q '"GUEST_HS_ANCHOR_S0_OFFSET"' "$registers" &&
    rg -q '"GUEST_HS_ANCHOR_GP_OFFSET"' "$registers" &&
    rg -q '"GUEST_HS_ANCHOR_TP_OFFSET"' "$registers" &&
    rg -q '"GUEST_HS_ANCHOR_CONTEXT_OFFSET"' "$registers" &&
    rg -q '"GUEST_HS_ANCHOR_FLOATING_OFFSET"' "$registers" &&
    rg -q '"GUEST_HS_ANCHOR_SCOUNTEREN_OFFSET"' "$registers" &&
    rg -q '"GUEST_HS_ANCHOR_SENVCFG_OFFSET"' "$registers" &&
    rg -q '"VCPU_HVIP_OFFSET"' "$registers" &&
    rg -q '"VCPU_VSTIMECMP_OFFSET"' "$registers" &&
    rg -q '"VCPU_SCOUNTEREN_OFFSET"' "$registers" &&
    rg -q '"VCPU_SENVCFG_OFFSET"' "$registers" &&
    rg -q '"TRAP_ACTION_ANCHOR_IRQ_TAIL"' "$registers" &&
    rg -q '"TRAP_FRAME_GUEST_ANCHOR_RETURN_OFFSET"' "$registers" &&
    rg -q '"TRAP_FRAME_SIZE"' "$registers" || {
    echo 'Rust must export the complete anchor and trap-frame layout to assembly' >&2
    exit 1
}

require_order "$entry" 'csrr[[:space:]]+t0,[[:space:]]+scounteren' \
    'ld[[:space:]]+t0,[[:space:]]+VCPU_SCOUNTEREN_OFFSET\(t6\)' \
    'guest entry must save host SCOUNTEREN before installing guest state'
require_order "$entry" 'csrr[[:space:]]+t0,[[:space:]]+senvcfg' \
    'ld[[:space:]]+t0,[[:space:]]+VCPU_SENVCFG_OFFSET\(t6\)' \
    'guest entry must save host SENVCFG before installing guest state'
require_order "$trap_body" 'sd[[:space:]]+t1,[[:space:]]+VCPU_SCOUNTEREN_OFFSET\(a0\)' \
    'ld[[:space:]]+t1,[[:space:]]+GUEST_HS_ANCHOR_SCOUNTEREN_OFFSET\(t0\)' \
    'guest trap entry must capture guest SCOUNTEREN before restoring the host'
require_order "$trap_body" 'sd[[:space:]]+t1,[[:space:]]+VCPU_SENVCFG_OFFSET\(a0\)' \
    'ld[[:space:]]+t1,[[:space:]]+GUEST_HS_ANCHOR_SENVCFG_OFFSET\(t0\)' \
    'guest trap entry must capture guest SENVCFG before restoring the host'
require_order "$trap_body" 'sd[[:space:]]+t1,[[:space:]]+VCPU_VSATP_OFFSET\(a0\)' \
    'csrw[[:space:]]+vsatp,[[:space:]]+zero' \
    'guest trap entry must capture and quiesce VSATP before host policy'
if rg -q 'csrr[[:space:]]+\{vsatp\},[[:space:]]+vsatp|vsatp[[:space:]]*=[[:space:]]*out\(reg\)' \
    "$context"; then
    echo 'post-quiesce Rust capture must not overwrite the assembly VSATP snapshot' >&2
    exit 1
fi
rg -U -q 'ld[[:space:]]+t0,[[:space:]]+VCPU_SCOUNTEREN_OFFSET\(t6\)\n[[:space:]]+csrw[[:space:]]+scounteren,[[:space:]]+t0' "$entry" &&
    rg -q 'csrw[[:space:]]+hcounteren,[[:space:]]+t0' "$entry" &&
    ! rg -q 'csrw[[:space:]]+hcounteren' "$guest_rust" || {
    echo 'guest SCOUNTEREN must remain separate from the HCOUNTEREN policy gate' >&2
    exit 1
}

rg -q 'sd[[:space:]]+t1,[[:space:]]+VCPU_HVIP_OFFSET\(a0\)' "$trap_body" &&
    rg -q 'csrc[[:space:]]+hvip,[[:space:]]+t2' "$trap_body" &&
    rg -q 'sd[[:space:]]+t1,[[:space:]]+VCPU_VSTIMECMP_OFFSET\(a0\)' "$trap_body" &&
    rg -q 'csrw[[:space:]]+0x24d,[[:space:]]+t1' "$trap_body" &&
    rg -q 'pub hvip: u64' "$context" &&
    ! rg -q 'pub vsip: u64' "$context" || {
    echo 'guest pending-interrupt and timer state must be owned and quiesced explicitly' >&2
    exit 1
}

rg -q 'fn clear_legacy_software_interrupt\(context: &mut VcpuContext\)' "$guest_rust" &&
    rg -q 'context\.hvip[[:space:]]*&=[[:space:]]*!HVIP_VSSIP' "$guest_rust" || {
    echo 'legacy clear-IPI must update the authoritative quiesced HVIP image' >&2
    exit 1
}

rg -U -q 'if[[:space:]]+!candidate\.supervisor_timer_compare[[:space:]]*\{\n[[:space:]]+return Err\(Error::MissingSstc\);' "$platform" || {
    echo 'the unconditional VSTIMECMP path requires an explicit Sstc platform contract' >&2
    exit 1
}
rg -U -q 'if[[:space:]]+!enable_supervisor_timer_compare\(\)[[:space:]]*\{\n[[:space:]]+return Err\(Error::SupervisorTimerCompareUnavailable\);' "$vm_vcpu" &&
    rg -q 'environment[[:space:]]*&[[:space:]]*HENVCFG_STCE[[:space:]]*!=[[:space:]]*0' \
        "$vm_vcpu" || {
    echo 'every hart must validate firmware STCE enablement before VSTIMECMP access' >&2
    exit 1
}

require_order "$guest_return" 'li[[:space:]]+t0,[[:space:]]+\(3 << 13\)' \
    'ld[[:space:]]+t0,[[:space:]]+TRAP_FRAME_T0_OFFSET\(sp\)' \
    'guest return must enable HS floating state without clobbering guest T0'

rg -U -q 'ld[[:space:]]+t0,[[:space:]]+VCPU_VSATP_OFFSET\(t6\)\n[[:space:]]+csrw[[:space:]]+vsatp,[[:space:]]+t0\n[[:space:]]+hfence\.vvma[[:space:]]+zero,[[:space:]]+zero' \
    "$entry" &&
    rg -U -q 'riscv64_activate_stage2:\n([[:space:]#].*\n)*[[:space:]]+csrw[[:space:]]+vsatp,[[:space:]]+zero\n[[:space:]]+hfence\.vvma[[:space:]]+zero,[[:space:]]+zero' \
        "$guest" &&
    rg -U -q 'ld[[:space:]]+t1,[[:space:]]+VCPU_VSATP_OFFSET\(t0\)\n[[:space:]]+csrw[[:space:]]+vsatp,[[:space:]]+t1\n[[:space:]]+hfence\.vvma[[:space:]]+zero,[[:space:]]+zero' \
        "$trap_body" || {
    echo 'VSATP must be quiesced across HGATP changes and fenced on every install' >&2
    exit 1
}
rg -q 'offset_of!\(TrapFrame, host_cpu_index\)' "$exception" &&
    rg -q 'offset_of!\(TrapFrame, guest_origin\)' "$exception" &&
    rg -q 'offset_of!\(TrapFrame, guest_anchor_return\)' "$exception" &&
    rg -q 'offset_of!\(TrapFrame, guest_context\)' "$exception" &&
    rg -q 'size_of::<TrapAction>\(\)[[:space:]]*==[[:space:]]*16' "$exception" &&
    rg -q 'size_of::<TrapFrame>\(\)[[:space:]]*==[[:space:]]*super::registers::TRAP_FRAME_SIZE' \
        "$exception" || {
    echo 'Rust must compiler-check the assembly-visible TrapFrame layout' >&2
    exit 1
}

require_order "$anchor_exit" 'csrw[[:space:]]+sscratch,[[:space:]]+zero' \
    'addi[[:space:]]+sp,[[:space:]]+sp,[[:space:]]+TRAP_FRAME_SIZE' \
    'typed unwind must close guest-origin publication before destroying the frame'
require_order "$anchor_exit" 'ld[[:space:]]+ra,[[:space:]]+GUEST_HS_ANCHOR_RA_OFFSET\(sp\)' \
    'addi[[:space:]]+sp,[[:space:]]+sp,[[:space:]]+GUEST_HS_ANCHOR_SIZE' \
    'typed unwind must restore the host return ABI before releasing its anchor'
if rg -q 'sret' "$anchor_exit"; then
    echo 'typed IRQ-tail unwind must return to the host anchor, not the guest' >&2
    exit 1
fi

require_order "$invalid_action" 'csrw[[:space:]]+sscratch,[[:space:]]+zero' \
    'call[[:space:]]+riscv64_invalid_trap_action' \
    'invalid trap actions must close guest publication before fail-stop'
if rg -q 'ebreak' "$invalid_action"; then
    echo 'invalid trap actions must not recursively re-enter the live vector' >&2
    exit 1
fi
rg -q 'fn riscv64_invalid_trap_action' "$exception" || {
    echo 'invalid trap actions require a bounded architecture fatal path' >&2
    exit 1
}

rg -q 'capture_guest_irq_tail\(frame\)' "$exception" &&
    rg -q 'TrapAction::anchor_irq_tail\(postlude\)' "$exception" || {
    echo 'guest IRQ postludes must capture state before selecting anchor unwind' >&2
    exit 1
}

rg -q 'fn begin_run\(' "$context" &&
    rg -q 'fn publish_irq_tail\(' "$context" &&
    rg -q 'fn consume_irq_tail\(' "$context" &&
    rg -q 'validate_anchor_state_machine\(\)' "$context" || {
    echo 'Rust must validate exact, one-shot guest-anchor state transitions' >&2
    exit 1
}

rg -q 'self\.virtual_count_offset[[:space:]]*=[[:space:]]*value\.wrapping_sub\(physical\)' \
    "$context" || {
    echo 'RISC-V HTIMEDELTA must model virtual time as physical plus offset' >&2
    exit 1
}

rg -q 'any\(CONFIG_ARCH_AARCH64, CONFIG_ARCH_RISCV64\)' "$selected_exception" &&
    rg -q 'InterruptOrigin::Guest => Some\(postlude\)' "$selected_exception" &&
    rg -q 'dispatch_kernel_rpc_entry\(origin: InterruptOrigin\)' "$kernel_irq" &&
    rg -q 'origin\.is_guest\(\)' "$kernel_irq" || {
    echo 'RISC-V IRQ-tail qualification must retain typed interrupt origin and SSIP accounting' >&2
    exit 1
}

rg -q 'any\(CONFIG_ARCH_AARCH64, CONFIG_ARCH_RISCV64\)' "$kernel_tests" &&
    rg -q 'RISC-V IRQ-tail Fair vCPU preemption passed' "$qemu_verify" || {
    echo 'RISC-V runtime acceptance must prove guest-to-host Fair preemption' >&2
    exit 1
}
