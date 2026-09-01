#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Prove the deferred log contract rejects representative ownership regressions.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/hyper-deferred-log-test.XXXXXX")
trap 'rm -rf "$fixture"' EXIT HUP INT TERM

copy_sources() {
    rm -rf "$fixture/src"
    mkdir -p "$fixture/src/kernel/log" "$fixture/src/kernel/entry" "$fixture/src/kernel/irq" \
        "$fixture/src/arch/aarch64" "$fixture/src/arch/riscv64" "$fixture/src/arch/x86_64" \
        "$fixture/src/hal" "$fixture/src/log" "$fixture/src/arch" "$fixture/src/sync"
    cp "$root/src/kernel/log/mod.rs" "$fixture/src/kernel/log/mod.rs"
    cp "$root/src/kernel/log/drain.rs" "$fixture/src/kernel/log/drain.rs"
    cp "$root/src/kernel/log/console.rs" "$fixture/src/kernel/log/console.rs"
    cp "$root/src/kernel/entry/irq.rs" "$fixture/src/kernel/entry/irq.rs"
    cp "$root/src/kernel/irq/mod.rs" "$fixture/src/kernel/irq/mod.rs"
    cp "$root/src/arch/irq.rs" "$fixture/src/arch/irq.rs"
    cp "$root/src/arch/aarch64/exception.rs" "$fixture/src/arch/aarch64/exception.rs"
    cp "$root/src/arch/riscv64/exception.rs" "$fixture/src/arch/riscv64/exception.rs"
    cp "$root/src/arch/x86_64/exception.rs" "$fixture/src/arch/x86_64/exception.rs"
    cp "$root/src/arch/x86_64/vmx.rs" "$fixture/src/arch/x86_64/vmx.rs"
    cp "$root/src/hal/console.rs" "$fixture/src/hal/console.rs"
    cp "$root/src/log/drain.rs" "$fixture/src/log/drain.rs"
    cp "$root/src/log/output.rs" "$fixture/src/log/output.rs"
    cp "$root/src/sync/deferred_work.rs" "$fixture/src/sync/deferred_work.rs"
    cp "$root/src/main.rs" "$fixture/src/main.rs"
}

