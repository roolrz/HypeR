#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Protect deferred normal-console ownership and its IRQ-safe wake boundary.
set -eu

root=${HYPER_DEFERRED_LOG_ROOT:-$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)}
cd "$root"

log=src/kernel/log/mod.rs
drain=src/kernel/log/drain.rs
console=src/kernel/log/console.rs
kconfig=Kconfig
build=build.rs
irq=src/kernel/entry/irq.rs
arch_irq=src/arch/irq.rs
riscv_irq=src/arch/riscv64/exception.rs
aarch64_irq=src/arch/aarch64/exception.rs
x86_irq=src/arch/x86_64/exception.rs
x86_vmx=src/arch/x86_64/vmx.rs
irq_init=src/kernel/irq/mod.rs
main=src/main.rs
hal_console=src/hal/console.rs
barrier_state=src/log/drain.rs
work_state=src/sync/deferred_work.rs
output_state=src/log/output.rs

require() {
    pattern=$1
    file=$2
    message=$3
    rg -q -U "$pattern" "$file" || {
        echo "$message" >&2
        exit 1
    }
}

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

producer=$(mktemp "${TMPDIR:-/tmp}/hyper-log-producer.XXXXXX")
console_tx=$(mktemp "${TMPDIR:-/tmp}/hyper-log-console-tx.XXXXXX")
prompt=$(mktemp "${TMPDIR:-/tmp}/hyper-log-prompt.XXXXXX")
rpc=$(mktemp "${TMPDIR:-/tmp}/hyper-log-rpc.XXXXXX")
riscv_private=$(mktemp "${TMPDIR:-/tmp}/hyper-log-riscv-private.XXXXXX")
aarch64_private=$(mktemp "${TMPDIR:-/tmp}/hyper-log-aarch64-private.XXXXXX")
x86_private=$(mktemp "${TMPDIR:-/tmp}/hyper-log-x86-private.XXXXXX")
x86_vmx_private=$(mktemp "${TMPDIR:-/tmp}/hyper-log-x86-vmx-private.XXXXXX")
request=$(mktemp "${TMPDIR:-/tmp}/hyper-log-request.XXXXXX")
retire=$(mktemp "${TMPDIR:-/tmp}/hyper-log-retire.XXXXXX")
barrier=$(mktemp "${TMPDIR:-/tmp}/hyper-log-barrier.XXXXXX")
registration=$(mktemp "${TMPDIR:-/tmp}/hyper-log-registration.XXXXXX")
waiter=$(mktemp "${TMPDIR:-/tmp}/hyper-log-waiter.XXXXXX")
runtime=$(mktemp "${TMPDIR:-/tmp}/hyper-log-runtime.XXXXXX")
trap 'rm -f "$producer" "$console_tx" "$prompt" "$rpc" "$riscv_private" "$aarch64_private" "$x86_private" "$x86_vmx_private" "$request" "$retire" "$barrier" "$registration" "$waiter" "$runtime"' EXIT HUP INT TERM
sed -n '/^pub fn log(/,/^}/p' "$log" >"$producer"
sed -n '/^pub(crate) fn write_guest_console_byte(/,/^}/p' "$console" >"$console_tx"
sed -n '/^pub(crate) fn dispatch(/,/^}/p' "$irq" >"$prompt"
sed -n '/^pub(crate) fn dispatch_kernel_rpc(/,/^}/p' "$irq" >"$rpc"
sed -n '/^extern "C" fn dispatch_trap(/,/^fn fatal_trap(/p' "$riscv_irq" >"$riscv_private"
sed -n '/^fn dispatch_irq(/,/^fn fatal_exception(/p' "$aarch64_irq" >"$aarch64_private"
sed -n '/^extern "C" fn x86_64_vector_dispatch(/,/^fn exception_crash_context(/p' "$x86_irq" >"$x86_private"
sed -n '/^fn handle_external_interrupt(/,/^fn prepare_guest_interrupt(/p' "$x86_vmx" >"$x86_vmx_private"
sed -n '/^pub(super) fn request(/,/^}/p' "$drain" >"$request"
sed -n '/^pub(crate) fn retire_bootstrap(/,/^}/p' "$console" >"$retire"
sed -n '/^pub(super) fn flush_sync(/,/^}/p' "$drain" >"$barrier"
sed -n '/^pub(super) fn register_flush_barrier(/,/^}/p' "$console" >"$registration"
sed -n '/^pub(super) fn wait_for_drain(/,/^impl FlushBarrier/p' "$console" >"$waiter"
sed -n '/^fn drain_runtime_batch(/,/^fn prepare_runtime_output(/p' "$drain" >"$runtime"

