#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Prove that representative guest-residency regressions are rejected.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/hyper-guest-residency-test.XXXXXX")
trap 'rm -rf "$fixture"' EXIT HUP INT TERM

copy_sources() {
    rm -rf "$fixture/src"
    mkdir -p "$fixture/src/mm" "$fixture/src/kernel/vm/vcpu" \
        "$fixture/src/kernel/task" "$fixture/src/kernel/mm/user_space" \
        "$fixture/src/kernel/process"
    cp "$root/src/mm/address_space_state.rs" "$fixture/src/mm/address_space_state.rs"
    cp "$root/src/kernel/vm/memory.rs" "$fixture/src/kernel/vm/memory.rs"
    cp "$root/src/kernel/vm/registry.rs" "$fixture/src/kernel/vm/registry.rs"
    cp "$root/src/kernel/vm/active_vcpu.rs" "$fixture/src/kernel/vm/active_vcpu.rs"
    cp "$root/src/kernel/vm/vcpu/transition.rs" "$fixture/src/kernel/vm/vcpu/transition.rs"
    cp "$root/src/kernel/task/thread.rs" "$fixture/src/kernel/task/thread.rs"
    cp "$root/src/kernel/mm/user_space/machine.rs" \
        "$fixture/src/kernel/mm/user_space/machine.rs"
    cp "$root/src/kernel/process/owner.rs" "$fixture/src/kernel/process/owner.rs"
}

check() {
    HYPER_GUEST_RESIDENCY_ROOT="$fixture" \
        sh "$root/tests/ci/check-guest-residency-contract.sh"
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
mutate 'retirement must permanently close residency' src/mm/address_space_state.rs \
    'self.phase = ResidencyPhase::Retired' 'self.phase = ResidencyPhase::Open'
mutate 'cuts must remain bound to the state that minted them' src/mm/address_space_state.rs \
    'cut.owner_nonce != self.owner_nonce' 'false'
mutate 'guest residency must attach to the composite execution claim' \
    src/kernel/vm/vcpu/transition.rs 'claim.attach_residency(residency)' \
    'let _ = residency;'
mutate 'execution release must consume guest residency first' src/kernel/vm/vcpu/transition.rs \
    'super::memory::leave(binding, residency)' \
    'super::memory::forget_residency(binding, residency)'
mutate 'migratable payload must not regain an unsafe Send escape hatch' \
    src/kernel/task/thread.rs 'pub struct VcpuExecution {' \
    'unsafe impl Send for VcpuExecution {}\npub struct VcpuExecution {'
mutate 'CPU-local ownership must retain the exact VM execution claim' \
    src/kernel/vm/active_vcpu.rs 'claim: Option<super::registry::VmExecutionClaim>' \
    'claim: Option<usize>'
mutate 'registry release must reject a still-armed residency' src/kernel/vm/registry.rs \
    'if claim.residency.is_some()' 'if false'
mutate 'active mapping changes must advance residency epoch' src/kernel/vm/memory.rs \
    'advance_single_active(cpu.get(), previous_epoch, self.translation_epoch)' \
    'ignore_active_epoch_advance(cpu.get(), previous_epoch, self.translation_epoch)'
mutate 'native retirement must consume unique ownership' \
    src/kernel/mm/user_space/machine.rs 'mut owner: UniqueFallibleArc<Self>' '\&mut self'
mutate 'residency retirement must precede identifier reuse' \
    src/kernel/mm/user_space/machine.rs 'state.residency.finish_retirement(cut)' \
    'state.residency.finish_retirement_later(cut)'

copy_sources
sed '/^pub(in crate::kernel) fn leave(/,/^}/ s/incarnation.translation_epoch()/claim.admitted.translation_epoch()/' \
    "$fixture/src/kernel/vm/memory.rs" >"$fixture/mutated"
mv "$fixture/mutated" "$fixture/src/kernel/vm/memory.rs"
if check >/dev/null 2>&1; then
    echo 'leave must tolerate a legitimate active mapping epoch advance' >&2
    exit 1
fi
