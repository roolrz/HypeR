#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Exercise the shared x86 Kernel RPC/TLB contract against deliberate regressions.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/hyper-x86-rpc-test.XXXXXX")
trap 'rm -rf "$fixture"' EXIT HUP INT TERM

copy_sources() {
    rm -rf "$fixture/src"
    mkdir -p "$fixture/src/arch/aarch64" "$fixture/src/arch/riscv64" \
        "$fixture/src/arch/x86_64" "$fixture/src/kernel/boot" \
        "$fixture/src/kernel/irq" "$fixture/src/kernel/mm" "$fixture/src/sync/lock"
    cp "$root/src/arch/irq.rs" "$fixture/src/arch/irq.rs"
    cp "$root/src/arch/aarch64/smp.rs" "$fixture/src/arch/aarch64/smp.rs"
    cp "$root/src/arch/riscv64/smp.rs" "$fixture/src/arch/riscv64/smp.rs"
    for source_file in tlb.rs exception.rs vmx.rs interrupts.rs platform.rs smp.rs interrupt_controller.rs; do
        cp "$root/src/arch/x86_64/$source_file" "$fixture/src/arch/x86_64/$source_file"
    done
    cp "$root/src/kernel/irq/cross_call.rs" "$fixture/src/kernel/irq/cross_call.rs"
    cp "$root/src/kernel/irq/mod.rs" "$fixture/src/kernel/irq/mod.rs"
    cp "$root/src/arch/x86_64/memory.rs" "$fixture/src/arch/x86_64/memory.rs"
    cp "$root/src/kernel/boot/state.rs" "$fixture/src/kernel/boot/state.rs"
    cp "$root/src/kernel/mm/stack.rs" "$fixture/src/kernel/mm/stack.rs"
    cp "$root/src/sync/lock/interrupt.rs" "$fixture/src/sync/lock/interrupt.rs"
    cp "$root/src/sync/publication.rs" "$fixture/src/sync/publication.rs"
}

check() {
    HYPER_X86_SHOOTDOWN_ROOT="$fixture" \
        sh "$root/tests/ci/check-x86-stage1-shootdown-contract.sh"
}

mutate() {
    description=$1
    file=$2
    expression=$3
    copy_sources
    before=$(cksum "$fixture/$file")
    sed "$expression" "$fixture/$file" >"$fixture/mutated"
    mv "$fixture/mutated" "$fixture/$file"
    after=$(cksum "$fixture/$file")
    if [ "$before" = "$after" ]; then
        echo "mutation did not change $file: $description" >&2
        exit 1
    fi
    if check >/dev/null 2>&1; then
        echo "$description" >&2
        exit 1
    fi
}

mutate_three() {
    description=$1
    file=$2
    first_expression=$3
    second_expression=$4
    third_expression=$5
    copy_sources
    before=$(cksum "$fixture/$file")
    sed -e "$first_expression" -e "$second_expression" -e "$third_expression" \
        "$fixture/$file" >"$fixture/mutated"
    mv "$fixture/mutated" "$fixture/$file"
    after=$(cksum "$fixture/$file")
    if [ "$before" = "$after" ]; then
        echo "mutation did not change $file: $description" >&2
        exit 1
    fi
    if check >/dev/null 2>&1; then
        echo "$description" >&2
        exit 1
    fi
}

copy_sources
check

mutate 'TLB request Release mutation was accepted' src/arch/x86_64/tlb.rs \
    's/REQUESTED_GENERATION.store(generation, Ordering::Release)/REQUESTED_GENERATION.store(generation, Ordering::Relaxed)/'
mutate_three 'TLB request publication after local invalidation was accepted' \
    src/arch/x86_64/tlb.rs \
    '/fn flush_all_online/,/^}/s@    REQUESTED_GENERATION.store(generation, Ordering::Release);@    __REQUEST_PUBLICATION__;@' \
    '/fn flush_all_online/,/^}/s@    flush_local();@    REQUESTED_GENERATION.store(generation, Ordering::Release);@' \
    '/fn flush_all_online/,/^}/s@    __REQUEST_PUBLICATION__;@    flush_local();@'
mutate 'TLB acknowledgement Acquire mutation was accepted' src/arch/x86_64/tlb.rs \
    's/ACKNOWLEDGED_GENERATION\[cpu\].load(Ordering::Acquire)/ACKNOWLEDGED_GENERATION[cpu].load(Ordering::Relaxed)/'
