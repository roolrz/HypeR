#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Prove that every x86 shared-stage-1 source ratchet rejects its regression.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/hyper-x86-shootdown-test.XXXXXX")
trap 'rm -rf "$fixture"' EXIT HUP INT TERM

mkdir -p \
    "$fixture/src/arch/x86_64" \
    "$fixture/src/kernel/boot" \
    "$fixture/src/kernel/mm" \
    "$fixture/src/sync/lock"

check() {
    HYPER_X86_SHOOTDOWN_ROOT="$fixture" \
        sh "$root/tests/ci/check-x86-stage1-shootdown-contract.sh"
}

write_valid_fixture() {
    printf '%s\n' \
        'pub(super) fn flush_all_online() {' \
        '    REQUESTED_GENERATION.store(generation, Ordering::Release);' \
        '    flush_local();' \
        '    super::smp::for_each_online_remote_cpu(current, |cpu, apic_id| {' \
        '        if !super::interrupt_controller::send_fixed_ipi(apic_id, InterruptId::new(SHOOTDOWN_VECTOR)) {}' \
        '    });' \
        '    while ACKNOWLEDGED_GENERATION[cpu].load(Ordering::Acquire) != generation {}' \
        '}' \
        'pub(super) fn synchronize_online_cpu() {' \
        '    let generation = REQUESTED_GENERATION.load(Ordering::Acquire);' \
        '    flush_local();' \
        '    ACKNOWLEDGED_GENERATION[cpu.get()].store(generation, Ordering::Release);' \
        '}' \
        'pub(super) fn service_pending() {' \
        '    let generation = REQUESTED_GENERATION.load(Ordering::Acquire);' \
        '    flush_local();' \
        '    ACKNOWLEDGED_GENERATION[cpu.get()].store(generation, Ordering::Release);' \
        '}' \
        'pub(super) fn handle_interrupt(vector: u32) -> bool {' \
        '    if vector != SHOOTDOWN_VECTOR { return false; }' \
        '    service_pending();' \
        '    super::interrupt_controller::end_local_interrupt();' \
        '    true' \
        '}' \
        >"$fixture/src/arch/x86_64/tlb.rs"
    printf '%s\n' \
        'pub fn send_fixed_ipi(target: u32, interrupt: InterruptId) -> bool {' \
        '    unsafe {' \
        '        asm!(' \
        '            "mfence",' \
        '            "lfence",' \
        '            "wrmsr",' \
        '            in("ecx") X2APIC_ICR,' \
        '            options(nostack),' \
        '        )' \
        '    };' \
        '    true' \
        '}' >"$fixture/src/arch/x86_64/interrupt_controller.rs"
    printf '%s\n' \
        'fn entry(vector: u32) {' \
        '    if super::tlb::handle_interrupt(vector) {' \
        '        return;' \
        '    }' \
        '    super::virtualization::observe_host_interrupt(vector);' \
        '}' >"$fixture/src/arch/x86_64/exception.rs"
    printf '%s\n' \
        'fn handle_external_interrupt() {' \
        '    let info = VMCS_EXIT_INTERRUPT_INFO;' \
        '    if super::tlb::handle_interrupt(vector) {' \
        '        return;' \
        '    }' \
        '    match crate::kernel::entry::irq::dispatch(interrupt) {}' \
        '}' >"$fixture/src/arch/x86_64/vmx.rs"
    printf '%s\n' \
        'fn handle_external_interrupt() {' \
        '    asm!("stgi", "sti", "nop", "cli", "clgi");' \
        '}' >"$fixture/src/arch/x86_64/svm.rs"
    printf '%s\n' \
        'fn online() {' \
        '    slot.store(true, Ordering::Release);' \
        '    super::tlb::synchronize_online_cpu();' \
        '}' \
        'pub(super) fn for_each_online_remote_cpu() {' \
        '    if !ONLINE[index].load(Ordering::Acquire) {}' \
        '}' >"$fixture/src/arch/x86_64/smp.rs"
    printf '%s\n' \
        'fn wait_for_lock_owner() {' \
        '    // Service architecture-private progress while IRQs are masked.' \
        '    super::tlb::service_pending();' \
        '}' >"$fixture/src/arch/x86_64/interrupts.rs"
    printf '%s\n' \
        'fn with() { lock.with_relax(operation, M::wait_for_lock_owner); }' \
        'fn retained() { lock.with_relax(operation, M::wait_for_lock_owner); }' \
        >"$fixture/src/sync/lock/interrupt.rs"
    printf '%s\n' 'static STACK_SLOTS: StackLock<StackSlots> = value;' \
        >"$fixture/src/kernel/mm/stack.rs"
    printf '%s\n' \
        'fn publish() { status.store(READY, Ordering::Release); }' \
        'fn read() { if status.load(Ordering::Acquire) != READY {} }' \
        >"$fixture/src/kernel/boot/state.rs"
    printf '%s\n' \
        'fn map() { super::tlb::flush_all_online(); }' \
        'fn grow() { super::tlb::flush_all_online(); }' \
        'fn unmap() { super::tlb::flush_all_online(); }' \
        >"$fixture/src/arch/x86_64/memory.rs"
}