require 'config LOG_COMPILE_LEVEL\n[\s\S]*range 0 7[\s\S]*default 6' "$kconfig" \
    'the compiled-in log ceiling must be an explicit 0..7 configuration with default 6'
require 'config CONSOLE_LOGLEVEL_DEFAULT\n[\s\S]*range 0 7[\s\S]*default 6' "$kconfig" \
    'the runtime Console threshold must have an independent default configuration'
require 'value\("LOG_COMPILE_LEVEL"\)[\s\S]*hyper_log_compile_\{name\}[\s\S]*value <= maximum[\s\S]*cargo:rustc-cfg=\{cfg\}' "$build" \
    'the build must derive per-severity Rust cfgs from CONFIG_LOG_COMPILE_LEVEL'
require 'if !compiled_in\(level\)[\s\S]*return Ok\(\(\)\)' "$producer" \
    'the log API must reject records above the compiled-in ceiling'
require_order "$producer" 'if !compiled_in\(level\)' 'let timestamp' \
    'compile-level rejection must precede timestamping, formatting, and ring access'
require 'macro_rules! __printk_static[\s\S]*#\[cfg\(\$configuration\)\][\s\S]*#\[cfg\(not\(\$configuration\)\)\][\s\S]*if false[\s\S]*format_args!' "$log" \
    'disabled static log callsites must have no runtime path while retaining format checking'
require 'macro_rules! pr_info[\s\S]*hyper_log_compile_info, hyper::log::Level::Info' "$log" \
    'pr_info must bind to the generated info compile gate'
require 'macro_rules! pr_debug[\s\S]*hyper_log_compile_debug, hyper::log::Level::Debug' "$log" \
    'pr_debug must bind to the generated debug compile gate'
if rg -q '(^|[^[:alnum:]_])(print|println)!' src --glob '*.rs'; then
    echo 'kernel diagnostics must select an explicit pr_* severity' >&2
    exit 1
fi
require 'hyper::config::CONSOLE_LOGLEVEL_DEFAULT' "$console" \
    'Console state must initialize its mutable threshold from the configured default'
require 'pub fn set_loglevel\(level: Level\)[\s\S]*state.maximum_level = level' "$console" \
    'the runtime Console threshold must remain dynamically mutable'
require 'record.level > snapshot.maximum_level[\s\S]*publish_log_progress' "$drain" \
    'runtime Console filtering must skip output without removing producer-side ring retention'
require 'record.level <= output.maximum_level[\s\S]*write_record' "$drain" \
    'bootstrap Console draining must apply the same runtime threshold'

require 'drain::request\(\)' "$producer" \
    'runtime log production must publish deferred drain work'
if rg -q 'console::|drain_batch|write_(?:bytes|record|raw)' "$producer"; then
    echo 'runtime log production must not enter console or drain code' >&2
    exit 1
fi

require 'fn try_write_byte\(&self, byte: u8\) -> bool;' "$hal_console" \
    'the HAL console contract must expose a nonblocking byte operation'
require 'fn write_byte[\s\S]*while !self\.try_write_byte\(byte\)' "$hal_console" \
    'blocking boot and emergency output must derive from the nonblocking driver primitive'
