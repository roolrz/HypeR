#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Protect the x86 shared-stage-1 invalidation and lock-progress contract.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
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

require \
    'REQUESTED_GENERATION\.store\(generation, Ordering::Release\)' \
    src/arch/x86_64/tlb.rs \
    'x86 TLB requests must release-publish page-table writes'
require \
    'ACKNOWLEDGED_GENERATION\[cpu\.get\(\)\]\.store\(generation, Ordering::Release\)' \
    src/arch/x86_64/tlb.rs \
    'x86 TLB handlers must release-publish completion'
require \
    'ACKNOWLEDGED_GENERATION\[cpu\]\.load\(Ordering::Acquire\) != generation' \
    src/arch/x86_64/tlb.rs \
    'x86 TLB initiators must acquire remote completion'
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