mutate() {
    description=$1
    source=$2
    expression=$3
    write_valid_fixture
    sed "$expression" "$fixture/$source" >"$fixture/mutated"
    mv "$fixture/mutated" "$fixture/$source"
    if check >/dev/null 2>&1; then
        echo "$description" >&2
        exit 1
    fi
}

mutate_three() {
    description=$1
    source=$2
    first_expression=$3
    second_expression=$4
    third_expression=$5
    write_valid_fixture
    sed \
        -e "$first_expression" \
        -e "$second_expression" \
        -e "$third_expression" \
        "$fixture/$source" >"$fixture/mutated"
    mv "$fixture/mutated" "$fixture/$source"
    if check >/dev/null 2>&1; then
        echo "$description" >&2
        exit 1
    fi
}

write_valid_fixture
check

mutate 'request publication must require Release ordering' \
    src/arch/x86_64/tlb.rs \
    's/REQUESTED_GENERATION.store(generation, Ordering::Release)/REQUESTED_GENERATION.store(generation, Ordering::Relaxed)/'
mutate_three 'request publication must precede local invalidation' \
    src/arch/x86_64/tlb.rs \
    '/fn flush_all_online/,/^}/s@    REQUESTED_GENERATION.store(generation, Ordering::Release);@    __REQUEST_PUBLICATION__;@' \
    '/fn flush_all_online/,/^}/s@    flush_local();@    REQUESTED_GENERATION.store(generation, Ordering::Release);@' \
    '/fn flush_all_online/,/^}/s@    __REQUEST_PUBLICATION__;@    flush_local();@'
mutate 'remote acknowledgement must require Release ordering' \
    src/arch/x86_64/tlb.rs \
    '/fn synchronize_online_cpu/,/^}/s/store(generation, Ordering::Release)/store(generation, Ordering::Relaxed)/'
mutate 'masked progress acknowledgement must require Release ordering' \
    src/arch/x86_64/tlb.rs \
    '/fn service_pending/,/^}/s/store(generation, Ordering::Release)/store(generation, Ordering::Relaxed)/'
mutate 'initiator acknowledgement observation must require Acquire ordering' \
    src/arch/x86_64/tlb.rs \
    's/ACKNOWLEDGED_GENERATION\[cpu\].load(Ordering::Acquire)/ACKNOWLEDGED_GENERATION[cpu].load(Ordering::Relaxed)/'
mutate 'CPU admission must acquire the published request generation' \
    src/arch/x86_64/tlb.rs \
    '/fn synchronize_online_cpu/,/^}/s/Ordering::Acquire/Ordering::Relaxed/'
mutate 'masked progress must acquire the published request generation' \
    src/arch/x86_64/tlb.rs \
    '/fn service_pending/,/^}/s/Ordering::Acquire/Ordering::Relaxed/'
mutate_three 'local invalidation must precede remote IPI delivery' \
    src/arch/x86_64/tlb.rs \
    '/fn flush_all_online/,/^}/s@    flush_local();@    __LOCAL_INVALIDATION__;@' \
    '/fn flush_all_online/,/^}/s@        if !super::interrupt_controller::send_fixed_ipi(apic_id, InterruptId::new(SHOOTDOWN_VECTOR)) {}@    flush_local();@' \
    '/fn flush_all_online/,/^}/s@    __LOCAL_INVALIDATION__;@        if !super::interrupt_controller::send_fixed_ipi(apic_id, InterruptId::new(SHOOTDOWN_VECTOR)) {}@'
