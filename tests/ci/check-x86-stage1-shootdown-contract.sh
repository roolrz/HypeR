#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Keep x86 stage-1 invalidation live when its shared Kernel RPC doorbell is
# multiplexed with other poll-safe cross-CPU work.
set -eu

root=${HYPER_X86_SHOOTDOWN_ROOT:-$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)}
cd "$root"

require() {
    pattern=$1
    source=$2
    message=$3
    LC_ALL=C rg -q -U "$pattern" "$source" || {
        echo "$message" >&2
        exit 1
    }
}

require_order() {
    source=$1
    first_pattern=$2
    second_pattern=$3
    message=$4
    first_line=$(LC_ALL=C rg -n -m1 "$first_pattern" "$source" | cut -d: -f1 || true)
    second_line=$(LC_ALL=C rg -n -m1 "$second_pattern" "$source" | cut -d: -f1 || true)
    if [ -z "$first_line" ] || [ -z "$second_line" ] || [ "$first_line" -ge "$second_line" ]; then
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
    if [ -z "$body" ] || ! printf '%s\n' "$body" | LC_ALL=C rg -q -U "$pattern"; then
        echo "$message" >&2
        exit 1
    fi
}

line_in_function() {
    checked_source=$1
    declaration=$2
    pattern=$3
    function_body "$checked_source" "$declaration" |
        LC_ALL=C rg -n "$pattern" | sed -n '1s/:.*//p'
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
        LC_ALL=C rg -o "$pattern" 2>/dev/null | wc -l | tr -d ' ')
    if [ "$count" -ne "$expected" ]; then
        echo "$message (expected $expected, found $count)" >&2
        exit 1
    fi
}

reject_commented_contract() {
    checked_source=$1
    declaration=$2
    raw_body=$(sed -n "/^$declaration/,/^}/p" "$checked_source")
    if printf '%s\n' "$raw_body" | LC_ALL=C rg -q -U \
        '(?s)/\*.*?(Ordering::(Acquire|Release)|REQUESTED_GENERATION|ACKNOWLEDGED_GENERATION|flush_local|publish_kernel_rpc|send_fixed_ipi|service_kernel_rpc|"(mfence|lfence|wrmsr)").*?\*/'; then
        echo "commented-out code must not satisfy $declaration" >&2
        exit 1
    fi
}

tlb=src/arch/x86_64/tlb.rs
rpc=src/kernel/irq/cross_call.rs
controller=src/arch/x86_64/interrupt_controller.rs
publication=src/sync/publication.rs

require 'if previous == u64::MAX \{[^}]*super::halt\(\)' "$tlb" \
    'TLB generations must fail closed before wrapping to zero'
require_order "$tlb" 'REQUESTED_GENERATION\.store' 'flush_local\(\);' \
    'the TLB request must be published before the initiating CPU flushes'
require_order "$tlb" 'publish_kernel_rpc\(' 'send_fixed_ipi\(' \
    'the target reason must be published before the Kernel RPC doorbell'
flush_function='pub(super) fn flush_all_online()'
admission_function='pub(super) fn synchronize_online_cpu()'
service_function='pub(super) fn service_pending()'

require_function "$tlb" "$flush_function" \
    '^[[:space:]]*REQUESTED_GENERATION\.store\(generation, Ordering::Release\);' \
    'TLB requests must release-publish page-table writes'
require_function "$tlb" "$flush_function" \
    '^[[:space:]]*while ACKNOWLEDGED_GENERATION\[cpu\]\.load\(Ordering::Acquire\) != generation' \
    'TLB waits must acquire remote acknowledgements'
require_function "$tlb" "$flush_function" \
    'publish_kernel_rpc\([[:space:]]*cpu,[[:space:]]*KernelRpcReasons::STAGE1_TLB_SHOOTDOWN\.bits\(\)' \
    'TLB shootdown must publish the stage-1 reason on the shared doorbell'
require_function "$tlb" "$flush_function" \
    'while ACKNOWLEDGED_GENERATION\[cpu\]\.load\(Ordering::Acquire\) != generation \{[[:space:]]*crate::arch::irq::service_kernel_rpc\(\);' \
    'TLB waits must drain all shared RPC work'

