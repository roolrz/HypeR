#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Protect the x86 shared-stage-1 invalidation and lock-progress contract.
set -eu

root=${HYPER_X86_SHOOTDOWN_ROOT:-$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)}
cd "$root"

require() {
    pattern=$1
    source=$2
    message=$3
    if ! rg -q -U "$pattern" "$source"; then
        echo "$message" >&2
        exit 1
    fi
}

function_body() {
    checked_source=$1
    declaration=$2
    sed -n "/^$declaration/,/^}/p" "$checked_source" |
        sed '\#^[[:space:]]*//#d'
}

require_function() {
    checked_source=$1
    declaration=$2
    pattern=$3
    message=$4
    body=$(function_body "$checked_source" "$declaration")
    if [ -z "$body" ] || ! printf '%s\n' "$body" | rg -q -U "$pattern"; then
        echo "$message" >&2
        exit 1
    fi
}

line_in_function() {
    checked_source=$1
    declaration=$2
    pattern=$3
    function_body "$checked_source" "$declaration" |
        rg -n "$pattern" | sed -n '1s/:.*//p'
}

require_function_order() {
    checked_source=$1
    declaration=$2
    first_pattern=$3
    second_pattern=$4
    message=$5
    first=$(line_in_function "$checked_source" "$declaration" "$first_pattern")
    second=$(line_in_function "$checked_source" "$declaration" "$second_pattern")
    if [ -z "$first" ] || [ -z "$second" ] || [ "$first" -ge "$second" ]; then
        echo "$message" >&2
        exit 1
    fi
}

require_function_occurrences() {
    checked_source=$1
    declaration=$2
    pattern=$3
    expected=$4
    message=$5
    count=$(function_body "$checked_source" "$declaration" |
        rg -o "$pattern" 2>/dev/null | wc -l | tr -d ' ')
    if [ "$count" -ne "$expected" ]; then
        echo "$message (expected $expected, found $count)" >&2
        exit 1
    fi
}

reject_commented_contract() {
    checked_source=$1
    declaration=$2
    raw_body=$(sed -n "/^$declaration/,/^}/p" "$checked_source")
    if printf '%s\n' "$raw_body" | rg -q -U \
        '(?s)/\*.*?(Ordering::(Acquire|Release|AcqRel)|REQUESTED_GENERATION|ACKNOWLEDGED_GENERATION|flush_local|send_fixed_ipi|service_pending|end_local_interrupt|"(mfence|lfence|wrmsr)").*?\*/'; then
        echo "commented-out code must not satisfy $declaration" >&2
        exit 1
    fi
}

tlb_source=src/arch/x86_64/tlb.rs
flush_function='pub(super) fn flush_all_online()'
admission_function='pub(super) fn synchronize_online_cpu()'
service_function='pub(super) fn service_pending()'
handler_function='pub(super) fn handle_interrupt'

require_function "$tlb_source" "$flush_function" \
    '^[[:space:]]*REQUESTED_GENERATION\.store\(generation, Ordering::Release\);' \
    'x86 TLB requests must release-publish page-table writes'
require_function "$tlb_source" "$flush_function" \
    '^[[:space:]]*while ACKNOWLEDGED_GENERATION\[cpu\]\.load\(Ordering::Acquire\) != generation' \
    'x86 TLB initiators must acquire remote completion'
require_function_order "$tlb_source" "$flush_function" \
    '^[[:space:]]*REQUESTED_GENERATION\.store' \
    '^[[:space:]]*flush_local\(\);' \
    'x86 TLB request publication must precede local invalidation'
require_function_order "$tlb_source" "$flush_function" \
    '^[[:space:]]*flush_local\(\);' \
    '^[[:space:]]*if !super::interrupt_controller::send_fixed_ipi' \
    'x86 local invalidation must precede remote shootdown delivery'
require_function_order "$tlb_source" "$flush_function" \
    '^[[:space:]]*if !super::interrupt_controller::send_fixed_ipi' \
    '^[[:space:]]*while ACKNOWLEDGED_GENERATION\[cpu\]\.load' \
    'x86 remote delivery must precede acknowledgement waits'
require_function_occurrences "$tlb_source" "$flush_function" \
    'REQUESTED_GENERATION\.store\(' 1 \
    'the shootdown initiator must publish exactly one generation'
require_function_occurrences "$tlb_source" "$flush_function" \
    'flush_local\(\);' 1 \
    'the shootdown initiator must invalidate locally exactly once'
require_function_occurrences "$tlb_source" "$flush_function" \
    'send_fixed_ipi\(' 1 \
    'the shootdown initiator must retain one remote delivery seam'
require_function_occurrences "$tlb_source" "$flush_function" \
    'ACKNOWLEDGED_GENERATION\[cpu\]\.load\(' 1 \
    'the shootdown initiator must retain one remote completion seam'