mutate 'TLB acknowledgement Release mutation was accepted' src/arch/x86_64/tlb.rs \
    's/ACKNOWLEDGED_GENERATION\[cpu.get()\].store(generation, Ordering::Release)/ACKNOWLEDGED_GENERATION[cpu.get()].store(generation, Ordering::Relaxed)/'
mutate 'CPU admission Acquire mutation was accepted' src/arch/x86_64/tlb.rs \
    '/fn synchronize_online_cpu/,/^}/s/REQUESTED_GENERATION.load(Ordering::Acquire)/REQUESTED_GENERATION.load(Ordering::Relaxed)/'
mutate_three 'CPU admission acknowledgement before invalidation was accepted' \
    src/arch/x86_64/tlb.rs \
    '/fn synchronize_online_cpu/,/^}/s@    flush_local();@    __LOCAL_INVALIDATION__;@' \
    '/fn synchronize_online_cpu/,/^}/s@    ACKNOWLEDGED_GENERATION\[cpu.get()\].store(generation, Ordering::Release);@    flush_local();@' \
    '/fn synchronize_online_cpu/,/^}/s@    __LOCAL_INVALIDATION__;@    ACKNOWLEDGED_GENERATION[cpu.get()].store(generation, Ordering::Release);@'
mutate 'masked service Acquire mutation was accepted' src/arch/x86_64/tlb.rs \
    '/fn service_pending/,/^}/s/REQUESTED_GENERATION.load(Ordering::Acquire)/REQUESTED_GENERATION.load(Ordering::Relaxed)/'
mutate 'masked service duplicate-generation check mutation was accepted' src/arch/x86_64/tlb.rs \
    '/fn service_pending/,/^}/s/== generation/!= generation/'
mutate_three 'masked service acknowledgement before invalidation was accepted' \
    src/arch/x86_64/tlb.rs \
    '/fn service_pending/,/^}/s@    flush_local();@    __LOCAL_INVALIDATION__;@' \
    '/fn service_pending/,/^}/s@    ACKNOWLEDGED_GENERATION\[cpu.get()\].store(generation, Ordering::Release);@    flush_local();@' \
    '/fn service_pending/,/^}/s@    __LOCAL_INVALIDATION__;@    ACKNOWLEDGED_GENERATION[cpu.get()].store(generation, Ordering::Release);@'
mutate 'TLB polling mutation was accepted' src/arch/x86_64/tlb.rs \
    's/crate::arch::irq::service_kernel_rpc()/core::hint::spin_loop()/'
mutate 'shared reason mutation was accepted' src/arch/x86_64/tlb.rs \
    's/STAGE1_TLB_SHOOTDOWN/LOCAL_IRQ_LIFECYCLE/'
mutate 'Kernel RPC wait progress mutation was accepted' src/kernel/irq/cross_call.rs \
    's/        service();/        core::hint::spin_loop();/'
mutate 'Kernel RPC acknowledgement Acquire mutation was accepted' src/kernel/irq/cross_call.rs \
    's/ACK\[cpu\].load(Ordering::Acquire)/ACK[cpu].load(Ordering::Relaxed)/'
mutate 'Kernel RPC acknowledgement Release mutation was accepted' src/kernel/irq/cross_call.rs \
    's/ACK\[cpu\].store(generation, Ordering::Release)/ACK[cpu].store(generation, Ordering::Relaxed)/'
mutate 'Kernel RPC subset mutation was accepted' src/kernel/irq/cross_call.rs \
    's/if !targeted {/if false {/'
mutate 'IDT interception mutation was accepted' src/arch/x86_64/exception.rs \
    's/irq::service_kernel_rpc()/irq::service_rpc_later()/'
mutate 'VMX interception mutation was accepted' src/arch/x86_64/vmx.rs \
    's/irq::service_kernel_rpc()/irq::service_rpc_later()/'
mutate 'masked progress mutation was accepted' src/arch/x86_64/interrupts.rs \
    's/irq::service_kernel_rpc()/core::hint::spin_loop()/'
mutate 'AArch64 reason Release mutation was accepted' src/arch/aarch64/smp.rs \
    's/fetch_or(reasons, Ordering::Release)/fetch_or(reasons, Ordering::Relaxed)/'
