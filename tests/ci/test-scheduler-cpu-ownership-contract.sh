#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Prove the CPU-ownership contract rejects representative regressions.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/hyper-scheduler-cpu-ownership.XXXXXX")
trap 'rm -rf "$fixture"' EXIT HUP INT TERM

copy_sources() {
    rm -rf "$fixture/src"
    mkdir -p "$fixture/src/kernel/task/scheduler"
    cp "$root/src/kernel/task/thread.rs" "$fixture/src/kernel/task/thread.rs"
    cp "$root/src/kernel/task/scheduler/state.rs" "$fixture/src/kernel/task/scheduler/state.rs"
    cp "$root/src/kernel/task/scheduler/registry.rs" "$fixture/src/kernel/task/scheduler/registry.rs"
    cp "$root/src/kernel/task/scheduler/queue.rs" "$fixture/src/kernel/task/scheduler/queue.rs"
    cp "$root/src/kernel/task/scheduler/mod.rs" "$fixture/src/kernel/task/scheduler/mod.rs"
}

check() {
    HYPER_SCHEDULER_CPU_OWNERSHIP_ROOT="$fixture" \
        sh "$root/tests/ci/check-scheduler-cpu-ownership-contract.sh"
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
mutate 'schedule storage stopped being address stable' \
    src/kernel/task/thread.rs \
    'UnsafeCell<ThreadScheduleState>' 'ThreadScheduleState'
mutate 'tick stopped reading the locked run-queue topology' \
    src/kernel/task/scheduler/state.rs \
    'local.run_queue.has_fair_threads()' 'false'
mutate 'placement admission reacquired the target CPU scheduler lock' \
    src/kernel/task/scheduler/state.rs \
    'self.schedulable_cpus.contains(cpu)' 'CPU_SCHEDULERS[cpu].with(|slot| slot.is_some())'
mutate 'local ready authority gained coordinator capability' \
    src/kernel/task/scheduler/queue.rs \
    'cpu: CpuThreadTableAuthority' 'coordinator: ThreadTableWriteAuthority'
mutate 'ordinary yield stopped using the local CPU path' \
    src/kernel/task/scheduler/state.rs \
    'pub(super) fn prepare_local_yield' 'pub(super) fn removed_local_yield'
mutate 'switch tail bypassed local completion' \
    src/kernel/task/scheduler/mod.rs \
    'state::complete_local_switch_tail(cpu, ticket)' 'Ok(state::LocalTailCompletion::NeedsCoordinator)'
mutate 'architecture switch stopped carrying the generation ticket' \
    src/kernel/task/scheduler/state.rs \
    'self.ticket as usize' '0'
mutate 'registry removal accepted a CPU-owned token' \
    src/kernel/task/scheduler/registry.rs \
    '!thread.schedule_is_coordinator_owned()' 'false'
mutate 'bootstrap retirement completion lost its reserved-slot rule' \
    src/kernel/task/scheduler/registry.rs \
    'pub fn complete_retirement' 'pub fn removed_complete_retirement'
mutate 'matching active CPU re-entered the lock wrapper' \
    src/kernel/task/scheduler/state.rs \
    'if cpu == active.cpu => Ok(None)' 'if cpu == active.cpu => Ok(Some(cpu))'
mutate 'top-level CPU access stopped sharing raw provenance with nested access' \
    src/kernel/task/scheduler/state.rs \
    'operation(self, unsafe { &mut \*local.as_ptr() })' \
    'operation(self, local.as_mut())'
mutate 'user stop lost its pre-resolution schedule snapshot' \
    src/kernel/task/scheduler/state.rs \
    'let (state, cpu, ticket) =' 'let (state_after_resolution, cpu, ticket) ='