require_function \
    "$tlb_source" "$admission_function" \
    '^[[:space:]]*let generation = REQUESTED_GENERATION\.load\(Ordering::Acquire\);' \
    'x86 CPU admission must acquire the latest published TLB generation'
require_function \
    "$tlb_source" "$admission_function" \
    '^[[:space:]]*ACKNOWLEDGED_GENERATION\[cpu\.get\(\)\]\.store\(generation, Ordering::Release\);' \
    'x86 CPU admission must release-publish its local invalidation'
require_function_order "$tlb_source" "$admission_function" \
    '^[[:space:]]*let generation = REQUESTED_GENERATION\.load' \
    '^[[:space:]]*flush_local\(\);' \
    'x86 CPU admission must acquire the request before local invalidation'
require_function_order "$tlb_source" "$admission_function" \
    '^[[:space:]]*flush_local\(\);' \
    '^[[:space:]]*ACKNOWLEDGED_GENERATION.*\.store' \
    'x86 CPU admission must acknowledge only after local invalidation'
require_function_occurrences "$tlb_source" "$admission_function" \
    'REQUESTED_GENERATION\.load\(' 1 \
    'x86 CPU admission must observe one generation'
require_function_occurrences "$tlb_source" "$admission_function" \
    'flush_local\(\);' 1 \
    'x86 CPU admission must perform one local invalidation'
require_function_occurrences "$tlb_source" "$admission_function" \
    'ACKNOWLEDGED_GENERATION.*\.store\(' 1 \
    'x86 CPU admission must publish one acknowledgement'
require_function \
    "$tlb_source" "$service_function" \
    '^[[:space:]]*let generation = REQUESTED_GENERATION\.load\(Ordering::Acquire\);' \
    'x86 masked/interrupt progress must acquire the published TLB generation'
require_function \
    "$tlb_source" "$service_function" \
    '^[[:space:]]*ACKNOWLEDGED_GENERATION\[cpu\.get\(\)\]\.store\(generation, Ordering::Release\);' \
    'x86 masked/interrupt progress must release-publish local invalidation'
require_function_order "$tlb_source" "$service_function" \
    '^[[:space:]]*let generation = REQUESTED_GENERATION\.load' \
    '^[[:space:]]*flush_local\(\);' \
    'x86 masked progress must acquire the request before local invalidation'
require_function_order "$tlb_source" "$service_function" \
    '^[[:space:]]*flush_local\(\);' \
    '^[[:space:]]*ACKNOWLEDGED_GENERATION.*\.store' \
    'x86 masked progress must acknowledge only after local invalidation'
require_function_occurrences "$tlb_source" "$service_function" \
    'REQUESTED_GENERATION\.load\(' 1 \
    'x86 masked progress must observe one generation'
require_function_occurrences "$tlb_source" "$service_function" \
    'flush_local\(\);' 1 \
    'x86 masked progress must perform one local invalidation'
require_function_occurrences "$tlb_source" "$service_function" \
    'ACKNOWLEDGED_GENERATION.*\.store\(' 1 \
    'x86 masked progress must publish one acknowledgement'
require_function "$tlb_source" "$handler_function" \
    '^[[:space:]]*service_pending\(\);' \
    'the private TLB handler must service its published generation'
require_function_order "$tlb_source" "$handler_function" \
    '^[[:space:]]*service_pending\(\);' \
    '^[[:space:]]*super::interrupt_controller::end_local_interrupt\(\);' \
    'the private TLB handler must complete invalidation before EOI'
require_function_occurrences "$tlb_source" "$handler_function" \
    'service_pending\(\);' 1 \
    'the private TLB handler must own exactly one service point'
require_function_occurrences "$tlb_source" "$handler_function" \
    'super::interrupt_controller::end_local_interrupt\(\);' 1 \
    'the private TLB handler must own exactly one local-APIC EOI'

reject_commented_contract "$tlb_source" "$flush_function"
reject_commented_contract "$tlb_source" "$admission_function"
reject_commented_contract "$tlb_source" "$service_function"
reject_commented_contract "$tlb_source" "$handler_function"

ipi_source=src/arch/x86_64/interrupt_controller.rs
ipi_function='pub fn send_fixed_ipi'
require_function "$ipi_source" "$ipi_function" \
    '(?m)^[ \t]*"mfence",[ \t]*\r?$\n[ \t]*"lfence",[ \t]*\r?$\n[ \t]*"wrmsr",' \
    'x2APIC fixed IPIs must retain the exact MFENCE;LFENCE;WRMSR sequence'
require_function_order "$ipi_source" "$ipi_function" \
    '^[[:space:]]*"mfence",' \
    '^[[:space:]]*"lfence",' \
    'x2APIC IPI publication requires MFENCE before LFENCE'