mutate_three 'remote IPI delivery must precede acknowledgement waits' \
    src/arch/x86_64/tlb.rs \
    '/fn flush_all_online/,/^}/s@        if !super::interrupt_controller::send_fixed_ipi(apic_id, InterruptId::new(SHOOTDOWN_VECTOR)) {}@        __REMOTE_DELIVERY__;@' \
    '/fn flush_all_online/,/^}/s@    while ACKNOWLEDGED_GENERATION\[cpu\].load(Ordering::Acquire) != generation {}@        if !super::interrupt_controller::send_fixed_ipi(apic_id, InterruptId::new(SHOOTDOWN_VECTOR)) {}@' \
    '/fn flush_all_online/,/^}/s@        __REMOTE_DELIVERY__;@    while ACKNOWLEDGED_GENERATION[cpu].load(Ordering::Acquire) != generation {}@'
mutate_three 'CPU admission must acquire before local invalidation' \
    src/arch/x86_64/tlb.rs \
    '/fn synchronize_online_cpu/,/^}/s@    let generation = REQUESTED_GENERATION.load(Ordering::Acquire);@    __REQUEST_ACQUIRE__;@' \
    '/fn synchronize_online_cpu/,/^}/s@    flush_local();@    let generation = REQUESTED_GENERATION.load(Ordering::Acquire);@' \
    '/fn synchronize_online_cpu/,/^}/s@    __REQUEST_ACQUIRE__;@    flush_local();@'
mutate_three 'CPU admission must invalidate before acknowledgement' \
    src/arch/x86_64/tlb.rs \
    '/fn synchronize_online_cpu/,/^}/s@    flush_local();@    __LOCAL_INVALIDATION__;@' \
    '/fn synchronize_online_cpu/,/^}/s@    ACKNOWLEDGED_GENERATION\[cpu.get()\].store(generation, Ordering::Release);@    flush_local();@' \
    '/fn synchronize_online_cpu/,/^}/s@    __LOCAL_INVALIDATION__;@    ACKNOWLEDGED_GENERATION[cpu.get()].store(generation, Ordering::Release);@'
mutate_three 'masked progress must acquire before local invalidation' \
    src/arch/x86_64/tlb.rs \
    '/fn service_pending/,/^}/s@    let generation = REQUESTED_GENERATION.load(Ordering::Acquire);@    __REQUEST_ACQUIRE__;@' \
    '/fn service_pending/,/^}/s@    flush_local();@    let generation = REQUESTED_GENERATION.load(Ordering::Acquire);@' \
    '/fn service_pending/,/^}/s@    __REQUEST_ACQUIRE__;@    flush_local();@'
mutate_three 'masked progress must invalidate before acknowledgement' \
    src/arch/x86_64/tlb.rs \
    '/fn service_pending/,/^}/s@    flush_local();@    __LOCAL_INVALIDATION__;@' \
    '/fn service_pending/,/^}/s@    ACKNOWLEDGED_GENERATION\[cpu.get()\].store(generation, Ordering::Release);@    flush_local();@' \
    '/fn service_pending/,/^}/s@    __LOCAL_INVALIDATION__;@    ACKNOWLEDGED_GENERATION[cpu.get()].store(generation, Ordering::Release);@'
mutate 'the private handler must service pending generations' \
    src/arch/x86_64/tlb.rs \
    '/fn handle_interrupt/,/^}/s/service_pending()/spin_loop()/'
mutate_three 'private generation service must precede local EOI' \
    src/arch/x86_64/tlb.rs \
    '/fn handle_interrupt/,/^}/s@    service_pending();@    __GENERATION_SERVICE__;@' \
    '/fn handle_interrupt/,/^}/s@    super::interrupt_controller::end_local_interrupt();@    service_pending();@' \
    '/fn handle_interrupt/,/^}/s@    __GENERATION_SERVICE__;@    super::interrupt_controller::end_local_interrupt();@'
mutate 'IDT entry must retain architecture-private interception' \
    src/arch/x86_64/exception.rs \
    's/super::tlb::handle_interrupt(vector)/false/'
mutate 'VMX exits must retain architecture-private interception' \
    src/arch/x86_64/vmx.rs \
    's/super::tlb::handle_interrupt(vector)/false/'
mutate 'SVM exits must retain checked IDT delivery' \
    src/arch/x86_64/svm.rs \
    's/"nop", //'
mutate 'CPU admission must synchronize with a concurrent snapshot' \
    src/arch/x86_64/smp.rs \
    's/synchronize_online_cpu/synchronize_later/'
