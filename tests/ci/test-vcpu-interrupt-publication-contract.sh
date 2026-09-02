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
    mkdir -p "$fixture/src/kernel/vm/vcpu" "$fixture/src/kernel/vm/device" "$fixture/src/kernel/task/scheduler" \
        "$fixture/src/kernel/entry" "$fixture/src/kernel/vm/linux"
    cp "$root/src/kernel/vm/vcpu/transition.rs" "$fixture/src/kernel/vm/vcpu/transition.rs"
    cp "$root/src/kernel/vm/device.rs" "$fixture/src/kernel/vm/device.rs"
    cp "$root/src/kernel/vm/device/aarch64.rs" "$fixture/src/kernel/vm/device/aarch64.rs"
    cp "$root/src/kernel/vm/registry.rs" "$fixture/src/kernel/vm/registry.rs"
    cp "$root/src/kernel/task/scheduler/state.rs" "$fixture/src/kernel/task/scheduler/state.rs"
    cp "$root/src/kernel/entry/irq.rs" "$fixture/src/kernel/entry/irq.rs"
    cp "$root/src/kernel/vm/linux/mod.rs" "$fixture/src/kernel/vm/linux/mod.rs"
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
mutate 'console routes must remain replaceable for future teardown' \
    src/kernel/vm/device/aarch64.rs 'InterruptSpinLock<Option<ConsoleRoute>' 'PublishedOnce<ConsoleRoute'
mutate 'guest console access must update its line inside the console lock transaction' \
    src/kernel/vm/device/aarch64.rs 'update(self.console_interrupt, outcome.interrupt_asserted)?' 'let _ = outcome.interrupt_asserted'
mutate 'host console receive must update its line inside the console lock transaction' \
    src/kernel/vm/device/aarch64.rs 'update(self.console_interrupt, asserted)' 'Ok(())'
mutate 'console teardown must not clear a different Thread route' \
    src/kernel/vm/device/aarch64.rs 'current.thread == expected_thread' 'true'
mutate 'closed endpoint console delivery must not mask the host UART' \
    src/kernel/vm/device/aarch64.rs 'Error::EndpointClosed' 'Error::EndpointStillOpen'
mutate 'generic registry installation must not select host-console policy' \
    src/kernel/vm/registry.rs 'control: VmControl::mint_for_install(id),' 'control: { super::device::try_publish_console_route(id, 0, boot_vcpu); VmControl::mint_for_install(id) },'
mutate 'non-running vCPU states must not be guessed prompt targets' \
    src/kernel/task/scheduler/state.rs 'ThreadState::Migrating' 'ThreadState::Running'
mutate 'VM work must trigger the independent guest IRQ tail' \
    src/kernel/entry/irq.rs 'current_interrupt_reconcile_pending' 'ignore_interrupt_reconcile_pending'