require 'drain::enqueue_console_tx_byte\(byte\)' "$console_tx" \
    'guest Console bytes must enter the bounded TX queue'
if rg -q 'write_byte|write_bytes|CONSOLE\.with' "$console_tx"; then
    echo 'guest console producers must not write the physical UART' >&2
    exit 1
fi

require_order "$prompt" 'interrupt::dispatch\(interrupt\)' 'log::service_irq_prompt\(\)' \
    'deferred log prompt service must follow interrupt registry dispatch'
require_order "$rpc" 'irq::cross_call::service\(\)' 'log::service_irq_prompt\(\)' \
    'private IRQ service must drain its mailbox before waking deferred log waiters'
require 'struct KernelRpcServices \{[\s\S]*poll: fn\(\),[\s\S]*interrupt: fn\([^)]*\) -> EntryAction,' "$arch_irq" \
    'poll-safe and scheduler-safe kernel RPC services must remain distinct'
require 'install_kernel_rpc_services\([\s\S]*cross_call::service,[\s\S]*entry::irq::dispatch_kernel_rpc' "$irq_init" \
    'kernel RPC services must be published together before the doorbell is armed'
require_order "$riscv_private" 'clear_software_interrupt\(\)' 'irq::service_kernel_rpc_interrupt\(' \
    'RISC-V software interrupts must be cleared before scheduler-safe kernel service'
require_order "$aarch64_private" 'end_interrupt\(interrupt\)' 'irq::service_kernel_rpc_interrupt\(' \
    'AArch64 kernel RPC SGIs must complete before scheduler-safe kernel service'
require_order "$x86_private" 'end_local_interrupt\(\)' 'irq::service_kernel_rpc_interrupt\(' \
    'x86 kernel RPC vectors must complete before scheduler-safe kernel service'
require_order "$x86_vmx_private" 'end_local_interrupt\(\)' 'irq::service_kernel_rpc_interrupt\(' \
    'x86 VM-exit kernel RPC vectors must complete before scheduler-safe kernel service'
require 'if mode == RUNTIME && WORK\.request\(\)[\s\S]*prompt_local_cpu\(\)' "$request" \
    'runtime producers must coalesce a lock-free IRQ prompt'
if rg -q 'scheduler::|WAKE\.|console::' "$request"; then
    echo 'producer request publication must not enter scheduler, wait, or console code' >&2
    exit 1
fi
require 'WORK\.consume_prompt\(\)[\s\S]*WORK\.claim_notification\(\)[\s\S]*WAKE\.complete\(\)' "$drain" \
    'IRQ prompt service must coalesce before waking the worker'
require 'pub struct DeferredWork \{\n[[:space:]]*state: AtomicU8,\n\}' "$work_state" \
    'deferred work, ownership, and prompt state must share one atomic word'
require 'const WORK_PENDING: u8[\s\S]*const WAKE_OUTSTANDING: u8[\s\S]*const IRQ_PROMPTED: u8' "$work_state" \
    'deferred drain state must retain distinct work, ownership, and prompt bits'
require 'pub fn request[\s\S]*compare_exchange_weak[\s\S]*Ordering::Release' "$work_state" \
    'producer publication and prompt election must be one release CAS transition'
require 'pub fn finish_batch[\s\S]*compare_exchange_weak[\s\S]*observed & !WAKE_OUTSTANDING' "$work_state" \
    'worker ownership release must linearize on the same atomic state word'
require 'const LOG_RECORDS_PER_BATCH: usize = [1-9][0-9]*;' "$drain" \
    'kernel log drain must retain a bounded record batch'
require 'const CONSOLE_TX_FRAME_BYTES: usize = [1-9][0-9]*;' "$drain" \
    'Console TX drain must retain a bounded frame'
require 'ByteRing<CONSOLE_TX_QUEUE_CAPACITY>' "$drain" \
    'Console TX output must use an allocation-free bounded FIFO'
