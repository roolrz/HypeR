#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Prove the VM retirement source contract rejects representative regressions.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/hyper-vm-retirement-test.XXXXXX")
trap 'rm -rf "$fixture"' EXIT HUP INT TERM

copy_sources() {
    rm -rf "$fixture/src"
    mkdir -p "$fixture/src/kernel/vm/vcpu" "$fixture/src/kernel/vm/device" "$fixture/src/kernel/vm/linux" \
        "$fixture/src/kernel/entry" "$fixture/src/kernel/irq" \
        "$fixture/src/hal/selected" "$fixture/src/hal" \
        "$fixture/src/arch/aarch64"
    cp "$root/src/kernel/vm/registry.rs" "$fixture/src/kernel/vm/registry.rs"
    cp "$root/src/kernel/vm/lifecycle.rs" "$fixture/src/kernel/vm/lifecycle.rs"
    cp "$root/src/kernel/vm/device.rs" "$fixture/src/kernel/vm/device.rs"
    cp "$root/src/kernel/vm/device/aarch64.rs" "$fixture/src/kernel/vm/device/aarch64.rs"
    cp "$root/src/kernel/vm/vcpu/runner.rs" "$fixture/src/kernel/vm/vcpu/runner.rs"
    cp "$root/src/kernel/vm/linux/mod.rs" "$fixture/src/kernel/vm/linux/mod.rs"
    cp "$root/src/kernel/entry/irq.rs" "$fixture/src/kernel/entry/irq.rs"
    cp "$root/src/kernel/vm/memory.rs" "$fixture/src/kernel/vm/memory.rs"
    cp "$root/src/kernel/irq/cross_call.rs" "$fixture/src/kernel/irq/cross_call.rs"
    cp "$root/src/hal/selected/vm.rs" "$fixture/src/hal/selected/vm.rs"
    cp "$root/src/hal/interrupt.rs" "$fixture/src/hal/interrupt.rs"
    cp "$root/src/arch/aarch64/stage2.rs" "$fixture/src/arch/aarch64/stage2.rs"
}

check() {
    HYPER_VM_RETIREMENT_ROOT="$fixture" \
        sh "$root/tests/ci/check-vm-retirement-contract.sh"
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
mutate 'registry cut must precede console cut' src/kernel/vm/registry.rs \
    'super::device::clear_console_route_for_vm(id);' 'let _ = id;'
mutate 'VM retirement authority must remain linear' src/kernel/vm/registry.rs \
    'pub(super) struct VmControl' '#[derive(Clone)] pub(super) struct VmControl'
mutate 'VM control construction must remain registry-private' src/kernel/vm/registry.rs \
    'const fn mint_for_install' 'pub(super) const fn mint_for_install'
mutate 'raw registry cut must not bypass linear authority' src/kernel/vm/registry.rs \
    'fn begin_quiesce_control' 'pub(super) fn begin_quiesce_control'
mutate 'quiescence must use unique-owner conversion' src/kernel/vm/registry.rs \
    'machine.try_into_unique()' 'Ok(machine)'
mutate 'console route needs its second Installed validation' src/kernel/vm/device/aarch64.rs \
    'if super::super::super::registry::is_installed(vm) {' 'if true {'
mutate 'guest termination must arm persistent reaping' src/kernel/vm/vcpu/runner.rs \
    'VcpuClosureReason::Guest(reason)' 'VcpuClosureReason::Administrative(reason)'
mutate 'runner must not accept admission close without durable stop' \
    src/kernel/vm/vcpu/runner.rs 'administrative_stop_reason(execution, current.thread)' \
    'Some(crate::kernel::vm::registry::AdministrativeStopReason::Requested)'
mutate 'reap-before-prompt must be recognized from endpoint state' \
    src/kernel/vm/registry.rs 'thread_absence_is_terminal()' 'ignore_terminal_progress()'
mutate 'secondary targets must reject retirement before registry mutation' \
    src/kernel/vm/registry.rs 'try_guest_stage2_retirement()' \
    'skip_guest_stage2_retirement()'
mutate 'common VM lifecycle must not regain host-architecture selection' \
    src/kernel/vm/registry.rs 'pub(super) struct VmControl' \
    '#[cfg(CONFIG_ARCH_AARCH64)] pub(super) struct VmControl'
mutate 'VMID completion must follow acknowledged residency retirement' \
    src/kernel/vm/memory.rs 'self.residency.finish_retirement(cut)' \
    'self.residency.finish_retirement_later(cut)'
mutate 'stage-2 request preparation must precede every retirement cut' \
    src/kernel/vm/memory.rs 'prepare_guest_stage2_retirement(capability, &self.stage2)' \
    'prepare_guest_stage2_retirement_later()'
mutate 'aggregate destruction must precede registry generation advance' \
    src/kernel/vm/registry.rs 'drop(owner);' 'core::mem::forget(owner);'
mutate 'guest stage-2 retirement needs a distinct RPC reason' \
    src/kernel/irq/cross_call.rs 'KernelRpcReasons::GUEST_STAGE2' \
    'KernelRpcReasons::USER_ADDRESS_SPACE'
mutate 'retirement asm outputs must not overlap live inputs' \
    src/arch/aarch64/stage2.rs 'saved_hcr = out(reg) _' 'saved_hcr = lateout(reg) _'