mutate 'RISC-V reason Acquire mutation was accepted' src/arch/riscv64/smp.rs \
    's/swap(0, Ordering::Acquire)/swap(0, Ordering::Relaxed)/'
mutate 'x86 reason Release mutation was accepted' src/arch/x86_64/smp.rs \
    's/fetch_or(reasons, Ordering::Release)/fetch_or(reasons, Ordering::Relaxed)/'
mutate 'CPU admission/shootdown synchronization mutation was accepted' src/arch/x86_64/smp.rs \
    's/synchronize_online_cpu/synchronize_online_cpu_later/'
mutate 'online snapshot Acquire mutation was accepted' src/arch/x86_64/smp.rs \
    '/fn for_each_online_remote_cpu/,/^}/s/ONLINE\[index\].load(Ordering::Acquire)/ONLINE[index].load(Ordering::Relaxed)/'
mutate 'one IRQ-safe acquisition path skipped RPC progress' src/sync/lock/interrupt.rs \
    '/pub unsafe fn with_mask_retained/,/^    }/s/with_relax(operation, M::wait_for_lock_owner)/with(operation)/'
mutate 'stage-1 mutation serializer mutation was accepted' src/kernel/mm/stack.rs \
    's/static STACK_SLOTS: StackLock<StackSlots>/static STACK_SLOTS: SpinLock<StackSlots>/'
mutate 'one-shot publication Release mutation was accepted' src/sync/publication.rs \
    's/state.store(READY, Ordering::Release)/state.store(READY, Ordering::Relaxed)/'
mutate 'one-shot publication Acquire mutation was accepted' src/sync/publication.rs \
    's/state.load(Ordering::Acquire) != READY/state.load(Ordering::Relaxed) != READY/'
mutate 'one-shot publication lost its Send bound' src/sync/publication.rs \
    's/T: Send + Sync/T: Sync/'
mutate 'BootState bypassed one-shot publication' src/kernel/boot/state.rs \
    's/PublishedOnce<BootState>/UnsafeCell<BootState>/'
mutate 'live unmap without all-CPU shootdown was accepted' src/arch/x86_64/memory.rs \
    '/pub unsafe fn unmap_stack/,/^    }/s/super::tlb::flush_all_online()/flush_local()/'
mutate 'x2APIC publication fence mutation was accepted' src/arch/x86_64/interrupt_controller.rs \
    '/            "mfence",/d'
mutate 'x2APIC compiler-memory mutation was accepted' src/arch/x86_64/interrupt_controller.rs \
    '/^pub fn send_fixed_ipi(/,/^}/s/options(nostack)/options(nomem, nostack)/'
mutate 'Kernel RPC bypassed one-shot publication' src/arch/irq.rs \
    's/PublishedOnce<fn()>/UnsafeCell<fn()>/'
mutate 'late Kernel RPC callback installation was accepted' src/kernel/irq/mod.rs \
    's/install_kernel_rpc_service(cross_call::service)/install_kernel_rpc_service_later(cross_call::service)/'

copy_sources
awk '
    /REQUESTED_GENERATION.store\(generation, Ordering::Release\)/ {
        print "    // REQUESTED_GENERATION.store(generation, Ordering::Release);"
        sub(/Ordering::Release/, "Ordering::Relaxed")
    }
    { print }
' "$fixture/src/arch/x86_64/tlb.rs" >"$fixture/mutated"
mv "$fixture/mutated" "$fixture/src/arch/x86_64/tlb.rs"
if check >/dev/null 2>&1; then
    echo 'comment-only TLB publication was accepted' >&2
    exit 1
fi

copy_sources
awk '
    /REQUESTED_GENERATION.store\(generation, Ordering::Release\)/ {
        print "    /* REQUESTED_GENERATION.store(generation, Ordering::Release); */"
        sub(/Ordering::Release/, "Ordering::Relaxed")
    }
    { print }
' "$fixture/src/arch/x86_64/tlb.rs" >"$fixture/mutated"
mv "$fixture/mutated" "$fixture/src/arch/x86_64/tlb.rs"
if check >/dev/null 2>&1; then
    echo 'block-comment TLB publication was accepted' >&2
    exit 1
fi