require_function_order "$tlb" "$flush_function" \
    '^[[:space:]]*REQUESTED_GENERATION\.store' '^[[:space:]]*flush_local\(\);' \
    'TLB request publication must precede initiating-CPU invalidation'
require_function_order "$tlb" "$flush_function" \
    '^[[:space:]]*flush_local\(\);' 'publish_kernel_rpc\(' \
    'initiating-CPU invalidation must precede remote reason publication'
require_function_order "$tlb" "$flush_function" \
    'publish_kernel_rpc\(' 'send_fixed_ipi\(' \
    'remote reason publication must precede its shared doorbell'
require_function_order "$tlb" "$flush_function" \
    'send_fixed_ipi\(' '^[[:space:]]*while ACKNOWLEDGED_GENERATION' \
    'remote delivery must precede acknowledgement waits'
require_function_occurrences "$tlb" "$flush_function" \
    'REQUESTED_GENERATION\.store\(' 1 'the initiator must publish exactly one generation'
require_function_occurrences "$tlb" "$flush_function" \
    'flush_local\(\);' 1 'the initiator must invalidate locally exactly once'
require_function_occurrences "$tlb" "$flush_function" \
    'publish_kernel_rpc\(' 1 'the initiator must retain one reason-publication seam'
require_function_occurrences "$tlb" "$flush_function" \
    'send_fixed_ipi\(' 1 'the initiator must retain one remote-delivery seam'
require_function_occurrences "$tlb" "$flush_function" \
    'ACKNOWLEDGED_GENERATION\[cpu\]\.load\(' 1 \
    'the initiator must retain one remote-completion seam'

require_function "$tlb" "$admission_function" \
    '^[[:space:]]*let generation = REQUESTED_GENERATION\.load\(Ordering::Acquire\);' \
    'CPU admission must acquire the latest published generation'
require_function "$tlb" "$admission_function" \
    '^[[:space:]]*ACKNOWLEDGED_GENERATION\[cpu\.get\(\)\]\.store\(generation, Ordering::Release\);' \
    'CPU admission must release-publish its local invalidation'
require_function_order "$tlb" "$admission_function" \
    'REQUESTED_GENERATION\.load' 'flush_local\(\);' \
    'CPU admission must acquire the request before invalidating'
require_function_order "$tlb" "$admission_function" \
    'flush_local\(\);' 'ACKNOWLEDGED_GENERATION.*\.store' \
    'CPU admission must acknowledge only after invalidating'
require_function_occurrences "$tlb" "$admission_function" \
    'REQUESTED_GENERATION\.load\(' 1 'CPU admission must observe one generation'
require_function_occurrences "$tlb" "$admission_function" \
    'flush_local\(\);' 1 'CPU admission must perform one local invalidation'
require_function_occurrences "$tlb" "$admission_function" \
    'ACKNOWLEDGED_GENERATION.*\.store\(' 1 'CPU admission must publish one acknowledgement'

require_function "$tlb" "$service_function" \
    '^[[:space:]]*let generation = REQUESTED_GENERATION\.load\(Ordering::Acquire\);' \
    'masked RPC progress must acquire the published generation'
require_function "$tlb" "$service_function" \
    'ACKNOWLEDGED_GENERATION\[cpu\.get\(\)\]\.load\(Ordering::Relaxed\) == generation \{[[:space:]]*return;' \
    'masked RPC progress must short-circuit an already-serviced generation'
require_function "$tlb" "$service_function" \
    '^[[:space:]]*ACKNOWLEDGED_GENERATION\[cpu\.get\(\)\]\.store\(generation, Ordering::Release\);' \
    'masked RPC progress must release-publish local invalidation'
require_function_order "$tlb" "$service_function" \
    'REQUESTED_GENERATION\.load' 'ACKNOWLEDGED_GENERATION.*\.load' \
    'masked RPC progress must acquire the request before duplicate detection'
require_function_order "$tlb" "$service_function" \
    'ACKNOWLEDGED_GENERATION.*\.load' 'flush_local\(\);' \
    'masked RPC progress must check for duplicate work before invalidating'
