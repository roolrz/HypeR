#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Prove the resource-alias contract rejects representative regressions.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/hyper-thread-resource-alias.XXXXXX")
trap 'rm -rf "$fixture"' EXIT HUP INT TERM

copy_sources() {
    rm -rf "$fixture/src"
    mkdir -p "$fixture/src/kernel/task/scheduler" "$fixture/src/kernel/entry"
    cp "$root/src/kernel/task/thread.rs" "$fixture/src/kernel/task/thread.rs"
    cp "$root/src/kernel/task/scheduler/state.rs" "$fixture/src/kernel/task/scheduler/state.rs"
    cp "$root/src/kernel/task/scheduler/mod.rs" "$fixture/src/kernel/task/scheduler/mod.rs"
    cp "$root/src/kernel/entry/user.rs" "$fixture/src/kernel/entry/user.rs"
}

check() {
    HYPER_THREAD_RESOURCE_ALIAS_ROOT="$fixture" \
        sh "$root/tests/ci/check-thread-resource-alias-contract.sh"
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
mutate 'resources returned to the outer Thread allocation' \
    src/kernel/task/thread.rs 'resources: Box<ThreadResources>' 'resources: ThreadResources'
mutate 'ThreadContext lost interior raw-pointer ownership' \
    src/kernel/task/thread.rs \
    'context: UnsafeCell<crate::hal::context::ThreadContext>' \
    'context: crate::hal::context::ThreadContext'
mutate 'vCPU payload lost interior raw-pointer ownership' \
    src/kernel/task/thread.rs \
    'Vcpu(Box<UnsafeCell<VcpuExecution>>)' 'Vcpu(Box<VcpuExecution>)'
mutate 'user payload lost interior raw-pointer ownership' \
    src/kernel/task/thread.rs \
    'User(Box<UnsafeCell<crate::kernel::process::UserExecution>>)' \
    'User(Box<crate::kernel::process::UserExecution>)'
mutate 'vCPU lookup again relied on a mutable payload borrow' \
    src/kernel/task/scheduler/state.rs \
    'vcpu_execution_pointer()' 'vcpu_execution_mut()'
mutate 'native-user owner pointer became an exclusive reference' \
    src/kernel/entry/user.rs \
    'execution.as_ref()' 'execution.as_mut()'
mutate 'resource accounting omitted the private allocation' \
    src/kernel/task/thread.rs \
    ' + core::mem::size_of::<ThreadResources>()' ''
