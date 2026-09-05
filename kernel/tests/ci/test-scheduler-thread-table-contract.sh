#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Prove the stable-table contract rejects representative regressions.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/hyper-thread-table.XXXXXX")
trap 'rm -rf "$fixture"' EXIT HUP INT TERM

copy_sources() {
    rm -rf "$fixture/src"
    mkdir -p "$fixture/src/kernel/task/scheduler"
    cp "$root/src/kernel/task/thread.rs" "$fixture/src/kernel/task/thread.rs"
    cp "$root/src/kernel/task/scheduler/registry.rs" "$fixture/src/kernel/task/scheduler/registry.rs"
    cp "$root/src/kernel/task/scheduler/queue.rs" "$fixture/src/kernel/task/scheduler/queue.rs"
    cp "$root/src/kernel/task/scheduler/state.rs" "$fixture/src/kernel/task/scheduler/state.rs"
    cp "$root/src/kernel/task/scheduler/mod.rs" "$fixture/src/kernel/task/scheduler/mod.rs"
}

check() {
    HYPER_THREAD_TABLE_ROOT="$fixture" \
        sh "$root/tests/ci/check-scheduler-thread-table-contract.sh"
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
mutate 'table publication became fallible-stage leakage' \
    src/kernel/task/scheduler/registry.rs 'Box::leak(table)' 'table.as_mut()'
mutate 'queue counter overflow stopped being preflighted' \
    src/kernel/task/scheduler/queue.rs 'checked_add(1)' 'wrapping_add(1)'
mutate 'ready ownership publication disappeared' \
    src/kernel/task/thread.rs 'publish_ready_ownership' 'publish_ready_state'
mutate 'crash observation resumed scanning the current stack' \
    src/kernel/task/scheduler/state.rs 'stack_statistics: None' \
    'stack_statistics: thread.kernel_stack_statistics()'
mutate 'stopped-stack scan lost its closure-bounded second lookup' \
    src/kernel/task/scheduler/mod.rs \
    'scheduler.with_thread(id, Thread::kernel_stack_statistics)' \
    'Ok(thread.kernel_stack_statistics())'
mutate 'control links re-entered the CPU-owned scheduling domain' \
    src/kernel/task/thread.rs 'control_queue_links: UnsafeCell<QueueLinks>' \
    'control_queue_links: QueueLinks'
mutate 'control queue lost its registry-only mutation authority' \
    src/kernel/task/scheduler/registry.rs 'struct ThreadControlAuthority' \
    'struct RemovedControlAuthority'
mutate 'waiting insertion bypassed the control queue API' \
    src/kernel/task/scheduler/state.rs 'queue::control_push' 'queue::push'
mutate 'crash observation changed from a try-lock to a blocking CPU lock' \
    src/kernel/task/scheduler/state.rs '.try_with(|slot|' '.with(|slot|'
