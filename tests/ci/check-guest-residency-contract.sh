#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Protect linear guest residency and irreversible address-space cuts.
set -eu

root=${HYPER_GUEST_RESIDENCY_ROOT:-$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)}
cd "$root"

residency=src/mm/address_space_state.rs
memory=src/kernel/vm/memory.rs
registry=src/kernel/vm/registry.rs
thread=src/kernel/task/thread.rs
active=src/kernel/vm/active_vcpu.rs
transition=src/kernel/vm/vcpu/transition.rs
native=src/kernel/mm/user_space/machine.rs
process=src/kernel/process/owner.rs

activate=$(sed -n '/^pub(crate) unsafe fn activate(/,/^}/p' "$transition")
leave=$(sed -n '/^pub(in crate::kernel) fn leave(/,/^}/p' "$memory")
release=$(sed -n '/^fn release_execution_or_fail(/,/^}/p' "$transition")
native_retire=$(sed -n '/^    pub(crate) fn retire(/,/^\/\/\/ Owned guard/p' "$native")

line_of() {
    printf '%s\n' "$1" | LC_ALL=C rg -n -m1 "$2" | cut -d: -f1 || true
}

require_order() {
    body=$1
    first=$(line_of "$body" "$2")
    second=$(line_of "$body" "$3")
    if [ -z "$first" ] || [ -z "$second" ] || [ "$first" -ge "$second" ]; then
        echo "$4" >&2
        exit 1
    fi
}

LC_ALL=C rg -q 'pub struct UpdateCut' "$residency" &&
    LC_ALL=C rg -q 'pub struct RetirementCut' "$residency" &&
    LC_ALL=C rg -q 'pub fn finish_retirement\(' "$residency" &&
    LC_ALL=C rg -q 'cut: RetirementCut<CPUS>' "$residency" || {
    echo 'update and irreversible retirement cuts must remain distinct' >&2
    exit 1
}
owner_checks=$(LC_ALL=C rg -o 'cut\.owner_nonce != self\.owner_nonce' "$residency" | wc -l | tr -d ' ')
if [ "$owner_checks" -ne 2 ]; then
    echo 'update and retirement cuts must validate their unique state owner' >&2
    exit 1
fi
LC_ALL=C rg -q 'static NEXT_OWNER_NONCE: AtomicU64' "$residency" &&
    LC_ALL=C rg -q 'pub struct CutFailure<Cut>' "$residency" &&
    LC_ALL=C rg -q 'pub fn into_cut\(self\) -> Cut' "$residency" || {
    echo 'residency construction must mint unique owners and failures must retain cuts' >&2
    exit 1
}
LC_ALL=C rg -q 'self.phase = ResidencyPhase::Retired' "$residency" &&
    LC_ALL=C rg -q 'ResidencyPhase::Retired => Err\(ResidencyError::Retired\)' "$residency" || {
    echo 'finished retirement must permanently reject admission' >&2
    exit 1
}

LC_ALL=C rg -q 'cpu_affine: PhantomData<\*mut \(\)>' "$memory" &&
    LC_ALL=C rg -q 'impl Drop for GuestResidencyClaim' "$memory" || {
    echo 'guest residency must remain a CPU-affine linear capability' >&2
    exit 1
}
printf '%s\n' "$leave" | LC_ALL=C rg -q \
    'claim\.admitted\.same_allocation\(incarnation\)' &&
    printf '%s\n' "$leave" | LC_ALL=C rg -q \
        '\.leave\(current\.get\(\), incarnation\.translation_epoch\(\)\)' || {
    echo 'leave must validate stable allocation identity and consume the current epoch' >&2
    exit 1
}

LC_ALL=C rg -q 'residency: Option<super::memory::GuestResidencyClaim>' "$registry" &&
    LC_ALL=C rg -q 'if claim.residency.is_some\(\)' "$registry" || {
    echo 'guest residency must be structurally coupled to VM execution ownership' >&2
    exit 1
}
if LC_ALL=C rg -q 'unsafe impl Send for VcpuExecution|active_execution:' "$thread"; then
    echo 'migratable VcpuExecution must not contain or override CPU-affine ownership' >&2
    exit 1
fi
LC_ALL=C rg -q 'assert_send::<VcpuExecution>\(\)' "$thread" || {
    echo 'VcpuExecution migration safety must remain compiler-proven' >&2
    exit 1
}
LC_ALL=C rg -q 'claim: Option<super::registry::VmExecutionClaim>' "$active" &&
    LC_ALL=C rg -q 'static OWNERSHIP: PerCpu<ActiveOwnershipSlot>' "$active" || {
    echo 'active VM execution ownership must reside in an explicit CPU-local slot' >&2
    exit 1
}
require_order "$activate" 'super::memory::activate\(binding\)' \
    'claim\.attach_residency\(residency\)' \
    'residency must attach immediately after stage-2 admission'
require_order "$activate" 'claim\.attach_residency\(residency\)' \
    'crate::hal::vm::activate_hardware\(' \
    'the composite claim must be armed before later fallible hardware work'
require_order "$release" 'super::memory::leave\(binding, residency\)' \
    'binding\.release_execution\(claim, cpu\)' \
    'residency leave must precede execution and run-admission release'
printf '%s\n' "$release" | LC_ALL=C rg -q \
    'if execution\.vm_binding\(\)\.is_some\(\)' || {
    echo 'claimless timer validation must remain distinct from installed execution' >&2
    exit 1
}

LC_ALL=C rg -q 'advance_single_active\(cpu\.get\(\), previous_epoch, self\.translation_epoch\)' \
    "$memory" || {
    echo 'active mapping publication must advance the admitted residency epoch' >&2
    exit 1
}

printf '%s\n' "$native_retire" | LC_ALL=C rg -q \
    'mut owner: UniqueFallibleArc<Self>' &&
    LC_ALL=C rg -q 'NativeAddressSpace::retire\(address_space\)' "$process" || {
    echo 'native retirement must consume the exact unique address-space owner' >&2
    exit 1
}
require_order "$native_retire" 'residency\.finish_retirement\(cut\)' \
    'complete_identifier_retirement\(retiring\)' \
    'residency must become irreversibly retired before identifier reuse'
if printf '%s\n' "$native_retire" | LC_ALL=C rg -q 'fn retire\(&mut self\)'; then
    echo 'native retirement must not leave a safely reusable moved-out object' >&2
    exit 1
fi