require 'DrainDisposition::Wait[\s\S]*WAKE\.wait\(\)' "$drain" \
    'the drain worker must block through the scheduler-aware wait primitive'
require 'BatchOutcome::Backpressured[\s\S]*WORK\.defer_until_irq\(\)[\s\S]*wait_for_worker_prompt\(\)' "$drain" \
    'UART backpressure must defer retained output to a later IRQ and block the worker'
require 'try_write_runtime_byte' "$runtime" \
    'runtime console draining must use the emergency-aware nonblocking byte operation'
if rg -q '\b(?:write_byte|write_bytes|ConsoleWriter)\b' "$runtime"; then
    echo 'runtime console draining must not enter a blocking console operation' >&2
    exit 1
fi
require 'progress\.blocked[\s\S]*BatchOutcome::Backpressured' "$runtime" \
    'runtime draining must distinguish UART rejection from byte-budget exhaustion'
require 'output\.device[\s\S]*try_write_runtime_byte\(device, byte\)' "$runtime" \
    'a partially emitted frame must retain its selected console device'
require 'output\.device = Some\(snapshot\.device\)' "$drain" \
    'frame preparation must retain the selected console device'
require 'output\.device = None' "$drain" \
    'frame commit must release the selected console device'
require 'match prepare_log_output\(output, snapshot\) \{[\s\S]*PrepareOutcome::Prepared[\s\S]*PrepareOutcome::MoreKernelRecords[\s\S]*PrepareOutcome::Idle if prepare_console_tx_frame\(output\)' "$drain" \
    'normal kernel records must be exhausted before pending Console TX frames'
require 'later eligible kernel record cannot be overtaken by userspace output[\s\S]*PrepareOutcome::MoreKernelRecords' "$drain" \
    'bounded filtering must resume kernel-log selection before Console TX'
if rg -q 'prefer_raw' "$drain"; then
    echo 'normal output must not alternate sources independently of source policy' >&2
    exit 1
fi
require 'queue\.bytes\.peek_into\(&mut bytes\)' "$drain" \
    'Console TX must prepare a bounded opaque prefix'
require 'output\.bytes\.push_bytes\(&bytes\[\.\.count\]\)' "$drain" \
    'Console TX must preserve accepted bytes exactly'
if sed -n '/^fn prepare_console_tx_frame(/,/^}/p' "$drain" | rg -q 'push_console_bytes|peek_through'; then
    echo 'Console TX must not interpret newline or terminal presentation' >&2
    exit 1
fi
require 'PendingCommit::ConsoleFrame[\s\S]*discard_front\(count\)' "$drain" \
    'Console TX queue ownership must advance only after its complete frame is accepted'
require 'write_console_tx_overflow' "$drain" \
    'the sole writer must report bounded Console TX overflow directly'
require 'flush_boot\(\)[\s\S]*state\.device = None' "$retire" \
    'bootstrap console retirement must drain before invalidating its device owner'
require_order "$barrier" 'register_flush_barrier\(\)' 'request\(\)' \
    'runtime flush must atomically register its finite watermark before requesting work'
require 'FlushBarrierRegistration::Pending\(barrier\)[\s\S]*super::console::wait_for_drain\(barrier\)' "$barrier" \
    'runtime flush must block on its exact registered barrier while the worker advances'
require 'let target_sequence = super::statistics\(\)\.next_sequence[\s\S]*\.register\(state\.next_sequence, target_sequence\)' "$registration" \
    'flush barrier registration must capture cursor and target under one console lock'
require 'CONSOLE\.with_mask_retained[\s\S]*DrainBarrierStatus::Pending[\s\S]*scheduler::begin_wait[\s\S]*prepare_registered_park_locked' "$waiter" \
    'flush waiting must close the condition-check-to-park race under the console lock'
require 'PrepareWait::Park[\s\S]*scheduler::complete_park\(scheduler::retain_park_mask' "$waiter" \
    'flush waiting must retain the console IRQ mask through its committed park'
