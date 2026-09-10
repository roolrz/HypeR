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
isa=src/arch/riscv64/isa.rs
arch_module=src/arch/riscv64/mod.rs
boot=src/kernel/boot/mod.rs
main=src/main.rs
vm_vcpu=src/arch/riscv64/vm_vcpu.rs
selected_exception=src/hal/selected/exception.rs
kernel_irq=src/kernel/entry/irq.rs
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

# The platform collector delegates ISA parsing to the same per-CPU qualifier
# used by the Native guest metadata guarantee. Keep the complete fail-closed
# chain, rather than matching the removed boolean-only FDT parser.
rg -U -q 'candidate[[:space:]]+\.isa[[:space:]]+\.validate\(node\.enabled\)' "$platform" &&
    rg -F -q 'Missing::Timer => Error::MissingSstc' "$platform" &&
    rg -U -q 'if bits & SSTC == 0 \{[[:space:]]+return Err\(Missing::Timer\);' "$isa" &&
    rg -F -q 'b"sstc" => SSTC' "$isa" &&
    rg -F -q 'platform::guest_baseline_available()' "$arch_module" || {
    echo 'the unconditional VSTIMECMP path requires every enabled CPU to qualify Sstc' >&2
    exit 1
}
rg -U -q 'if[[:space:]]+!enable_supervisor_timer_compare\(\)[[:space:]]*\{\n[[:space:]]+return Err\(Error::SupervisorTimerCompareUnavailable\);' "$vm_vcpu" &&
    rg -q 'environment[[:space:]]*&[[:space:]]*\(1 << 63\)[[:space:]]*!=[[:space:]]*0' \
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

rg -q 'kernel self-tests completed' "$qemu_verify" || {
    echo 'RISC-V runtime acceptance must require completed kernel self-tests' >&2
    exit 1
}

# Guest register bank snapshots are authoritative after vector entry. Every
# register must be captured before Rust, and installed on both return paths.
for bank in VSSTATUS VSIE VSTVEC VSSCRATCH VSEPC VSCAUSE VSTVAL HVIP VSTIMECMP; do
    require_order "$trap_body" "sd[[:space:]]+t1,[[:space:]]+VCPU_${bank}_OFFSET\\(a0\\)" \
        'call[[:space:]]+riscv64_trap_dispatch' \
        "guest $bank must be captured before Rust policy"
    rg -q "ld[[:space:]]+t[01],[[:space:]]+VCPU_${bank}_OFFSET\\(t6\\)" "$entry" &&
        rg -q "ld[[:space:]]+t[12],[[:space:]]+VCPU_${bank}_OFFSET\\(t0\\)" "$trap_body" || {
        echo "guest $bank must be restored for initial entry and direct trap return" >&2
        exit 1
    }
done
require_order "$trap_body" 'sd[[:space:]]+t1,[[:space:]]+VCPU_VSIE_OFFSET\(a0\)' \
    'csrw[[:space:]]+vsie,[[:space:]]+zero' \
    'VS interrupt delivery must quiesce only after the guest enable snapshot'
require_order "$trap_body" 'sd[[:space:]]+t1,[[:space:]]+VCPU_HVIP_OFFSET\(a0\)' \
    'csrc[[:space:]]+hvip,[[:space:]]+t2' \
    'VSSIP and VSEIP snapshots must precede clearing local virtual delivery'

rg -U -q 'srli t1, t1, 8\n[[:space:]]+andi t1, t1, 1\n[[:space:]]+sd t1, VCPU_SUPERVISOR_OFFSET\(a0\)' "$trap_body" &&
    rg -q 'ld t0, VCPU_SUPERVISOR_OFFSET\(t6\)' "$entry" &&
    rg -q 'ld t1, VCPU_SUPERVISOR_OFFSET\(t6\)' "$entry" || {
    echo 'guest VU/VS privilege must survive a detached IRQ-tail continuation' >&2
    exit 1
}

rg -q 'struct StoppedGuestRun' "$context" &&
    rg -q 'not_send_or_sync: core::marker::PhantomData<alloc::rc::Rc<\(\)>>' "$context" &&
    rg -q 'impl Drop for StoppedGuestRun' "$context" &&
    rg -q '!core::ptr::eq\(self.context, context\)' "$context" &&
    rg -q 'TRAP_ACTION_ANCHOR_STOPPED' "$exception" &&
    rg -q 'GUEST_ANCHOR_EXIT_STOPPED' "$anchor_exit" || {
    echo 'terminal and WFI exits require an exact non-transferable stopped-run proof' >&2
    exit 1
}
require_order "$vm_vcpu" 'stopped.validate_for\(context\)' \
    'unsafe \{ deactivate\(context, vcpu_id, interrupts, physical_count\) \}' \
    'stopped hardware detach must first validate the exact captured owner'
require_order "$vm_vcpu" 'unsafe \{ deactivate\(context, vcpu_id, interrupts, physical_count\) \}' \
    'stopped.consume_for\(context\)' \
    'the stopped-run proof must remain armed until hardware detach succeeds'