require_function_order "$tlb" "$service_function" \
    'flush_local\(\);' 'ACKNOWLEDGED_GENERATION.*\.store' \
    'masked RPC progress must acknowledge only after invalidating'
require_function_occurrences "$tlb" "$service_function" \
    'REQUESTED_GENERATION\.load\(' 1 'masked RPC progress must observe one generation'
require_function_occurrences "$tlb" "$service_function" \
    'flush_local\(\);' 1 'masked RPC progress must perform one local invalidation'
require_function_occurrences "$tlb" "$service_function" \
    'ACKNOWLEDGED_GENERATION.*\.store\(' 1 'masked RPC progress must publish one acknowledgement'

reject_commented_contract "$tlb" "$flush_function"
reject_commented_contract "$tlb" "$admission_function"
reject_commented_contract "$tlb" "$service_function"

require 'pub\(crate\) fn service\(\) \{[^}]*loop \{[^}]*take_kernel_rpc_reasons\(\)' "$rpc" \
    'Kernel RPC service must drain the durable reason mailbox'
require_order "$rpc" 'contains\(KernelRpcReasons::STAGE1_TLB_SHOOTDOWN\)' \
    'contains\(KernelRpcReasons::LOCAL_IRQ_LIFECYCLE\)' \
    'Kernel RPC dispatch must service TLB work before IRQ lifecycle work'
require 'spin_wait_until\(TIMEOUT_NS, \|\| \{[[:space:]]*service\(\);' "$rpc" \
    'Kernel RPC acknowledgement waits must make shared-doorbell progress'
require 'ACK\[cpu\]\.load\(Ordering::Acquire\) == generation' "$rpc" \
    'Kernel RPC waits must acquire generation-tagged acknowledgements'
require 'ACK\[cpu\]\.store\(generation, Ordering::Release\)' "$rpc" \
    'Kernel RPC handlers must release-publish completion'
require 'if !targeted \{[[:space:]]*continue;' "$rpc" \
    'Kernel RPC completion must ignore CPUs outside the target set'
require 'checked_add\(1\) \{[[:space:]]*Some\(generation\) if generation != 0' "$rpc" \
    'Kernel RPC generations must reject exhaustion and zero reuse'
require 'poison\("kernel RPC route rejected"\)' "$rpc" \
    'ambiguous Kernel RPC route failure must poison the transport'
require 'poison\("kernel RPC acknowledgement timed out"\)' "$rpc" \
    'ambiguous Kernel RPC timeout must poison the transport'

for source in src/arch/aarch64/smp.rs src/arch/riscv64/smp.rs src/arch/x86_64/smp.rs; do
    require 'fetch_or\(reasons, Ordering::Release\)' "$source" \
        "$source must release-publish Kernel RPC reasons"
    require 'swap\(0, Ordering::Acquire\)' "$source" \
        "$source must acquire and atomically drain Kernel RPC reasons"
done

require 'if vector == super::platform::KERNEL_RPC_VECTOR \{[[:space:]]*crate::arch::irq::service_kernel_rpc\(\);[[:space:]]*super::interrupt_controller::end_local_interrupt\(\);[[:space:]]*return;' \
    src/arch/x86_64/exception.rs 'IDT must consume and EOI Kernel RPC exactly once'
require 'if vector == super::platform::KERNEL_RPC_VECTOR \{[[:space:]]*crate::arch::irq::service_kernel_rpc\(\);[[:space:]]*super::interrupt_controller::end_local_interrupt\(\);[[:space:]]*return;' \
    src/arch/x86_64/vmx.rs 'VMX must consume and EOI Kernel RPC exactly once'
require 'fn wait_for_lock_owner\(\) \{[^}]*crate::arch::irq::service_kernel_rpc\(\);' \
    src/arch/x86_64/interrupts.rs 'masked lock waits must drain Kernel RPC'
require 'static KERNEL_RPC_SERVICE: PublishedOnce<fn\(\)> = PublishedOnce::new\(\);' \
    src/arch/irq.rs 'Kernel RPC service installation must use one-shot publication'
require 'KERNEL_RPC_SERVICE[[:space:]]*\.publish\(callback\)' src/arch/irq.rs \
    'Kernel RPC callback must be published through the one-shot cell'