require 'impl Drop for FlushBarrier[\s\S]*self\.release\(\)' "$console" \
    'every flush exit path must release its generation-qualified barrier slot'
require 'slot\.generation = slot\.generation\.wrapping_add\(1\)' "$barrier_state" \
    'barrier slot reuse must publish a new generation'
require 'slot\.active && slot\.generation == token\.generation' "$barrier_state" \
    'barrier observations must reject stale generation tokens'
require 'if self\.active == 0[\s\S]*return;' "$barrier_state" \
    'the ordinary no-flush log path must not scan the complete barrier table'
require 'ConsoleFlushOutcome::Overrun[\s\S]*FlushOutcome::Overrun' "$barrier" \
    'runtime flush must expose bounded-ring loss as a typed outcome'
require 'ConsoleFlushOutcome::NoConsole[\s\S]*FlushOutcome::NoConsole' "$barrier" \
    'runtime flush must not wait forever without a console'
require 'ConsoleFlushOutcome::Emergency[\s\S]*FlushOutcome::Emergency' "$barrier" \
    'runtime flush must report emergency ownership observed during registration'
require '\.advance_overrun\(state\.next_sequence, sequence, missed\)' "$console" \
    'console progress must attribute loss to the exact crossed sequence interval'
require 'PendingCommit::RingFailure => worker_failure' "$drain" \
    'a corrupt log ring must fail-stop after its retained diagnostic is accepted'
require 'const NORMAL_IDLE: u8 = 0;[\s\S]*const NORMAL_ACTIVE: u8 = 1;[\s\S]*const STOP_REQUESTED: u8 = 2;[\s\S]*const QUIESCED: u8 = 3;[\s\S]*const EMERGENCY: u8 = 4;' "$output_state" \
    'normal and emergency UART ownership must use one monotonic atomic state machine'
require 'active_cpu: AtomicUsize' "$output_state" \
    'emergency handoff must identify a locally interrupted normal writer'
require 'STOP_REQUESTED[\s\S]*active_cpu\.load\(Ordering::Acquire\) == current_cpu[\s\S]*LocalOwnerAbandoned' "$output_state" \
    'the fatal CPU must be able to abandon its own nonreturning normal-byte frame'
require 'RemoteOwnerTimedOut' "$output_state" \
    'a stalled remote normal writer must fail closed instead of sharing the UART'
require 'try_begin_normal_byte\(cpu\.get\(\)\)[\s\S]*device\.try_write_byte\(byte\)' "$console" \
    'each normal UART byte must hold one emergency-aware ownership permit'
require 'retire_normal_writer\(current_cpu, EMERGENCY_QUIESCENCE_POLLS\)' "$console" \
    'fatal ownership transfer must use a fixed quiescence bound'
require 'if !WRITE_GATE\.emergency_enabled\(\)[\s\S]*return None' "$console" \
    'direct emergency UART access must remain disabled after remote timeout'
require 'const EMERGENCY_UART_ATTEMPTS: usize = [1-9][0-9]*;' "$console" \
    'each emergency write must have a fixed nonzero attempt budget'
require 'while \*attempts != 0[\s\S]*console\.try_write_byte\(byte\)' "$console" \
    'emergency output must poll nonblocking UART access only within its budget'

require_order "$main" 'kernel::time::initialize' 'kernel::log::initialize' \
    'deferred logging must start after timer initialization'
require_order "$main" 'kernel::log::initialize' 'kernel::cpu::initialize' \
    'deferred logging must start before SMP admission'
require 'drain::enter_emergency_mode\(\);[\s\S]*match console::enter_emergency_mode\(\)[\s\S]*LocalOwnerAbandoned => emergency[\s\S]*RemoteOwnerTimedOut => emergency' "$log" \
    'fatal transition must retire deferred ownership and retain abnormal console handoff outcomes'
