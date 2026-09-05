#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Prove the thread-retirement contract rejects representative race regressions.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/hyper-thread-retirement-test.XXXXXX")
trap 'rm -rf "$fixture"' EXIT HUP INT TERM

copy_sources() {
    rm -rf "$fixture/src" "$fixture/tests"
    mkdir -p "$fixture/src/kernel/task/scheduler" "$fixture/tests/kernel"
    cp "$root/src/kernel/task/scheduler/mod.rs" "$fixture/src/kernel/task/scheduler/mod.rs"
    cp "$root/src/kernel/task/scheduler/state.rs" "$fixture/src/kernel/task/scheduler/state.rs"
    cp "$root/src/kernel/task/scheduler/registry.rs" "$fixture/src/kernel/task/scheduler/registry.rs"
    cp "$root/src/kernel/reaper.rs" "$fixture/src/kernel/reaper.rs"
    cp "$root/tests/kernel/support.rs" "$fixture/tests/kernel/support.rs"
}

check() {
    HYPER_THREAD_RETIREMENT_ROOT="$fixture" \
        sh "$root/tests/ci/check-thread-retirement-contract.sh"
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
mutate 'retirement epoch publication was removed' \
    src/kernel/task/scheduler/state.rs \
    'ResourceRetirement::begin()' 'ResourceRetirement { _private: () }'
mutate 'resource ownership outlived retirement completion' \
    src/kernel/task/scheduler/mod.rs \
    'retire_detached_thread(thread);' 'drop(thread);'
mutate 'switch tail performed resource teardown directly' \
    src/kernel/task/scheduler/mod.rs \
    'crate::kernel::reaper::request();' 'retire_detached_thread(thread);'
mutate 'vCPU reaping treated a Retiring generation as absent' \
    src/kernel/task/scheduler/mod.rs \
    'ThreadRegistryStatus::Retiring(' 'ThreadRegistryStatus::Occupied('
mutate 'retirement completion lost release publication' \
    src/kernel/task/scheduler/mod.rs \
    'fetch_update(Ordering::Release, Ordering::Relaxed' \
    'fetch_update(Ordering::Relaxed, Ordering::Relaxed'
mutate 'retirement observation lost acquire ordering' \
    src/kernel/task/scheduler/mod.rs \
    'RETIREMENTS_IN_PROGRESS.load(Ordering::Acquire)' \
    'RETIREMENTS_IN_PROGRESS.load(Ordering::Relaxed)'
mutate 'quiescence ignored detached resources' \
    tests/kernel/support.rs \
    'statistics.retirements_in_progress != 0' \
    'statistics.retirements_in_progress == usize::MAX'
mutate 'quiescence spun without yielding physical CPU progress' \
    tests/kernel/support.rs \
    'crate::kernel::task::sleep_ms(1)?;' \
    'scheduler::yield_now()?;'