require_function_order "$ipi_source" "$ipi_function" \
    '^[[:space:]]*"lfence",' \
    '^[[:space:]]*"wrmsr",' \
    'x2APIC IPI publication requires LFENCE immediately before WRMSR ordering'
require_function "$ipi_source" "$ipi_function" \
    '^[[:space:]]*in\("ecx"\) X2APIC_ICR,' \
    'fixed IPIs must write the x2APIC interrupt-command register'
require_function "$ipi_source" "$ipi_function" \
    '^[[:space:]]*options\(nostack\),' \
    'x2APIC IPI assembly must remain a compiler memory boundary'
if function_body "$ipi_source" "$ipi_function" | rg -q 'options\([^)]*nomem'; then
    echo 'x2APIC IPI assembly must not claim that it leaves memory unobserved' >&2
    exit 1
fi
reject_commented_contract "$ipi_source" "$ipi_function"
require \
    'if super::tlb::handle_interrupt\(vector\) \{[[:space:]]*return;[[:space:]]*\}[[:space:]]*super::virtualization::observe_host_interrupt' \
    src/arch/x86_64/exception.rs \
    'the architecture-private TLB vector must bypass kernel and VM IRQ policy'
require \
    'fn handle_external_interrupt\(\) \{[^}]*VMCS_EXIT_INTERRUPT_INFO[^}]*if super::tlb::handle_interrupt\(vector\) \{[[:space:]]*return;[[:space:]]*\}[[:space:]]*match crate::kernel::entry::irq::dispatch' \
    src/arch/x86_64/vmx.rs \
    'VMX external-interrupt exits must consume the private TLB vector before kernel IRQ policy'
require \
    'fn handle_external_interrupt\(\) \{[^}]*asm!\("stgi", "sti", "nop", "cli", "clgi"' \
    src/arch/x86_64/svm.rs \
    'SVM external-interrupt exits must route host vectors through the checked IDT path'
require \
    'slot\.store\(true, Ordering::Release\);[[:space:]]*super::tlb::synchronize_online_cpu\(\);' \
    src/arch/x86_64/smp.rs \
    'x86 CPU admission must close the online/shootdown snapshot race'
require_function \
    src/arch/x86_64/smp.rs \
    'pub(super) fn for_each_online_remote_cpu' \
    '^[[:space:]]*if .*ONLINE\[index\]\.load\(Ordering::Acquire\)' \
    'x86 shootdown target snapshots must acquire CPU-online publication'
require \
    'fn wait_for_lock_owner\(\) \{[[:space:]]*//[^}]*super::tlb::service_pending\(\);' \
    src/arch/x86_64/interrupts.rs \
    'x86 IRQ-masked lock contention must service pending TLB generations'
require \
    'with_relax\(operation, M::wait_for_lock_owner\)' \
    src/sync/lock/interrupt.rs \
    'IRQ-safe locks must invoke the selected masked-contention progress hook'
relaxed_paths=$(rg -c 'with_relax\(operation, M::wait_for_lock_owner\)' src/sync/lock/interrupt.rs)
if [ "$relaxed_paths" -ne 2 ]; then
    echo "every blocking IRQ-safe acquisition path must make masked progress (found $relaxed_paths, expected 2)" >&2
    exit 1
fi
require \
    'static STACK_SLOTS: StackLock<StackSlots>' \
    src/kernel/mm/stack.rs \
    'STACK_SLOTS must serialize slot ownership and stage-1 mutation'
require \
    'status\.store\(READY, Ordering::Release\)' \
    src/kernel/boot/state.rs \
    'BootState installation must use one-time release publication'
require \
    'status\.load\(Ordering::Acquire\) != READY' \
    src/kernel/boot/state.rs \
    'BootState readers must acquire one-time publication'

flushes=$(rg -c 'super::tlb::flush_all_online\(\);' src/arch/x86_64/memory.rs)
if [ "$flushes" -ne 3 ]; then
    echo "every x86 live stage-1 update must use shared TLB shootdown (found $flushes, expected 3)" >&2
    exit 1
fi

if ! rg -q 'pub\(super\) fn service_pending\(\)' src/arch/x86_64/tlb.rs; then
    echo 'x86 must retain an allocation-free masked shootdown progress path' >&2
    exit 1
fi

private_eoi=$(rg -c 'end_local_interrupt\(\);' src/arch/x86_64/tlb.rs)
if [ "$private_eoi" -ne 1 ]; then
    echo "the private TLB handler must own exactly one local-APIC EOI (found $private_eoi)" >&2
    exit 1
fi

if rg -q 'InterruptSpinLock' src/kernel/boot/state.rs; then
    echo 'immutable BootState access must remain non-blocking after publication' >&2
    exit 1
fi
