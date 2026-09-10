#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Prove the durable vCPU-interrupt source contract rejects representative regressions.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/hyper-vcpu-interrupt-test.XXXXXX")
trap 'rm -rf "$fixture"' EXIT HUP INT TERM

copy_sources() {
    rm -rf "$fixture/src"
    mkdir -p "$fixture/src/kernel/vm/vcpu" "$fixture/src/kernel/vm/device" \
        "$fixture/src/kernel/vm/registry" "$fixture/src/kernel/task/scheduler" \
        "$fixture/src/kernel/entry"
    cp "$root/src/kernel/vm/vcpu/transition.rs" "$fixture/src/kernel/vm/vcpu/transition.rs"
    cp "$root/src/kernel/vm/device.rs" "$fixture/src/kernel/vm/device.rs"
    cp "$root/src/kernel/vm/device/aarch64.rs" "$fixture/src/kernel/vm/device/aarch64.rs"
    cp "$root/src/kernel/vm/registry.rs" "$fixture/src/kernel/vm/registry.rs"
    cp "$root/src/kernel/vm/registry/execution.rs" \
        "$fixture/src/kernel/vm/registry/execution.rs"
    cp "$root/src/kernel/vm/registry/construction.rs" \
        "$fixture/src/kernel/vm/registry/construction.rs"
    cp "$root/src/kernel/task/scheduler/state.rs" "$fixture/src/kernel/task/scheduler/state.rs"
    cp "$root/src/kernel/entry/irq.rs" "$fixture/src/kernel/entry/irq.rs"
}

check() {
    HYPER_VCPU_INTERRUPT_ROOT="$fixture" \
        sh "$root/tests/ci/check-vcpu-interrupt-publication-contract.sh"
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
mutate 'active-vCPU publication must retain both reconcile claims' \
    src/kernel/vm/vcpu/transition.rs 'take_interrupt_reconcile' 'drop_interrupt_reconcile'
mutate 'console input must not return to active-vCPU routing' \
    src/kernel/vm/device/aarch64.rs 'super::super::registry::with_binding' 'super::super::active_vcpu::with'
mutate 'guest console access must update its line inside the console lock transaction' \
    src/kernel/vm/device/aarch64.rs 'update(self.console_interrupt, outcome.interrupt_asserted)?' 'let _ = outcome.interrupt_asserted'
mutate 'virtual serial receive must update its line inside the console lock transaction' \
    src/kernel/vm/device/aarch64.rs 'update(self.console_interrupt, console.interrupt_asserted())' 'Ok(())'
mutate 'non-running vCPU states must not be guessed prompt targets' \
    src/kernel/task/scheduler/state.rs 'ThreadState::Migrating' 'ThreadState::Running'
mutate 'VM work must trigger the independent guest IRQ tail' \
    src/kernel/entry/irq.rs 'current_interrupt_reconcile_pending' 'ignore_interrupt_reconcile_pending'