check() {
    HYPER_DEFERRED_LOG_ROOT="$fixture" \
        sh "$root/tests/ci/check-deferred-log-contract.sh"
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
mutate 'log producers must not synchronously flush the console' \
    src/kernel/log/mod.rs 'drain::request();' 'console::flush();'
mutate 'IRQ prompt service must follow registry dispatch' \
    src/kernel/entry/irq.rs 'crate::kernel::irq::interrupt::dispatch(interrupt);' \
    'crate::kernel::log::service_irq_prompt();'
mutate 'guest bytes must not bypass the deferred FIFO' \
    src/kernel/log/console.rs 'super::drain::enqueue_raw(byte);' \
    'CONSOLE.with(|_| device.write_byte(byte));'
mutate 'normal drain batches must remain bounded' \
    src/kernel/log/drain.rs 'const LOG_RECORDS_PER_BATCH: usize = 32;' \
    'const LOG_RECORDS_PER_BATCH_REMOVED: usize = 32;'
mutate 'emergency ownership must retire deferred draining first' \
    src/kernel/log/mod.rs 'drain::enter_emergency_mode();' \
    'drain::request();'
mutate 'runtime producer requests must not enter scheduler code' \
    src/kernel/log/drain.rs 'prompt_local_cpu();' \
    'scheduler::yield_now().ok();'
mutate 'IRQ service must consume the coalesced producer prompt' \
    src/kernel/log/drain.rs '!WORK.consume_prompt()' \
    'false'
mutate 'deferred wake state must retain one atomic modification order' \
    src/sync/deferred_work.rs 'state: AtomicU8,' \
    'pending: AtomicU8,'
mutate 'worker release must remain a same-word atomic transition' \
    src/sync/deferred_work.rs 'observed & !WAKE_OUTSTANDING,' \
    'observed,'
mutate 'bootstrap retirement must flush before dropping its device' \
    src/kernel/log/console.rs 'super::drain::flush_boot();' \
    'super::drain::request();'
mutate 'runtime flush must atomically register its finite watermark' \
    src/kernel/log/drain.rs 'super::console::register_flush_barrier()' \
    'return Ok(FlushOutcome::Drained)'
mutate 'runtime flush must report an absent console' \
    src/kernel/log/drain.rs 'super::console::ConsoleFlushOutcome::NoConsole => Ok(FlushOutcome::NoConsole),' \
    'super::console::ConsoleFlushOutcome::NoConsole => Ok(FlushOutcome::Drained),'
mutate 'runtime flush must sleep on the exact progress waiter' \
    src/kernel/log/drain.rs 'super::console::wait_for_drain(barrier)' \
    'scheduler::yield_now().map(|_| super::console::ConsoleFlushOutcome::Drained)'
mutate 'flush barrier target capture must remain under the console lock' \
    src/kernel/log/console.rs 'let target_sequence = super::statistics().next_sequence;' \
    'let target_sequence = u64::MAX;'
mutate 'overrun attribution must retain its exact sequence interval' \
    src/kernel/log/console.rs '.advance_overrun(state.next_sequence, sequence, missed)' \
    '.advance(sequence)'
mutate 'flush wait must retain the IRQ mask through its committed park' \
    src/kernel/log/console.rs 'scheduler::complete_park(scheduler::retain_park_mask(commit, interrupt_mask))' \
    'scheduler::complete_park_without_mask(commit)'
mutate 'flush barrier slots must be released on every exit path' \
    src/kernel/log/console.rs 'self.release();' \
    'return;'
mutate 'barrier slot reuse must invalidate stale tokens' \
    src/log/drain.rs 'slot.generation = slot.generation.wrapping_add(1);' \
    'slot.generation = slot.generation;'
mutate 'ring corruption must not leave finite flush waiters pending forever' \
    src/kernel/log/drain.rs 'PendingCommit::RingFailure => worker_failure(' \
    'PendingCommit::RingFailure => preserve_failure('
mutate 'runtime output must not use a blocking console operation' \
    src/kernel/log/drain.rs 'super::console::try_write_runtime_byte(device, byte)' \
    '{ device.write_byte(byte); super::console::RuntimeByteWrite::Accepted }'
mutate 'runtime output must distinguish rejection from budget exhaustion' \
    src/kernel/log/drain.rs 'progress.blocked' \
    'progress.accepted < budget'
mutate 'UART rejection must defer work to a later IRQ' \
    src/kernel/log/drain.rs 'WORK.defer_until_irq();' \
    'let _ = WORK.request();'
mutate 'partial frames must retain their selected device' \
    src/kernel/log/drain.rs 'output.device = Some(snapshot.device);' \
    'output.device = None;'
mutate 'guest raw output must retain console newline translation' \
    src/kernel/log/drain.rs 'push_console_bytes(core::slice::from_ref(&byte))' \
    'push_byte(byte)'
mutate 'raw queue ownership must not advance before physical acceptance' \
    src/kernel/log/drain.rs 'queue.bytes.pop_front()' \
    'Some(expected)'
mutate 'RISC-V software prompts must reach the scheduler-safe log seam' \
    src/arch/riscv64/exception.rs 'crate::arch::irq::service_kernel_rpc_interrupt' \
    'crate::arch::irq::discard_kernel_rpc_interrupt'
mutate 'private IRQ service must drain RPC work before waking log waiters' \
    src/kernel/entry/irq.rs 'crate::kernel::irq::cross_call::service();' \
    'crate::kernel::log::service_irq_prompt();'
mutate 'AArch64 private IRQ service must follow controller completion' \
    src/arch/aarch64/exception.rs 'super::end_interrupt(interrupt);' \
    'crate::arch::irq::discard_kernel_rpc_interrupt(interrupt_origin);'
mutate 'the HAL must retain a nonblocking console primitive' \
    src/hal/console.rs 'fn try_write_byte(&self, byte: u8) -> bool;' \
    'fn try_write_byte_removed(byte: u8) -> bool;'
mutate 'normal UART bytes must hold emergency-aware ownership' \
    src/kernel/log/console.rs 'WRITE_GATE.try_begin_normal_byte(cpu.get())' \
    'RuntimeByteAccess::Acquired(fake_permit())'
mutate 'emergency UART handoff must remain bounded' \
    src/kernel/log/console.rs 'WRITE_GATE.retire_normal_writer(current_cpu, EMERGENCY_QUIESCENCE_POLLS)' \
    'WRITE_GATE.retire_normal_writer(current_cpu, usize::MAX)'
mutate 'emergency UART polling must remain bounded' \
    src/kernel/log/console.rs 'while \*attempts != 0 {' \
    'loop {'
mutate 'remote handoff timeout must keep direct UART access disabled' \
    src/kernel/log/console.rs 'if !WRITE_GATE.emergency_enabled() {' \
    'if false {'