rg -q '"csrw hgatp, zero"' "$vm_vcpu" &&
    rg -q '"hfence.gvma zero, zero"' "$vm_vcpu" &&
    rg -q '"csrw htimedelta, zero"' "$vm_vcpu" || {
    echo 'guest detach must remove local stage-2 and virtual-time ownership' >&2
    exit 1
}
# PLIC reconciliation owns VSEIP alone; VSSIP must retain guest clear semantics.
rg -F -q 'context.hvip = (context.hvip & !(1 << 10)) | (u64::from(pending) << 10)' "$vm_vcpu" || {
    echo 'PLIC reconciliation must preserve unrelated guest pending interrupts' >&2
    exit 1
}

# Admission probes firmware delegation on every CPU without leaving probe
# state installed. Activation separately commits STCE and checks the readback.
rg -q 'accepted & \(1 << 63\) != 0' "$vm_vcpu" &&
    rg -F -q '"csrw henvcfg, {original}"' "$vm_vcpu" &&
    rg -F -q 'vm_vcpu::discover_local_timer()' "$arch_module" &&
    rg -U -q 'pub fn secondary_cpu_is_compatible\(\) -> bool \{\n[[:space:]]+prepare_primary_cpu_admission\(\)' "$arch_module" &&
    rg -F -q 'if !crate::hal::cpu::prepare_primary_admission()' "$boot" &&
    rg -F -q 'if !crate::hal::cpu::secondary_is_compatible()' "$main" || {
    echo 'primary and secondary admission must probe STCE and restore host HENVCFG' >&2
    exit 1
}
require_order "$vm_vcpu" 'if !enable_supervisor_timer_compare\(\)' \
    'context.activate_system_registers\(\)' \
    'activation must validate committed STCE before installing any guest bank'

# Enabling SIE in the legacy preparation hook admits a host-origin interrupt
# while a vCPU owns hardware but has not published its guest anchor. Only the
# final SRET may make interrupts deliverable for this returning backend.
rg -U -q 'pub const fn prepare_interrupts_for_guest_entry\(\)[[:space:]]*\{[[:space:]]*\}' "$arch_module" || {
    echo 'guest preparation must preserve masked interrupts until anchor publication and SRET' >&2
    exit 1
}
rg -F -q '"csrr {status}, sstatus"' "$context" &&
    rg -F -q '"csrr {scratch}, sscratch"' "$context" &&
    rg -U -q 'if status & registers::SSTATUS_SIE != 0 \|\| scratch != 0 \{\n[[:space:]]+return Err\(GuestRunError::State\);' "$context" || {
    echo 'returning guest runs must reject enabled IRQs or an already published anchor' >&2
    exit 1
}
require_order "$context" 'if status & registers::SSTATUS_SIE != 0 \|\| scratch != 0' \
    'if unsafe \{ \(&mut \*context\).begin_run\(\) \}' \
    'the masked, empty-anchor check must precede guest run-state publication'

# VSIE bits are read-only zero while their HIDELEG gates are closed. Restore
# delegation first on every entry, including migration to a previously unused
# hart; otherwise restoring STIE/SEIE/SSIE silently loses the saved state.
require_order "$entry" 'csrw[[:space:]]+hideleg,[[:space:]]+t0' \
    'csrw[[:space:]]+vsie,[[:space:]]+t0' \
    'HIDELEG must expose virtual interrupt enable bits before restoring VSIE'

# Cached residency does not prove HGATP remains installed after a stopped
# detach. Reject Bare/invalid roots before publishing the active hardware owner.
rg -F -q '"csrr {value}, hgatp"' "$vm_vcpu" &&
    rg -U -q 'if hgatp >> 60 != 8 \|\| root == 0 \|\| root & 0x3fff != 0 \{\n[[:space:]]+return Err\(Error::Stage2NotSelected\);' "$vm_vcpu" || {
    echo 'guest activation must reject a missing Sv39x4 hardware selection' >&2
    exit 1
}
require_order "$vm_vcpu" 'if hgatp >> 60 != 8' \
    'context.activate_system_registers\(\)' \
    'guest activation must verify HGATP before installing context-owned state'
require_order "$vm_vcpu" 'if hgatp >> 60 != 8' \
    'store\(core::ptr::from_mut\(context\).addr\(\)' \
    'guest activation must verify HGATP before publishing its active owner'

# A VU HS-qualified CSR raises virtual-instruction in HS. With no nested-H
# guest support it must become a guest illegal trap, never supervisor emulation.
rg -q 'GuestSyncExit::IllegalInstruction' "$guest_rust" &&
    rg -q 'Completion::IllegalInstruction' "$guest_rust" &&
    rg -F -q 'context.vsepc = exit.program_counter;' "$guest_rust" &&
    rg -F -q 'context.vscause = 2;' "$guest_rust" &&
    rg -F -q 'context.vstval = exit.instruction;' "$guest_rust" &&
    rg -F -q 'program_counter: context.vstvec & !3' "$guest_rust" &&
    rg -F -q 'frame.sstatus |= 1 << 8;' "$guest_rust" &&
    rg -F -q 'frame.hstatus |= (1 << 7) | (1 << 8);' "$guest_rust" &&
    rg -F -q 'validate_illegal_instruction_traps()' "$guest_rust" || {
    echo 'guest illegal instruction forwarding must preserve its trap bank and VS return privilege' >&2
    exit 1
}
require_order "$guest_rust" 'if frame.hstatus & \(1 << 8\) == 0' \
    'return illegal_instruction\(frame\)' \
    'VU virtual instructions must be forwarded before privileged emulation'