require 'KERNEL_RPC_SERVICE\.get\(\)\.copied\(\)' src/arch/irq.rs \
    'Kernel RPC callback entry must acquire the published callback'
require 'compare_exchange\(EMPTY, INSTALLING, Ordering::Relaxed, Ordering::Relaxed\)' \
    "$publication" 'one-shot publication must claim exactly one initializer'
require 'state\.store\(READY, Ordering::Release\)' "$publication" \
    'one-shot publication must release-publish initialized values'
require 'state\.load\(Ordering::Acquire\) != READY' "$publication" \
    'one-shot readers must acquire initialization'
require 'unsafe impl<T: Send \+ Sync> Sync for PublishedOnce<T>' "$publication" \
    'shared publication must require both ownership transfer and shared access'
require 'static BOOT_STATE: PublishedOnce<BootState> = PublishedOnce::new\(\);' \
    src/kernel/boot/state.rs 'BootState must use the shared one-shot publication cell'
require 'BOOT_STATE[[:space:]]*\.publish\(state\)' src/kernel/boot/state.rs \
    'BootState installation must publish through the one-shot cell'
require 'BOOT_STATE\.get\(\)' src/kernel/boot/state.rs \
    'BootState access must acquire the published state'
require_order src/kernel/irq/mod.rs 'install_kernel_rpc_service\(cross_call::service\)' \
    'initialize_local_rpc_transport\(\)' \
    'the opaque Kernel RPC dispatcher must be installed before its doorbell is armed'
require 'KERNEL_RPC_VECTOR != TIMER_VECTOR' src/arch/x86_64/platform.rs \
    'Kernel RPC and timer vectors must remain distinct'
require 'KERNEL_RPC_VECTOR != RESCHEDULE_VECTOR' src/arch/x86_64/platform.rs \
    'Kernel RPC and reschedule vectors must remain distinct'
require '"mfence",[[:space:]]*"lfence",[[:space:]]*"wrmsr"' "$controller" \
    'x2APIC Kernel RPC publication must retain MFENCE;LFENCE;WRMSR ordering'
if sed -n '/^pub fn send_fixed_ipi(/,/^}/p' "$controller" | LC_ALL=C rg -q 'options\([^)]*nomem'; then
    echo 'send_fixed_ipi must remain a compiler memory boundary' >&2
    exit 1
fi
reject_commented_contract "$controller" 'pub fn send_fixed_ipi'

require 'slot\.store\(true, Ordering::Release\);[[:space:]]*super::tlb::synchronize_online_cpu\(\);' \
    src/arch/x86_64/smp.rs 'CPU admission must close the online/shootdown snapshot race'
require_function src/arch/x86_64/smp.rs 'pub(super) fn for_each_online_remote_cpu' \
    'ONLINE\[index\]\.load\(Ordering::Acquire\)' \
    'shootdown snapshots must acquire CPU-online publication'
require 'with_relax\(operation, M::wait_for_lock_owner\)' src/sync/lock/interrupt.rs \
    'IRQ-safe locks must invoke the masked-contention progress hook'
relaxed_paths=$(LC_ALL=C rg -c 'with_relax\(operation, M::wait_for_lock_owner\)' \
    src/sync/lock/interrupt.rs)
if [ "$relaxed_paths" -ne 2 ]; then
    echo "every blocking IRQ-safe acquisition path must make masked progress (found $relaxed_paths, expected 2)" >&2
    exit 1
fi
require 'static STACK_SLOTS: StackLock<StackSlots>' src/kernel/mm/stack.rs \
    'STACK_SLOTS must serialize slot ownership and stage-1 mutation'

flushes=$(LC_ALL=C rg -c 'super::tlb::flush_all_online\(\);' src/arch/x86_64/memory.rs)
if [ "$flushes" -ne 3 ]; then
    echo "every x86 live stage-1 update must shoot down all online CPUs (found $flushes, expected 3)" >&2
    exit 1
fi
if LC_ALL=C rg -q 'InterruptSpinLock' src/kernel/boot/state.rs; then
    echo 'immutable BootState access must remain non-blocking after publication' >&2
    exit 1
fi