mutate 'shootdown snapshots must acquire CPU-online publication' \
    src/arch/x86_64/smp.rs \
    's/ONLINE\[index\].load(Ordering::Acquire)/ONLINE[index].load(Ordering::Relaxed)/'
mutate 'IRQ-masked lock waits must service private progress' \
    src/arch/x86_64/interrupts.rs \
    's/service_pending/spin_loop/'
mutate 'both IRQ-safe lock acquisition paths must invoke progress' \
    src/sync/lock/interrupt.rs \
    '2s/with_relax/with/'
mutate 'stack slots must remain the stage-1 mutation serializer' \
    src/kernel/mm/stack.rs \
    's/StackLock/SpinLock/'
mutate 'BootState publication must require Release ordering' \
    src/kernel/boot/state.rs \
    's/Ordering::Release/Ordering::Relaxed/'
mutate 'BootState observation must require Acquire ordering' \
    src/kernel/boot/state.rs \
    's/Ordering::Acquire/Ordering::Relaxed/'
mutate 'every live stage-1 mutation must shoot down remote TLBs' \
    src/arch/x86_64/memory.rs \
    '3s/flush_all_online/flush_local/'
mutate 'masked lock contention must retain its service endpoint' \
    src/arch/x86_64/tlb.rs \
    's/pub(super) fn service_pending/fn service_pending/'
mutate 'the private handler must own exactly one EOI' \
    src/arch/x86_64/tlb.rs \
    '/fn handle_interrupt/,/^}/s/end_local_interrupt()/finish_later()/'
mutate 'fixed IPI publication must retain MFENCE' \
    src/arch/x86_64/interrupt_controller.rs \
    's/"mfence"/"nop"/'
mutate 'fixed IPI publication must retain LFENCE' \
    src/arch/x86_64/interrupt_controller.rs \
    's/"lfence"/"nop"/'
mutate_three 'fixed IPI publication fences must retain their prescribed order' \
    src/arch/x86_64/interrupt_controller.rs \
    's/"mfence"/"__FIRST_FENCE__"/' \
    's/"lfence"/"mfence"/' \
    's/"__FIRST_FENCE__"/"lfence"/'
mutate 'fixed IPIs must target the x2APIC ICR' \
    src/arch/x86_64/interrupt_controller.rs \
    's/X2APIC_ICR/X2APIC_EOI/'
mutate 'fixed IPI assembly must remain a compiler memory boundary' \
    src/arch/x86_64/interrupt_controller.rs \
    's/options(nostack)/options(nomem, nostack)/'
mutate 'immutable BootState must not regain a blocking IRQ-safe lock' \
    src/kernel/boot/state.rs \
    '1s/^/InterruptSpinLock /'

write_valid_fixture
awk '
    /REQUESTED_GENERATION.store\(generation, Ordering::Release\)/ {
        print "    // REQUESTED_GENERATION.store(generation, Ordering::Release);"
        sub(/Ordering::Release/, "Ordering::Relaxed")
    }
    { print }
' "$fixture/src/arch/x86_64/tlb.rs" >"$fixture/mutated"
mv "$fixture/mutated" "$fixture/src/arch/x86_64/tlb.rs"
if check >/dev/null 2>&1; then
    echo 'comment-only ordering expressions must not satisfy the shootdown contract' >&2
    exit 1
fi

write_valid_fixture
awk '
    /REQUESTED_GENERATION.store\(generation, Ordering::Release\)/ {
        print "    /* REQUESTED_GENERATION.store(generation, Ordering::Release); */"
        sub(/Ordering::Release/, "Ordering::Relaxed")
    }
    { print }
' "$fixture/src/arch/x86_64/tlb.rs" >"$fixture/mutated"
mv "$fixture/mutated" "$fixture/src/arch/x86_64/tlb.rs"
if check >/dev/null 2>&1; then
    echo 'block-comment ordering expressions must not satisfy the shootdown contract' >&2
    exit 1
fi

write_valid_fixture
sed '/fn handle_interrupt/,/^}/s/end_local_interrupt()/finish_later()/' \
    "$fixture/src/arch/x86_64/tlb.rs" >"$fixture/mutated"
printf '%s\n' \
    'fn unrelated_decoy() { super::interrupt_controller::end_local_interrupt(); }' \
    >>"$fixture/mutated"
mv "$fixture/mutated" "$fixture/src/arch/x86_64/tlb.rs"
if check >/dev/null 2>&1; then
    echo 'an unrelated EOI must not satisfy private-handler ownership' >&2
    exit 1
fi
