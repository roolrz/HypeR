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
    mkdir -p "$fixture/src/kernel/vm/vcpu" "$fixture/src/kernel/vm/device" \
        "$fixture/src/kernel/vm/registry" \
        "$fixture/src/kernel/vm/memory" \
        "$fixture/src/kernel/entry" "$fixture/src/kernel/irq" \
        "$fixture/hal/src/hal" "$fixture/src/hal" \
        "$fixture/hal/src/arch/aarch64"
    cp "$root/src/kernel/vm/registry.rs" "$fixture/src/kernel/vm/registry.rs"
    cp "$root/src/kernel/vm/registry/construction.rs" \
        "$fixture/src/kernel/vm/registry/construction.rs"
    cp "$root/src/kernel/vm/registry/control.rs" \
        "$fixture/src/kernel/vm/registry/control.rs"
    cp "$root/src/kernel/vm/registry/execution.rs" \
        "$fixture/src/kernel/vm/registry/execution.rs"
    cp "$root/src/kernel/vm/lifecycle.rs" "$fixture/src/kernel/vm/lifecycle.rs"
    cp "$root/src/kernel/vm/device.rs" "$fixture/src/kernel/vm/device.rs"
    cp "$root/src/kernel/vm/device/aarch64.rs" "$fixture/src/kernel/vm/device/aarch64.rs"
    cp "$root/src/kernel/vm/vcpu/runner.rs" "$fixture/src/kernel/vm/vcpu/runner.rs"
    cp "$root/src/kernel/vm/vcpu/execution.rs" "$fixture/src/kernel/vm/vcpu/execution.rs"
    cp "$root/src/kernel/entry/irq.rs" "$fixture/src/kernel/entry/irq.rs"
    cp "$root/src/kernel/vm/memory.rs" "$fixture/src/kernel/vm/memory.rs"
    cp "$root/src/kernel/vm/memory/retirement.rs" \
        "$fixture/src/kernel/vm/memory/retirement.rs"
    cp "$root/src/kernel/irq/cross_call.rs" "$fixture/src/kernel/irq/cross_call.rs"
    cp "$root/hal/src/hal/vm.rs" "$fixture/hal/src/hal/vm.rs"
    cp "$root/src/hal/interrupt.rs" "$fixture/src/hal/interrupt.rs"
    cp "$root/hal/src/arch/aarch64/stage2.rs" "$fixture/hal/src/arch/aarch64/stage2.rs"
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
mutate 'serial endpoint must disconnect after registry cut' src/kernel/vm/registry/control.rs \
    'machine.disconnect_virtual_serial();' 'let _ = id;'
mutate 'VM retirement authority must remain linear' src/kernel/vm/registry/construction.rs \
    'pub(in crate::kernel::vm) struct VmControl' \
    '#[derive(Clone)] pub(in crate::kernel::vm) struct VmControl'
mutate 'VM control construction must remain registry-private' \
    src/kernel/vm/registry/construction.rs \
    'const fn mint_for_install' 'pub(super) const fn mint_for_install'
mutate 'raw registry cut must not bypass linear authority' src/kernel/vm/registry/control.rs \
    'fn begin_quiesce_control' 'pub(super) fn begin_quiesce_control'
mutate 'quiescence must use unique-owner conversion' src/kernel/vm/registry.rs \
    'machine.try_into_unique()' 'Ok(machine)'
mutate 'guest termination must arm persistent reaping' src/kernel/vm/vcpu/runner.rs \
    'ClosureReason::Guest(terminal_reason(' 'ClosureReason::Administrative(terminal_reason('
mutate 'runner must not accept admission close without durable stop' \
    src/kernel/vm/vcpu/runner.rs 'administrative_stop_reason(execution, current.thread)' \
    'Some(crate::kernel::vm::registry::AdministrativeStopReason::Requested)'
mutate 'reap-before-prompt must be recognized from endpoint state' \
    src/kernel/vm/registry/execution.rs 'thread_absence_is_terminal()' 'ignore_terminal_progress()'
mutate 'secondary targets must reject retirement before registry mutation' \
    src/kernel/vm/registry/control.rs 'try_guest_stage2_retirement()' \
    'skip_guest_stage2_retirement()'
mutate 'common VM lifecycle must not regain host-architecture selection' \
    src/kernel/vm/registry/construction.rs 'pub(in crate::kernel::vm) struct VmControl' \
    '#[cfg(CONFIG_ARCH_AARCH64)] pub(in crate::kernel::vm) struct VmControl'
mutate 'VMID completion must follow acknowledged residency retirement' \
    src/kernel/vm/memory/retirement.rs 'self.residency.finish_retirement(cut)' \
    'self.residency.finish_retirement_later(cut)'
mutate 'stage-2 request preparation must precede every retirement cut' \
    src/kernel/vm/memory/retirement.rs 'prepare_guest_stage2_retirement(capability, &self.stage2)' \
    'prepare_guest_stage2_retirement_later()'
mutate 'aggregate destruction must precede registry generation advance' \
    src/kernel/vm/registry/control.rs 'drop(owner);' 'core::mem::forget(owner);'
mutate 'guest stage-2 retirement needs a distinct RPC reason' \
    src/kernel/irq/cross_call.rs 'KernelRpcReasons::GUEST_STAGE2' \
    'KernelRpcReasons::USER_ADDRESS_SPACE'
mutate 'retirement asm outputs must not overlap live inputs' \
    hal/src/arch/aarch64/stage2.rs 'saved_hcr = out(reg) _' 'saved_hcr = lateout(reg) _'

mutate 'device closure must not precede administrative stop intent' src/kernel/vm/registry/control.rs \
    'let lease = REGISTRY.with' 'let _ = machine.quiesce_devices(); let lease = REGISTRY.with'

mutate 'registry lookup must survive until stop intent is published' src/kernel/vm/registry/control.rs \
    'let lease = REGISTRY.with' 'let _ = registry.begin_quiesce(id); let lease = REGISTRY.with'
mutate 'hot MMIO routes must survive until stop intent is published' src/kernel/vm/registry/control.rs \
    'let lease = REGISTRY.with' 'machine.close_io_routes(); let lease = REGISTRY.with'
mutate 'post-stop errors must not return reversible installed control' src/kernel/vm/registry/control.rs \
    'drop(lease);' 'drop(lease); return Err(error);'
mutate 'post-stop registry cut must not propagate a reversible error' src/kernel/vm/registry/control.rs \
    'drop(lease);' 'drop(lease); fallible_cut()?;'
