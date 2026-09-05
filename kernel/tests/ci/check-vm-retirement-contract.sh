#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Protect the private VM quiescence authority and irreversible cut ordering.
set -eu

root=${HYPER_VM_RETIREMENT_ROOT:-$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)}
cd "$root"

registry=src/kernel/vm/registry.rs
lifecycle=src/kernel/vm/lifecycle.rs
device=src/kernel/vm/device/aarch64.rs
runner=src/kernel/vm/vcpu/runner.rs
irq=src/kernel/entry/irq.rs
linux=src/kernel/vm/linux/mod.rs
memory=src/kernel/vm/memory.rs
cross_call=src/kernel/irq/cross_call.rs
hal_vm=src/hal/selected/vm.rs
hal_interrupt=src/hal/interrupt.rs
aarch_stage2=src/arch/aarch64/stage2.rs

if rg -n 'CONFIG_ARCH_' "$registry" "$memory" "$cross_call"; then
    echo 'common VM lifecycle and retirement policy must not select a host architecture' >&2
    exit 1
fi

begin=$(mktemp "${TMPDIR:-/tmp}/hyper-vm-retirement-begin.XXXXXX")
stops=$(mktemp "${TMPDIR:-/tmp}/hyper-vm-retirement-stops.XXXXXX")
promotion=$(mktemp "${TMPDIR:-/tmp}/hyper-vm-retirement-promotion.XXXXXX")
runner_activation=$(mktemp "${TMPDIR:-/tmp}/hyper-vm-retirement-runner.XXXXXX")
arch_retire=$(mktemp "${TMPDIR:-/tmp}/hyper-vm-retirement-arch.XXXXXX")
trap 'rm -f "$begin" "$stops" "$promotion" "$runner_activation" "$arch_retire"' EXIT HUP INT TERM
sed -n '/^fn begin_quiesce_control(/,/^}/p' "$registry" >"$begin"
sed -n '/^    fn request_all_stops(/,/^    fn is_quiescent(/p' "$registry" | sed '$d' >"$stops"
sed -n '/^    fn try_hold_quiescent(/,/^}/p' "$registry" >"$promotion"
sed -n '/if let Err(error) = super::activate(execution)/,/prepare_interrupts_for_entry/p' \
    "$runner" >"$runner_activation"
sed -n '/^pub(crate) fn retire_local(/,/^fn best_level(/p' "$aarch_stage2" | sed '$d' \
    >"$arch_retire"

line_first() {
    rg -n "$2" "$1" | sed -n '1s/:.*//p'
}

require_order() {
    first=$(line_first "$1" "$2")
    second=$(line_first "$1" "$3")
    if [ -z "$first" ] || [ -z "$second" ] || [ "$first" -ge "$second" ]; then
        echo "$4" >&2
        exit 1
    fi
}

rg -q 'control: VmControl' "$registry" &&
    rg -q 'static DEFAULT_VM: ControlLock' "$lifecycle" &&
    rg -q 'retain_default\(control\)' "$linux" || {
    echo 'installation must mint and boot policy must retain one linear VM control' >&2
    exit 1
}
if rg -U -q 'derive\([^)]*(Clone|Copy)[^)]*\)\][[:space:]]*pub\(super\) struct VmControl' \
    "$registry"; then
    echo 'VM lifecycle authority must not be cloneable' >&2
    exit 1
fi
rg -q '^    const fn mint_for_install\(id: VmId\)' "$registry" || {
    echo 'only registry installation may privately mint VM lifecycle authority' >&2
    exit 1
}
if rg -q 'pub.*fn (mint_for_install|begin_quiesce_control|poll_quiescent_control)' "$registry" ||
    rg -q 'VmControl[[:space:]]*\{' "$lifecycle"; then
    echo 'raw VmId retirement transitions and token construction must remain registry-private' >&2
    exit 1
fi

rg -q 'Installed\(FallibleArc<VirtualMachine>\)' "$registry" &&
    rg -q 'Quiescing\(FallibleArc<VirtualMachine>\)' "$registry" &&
    rg -q 'QuiescentHeld' "$registry" &&
    rg -q 'UniqueFallibleArc<VirtualMachine>' "$registry" || {
    echo 'registry must retain Installed, Quiescing, and unique-held typestates' >&2
    exit 1
}
if rg -q 'strong_count' "$registry"; then
    echo 'VM retirement must use try_into_unique instead of refcount polling' >&2
    exit 1
fi
require_order "$begin" 'registry.begin_quiesce\(id\)' 'clear_console_route_for_vm\(id\)' \
    'registry visibility must be cut before console producer routing'
require_order "$begin" 'clear_console_route_for_vm\(id\)' 'machine.request_all_stops\(\)' \
    'console routing must be cut before endpoint stop publication'
require_order "$stops" '\.request_stop\(' 'self.run_admission.close\(\)' \
    'run admission must close only after durable endpoint stop publication'
rg -q 'Error::ThreadNotFound' "$stops" &&
    rg -q 'thread_absence_is_terminal' "$stops" || {
    echo 'completion-before-prompt must be classified from exact endpoint progress' >&2
    exit 1
}
require_order "$promotion" 'machine.is_quiescent\(\)' 'machine.try_into_unique\(\)' \
    'quiescence proof must precede unique-owner conversion'

installed_checks=$(rg -o 'registry::is_installed\(vm\)' "$device" | wc -l | tr -d ' ')
if [ "$installed_checks" -ne 2 ]; then
    echo 'console publication must validate Installed on both sides of route publication' >&2
    exit 1
fi
rg -q 'clear_console_route_for_vm' "$device" || {
    echo 'quiescence must have a standalone exact VM console cut' >&2
    exit 1
}
rg -q 'lifecycle_machine\(publication.vm\)' "$registry" || {
    echo 'exact reaper completion must remain valid while the VM is Quiescing' >&2
    exit 1
}
rg -q 'VcpuClosureReason::Guest' "$runner" &&
    rg -q 'VcpuClosureReason::Administrative' "$runner" || {
    echo 'guest and administrative vCPU closure must both arm exact reaping' >&2
    exit 1
}
rg -q 'VmExecutionError::AdmissionClosed' "$runner" &&
    rg -q 'administrative_stop_reason' "$runner_activation" &&
    rg -q 'VmExecutionError::AdmissionClosed' "$irq" &&
    rg -q 'complete_detached_stop_if_requested' "$irq" || {
    echo 'admission-close activation races must recheck durable stop authority' >&2
    exit 1
}

rg -F -q 'RetiringHeld(FallibleArc<VirtualMachine>)' "$registry" &&
    rg -q 'RetiredHeld' "$registry" &&
    rg -q 'Destroying' "$registry" || {
    echo 'final retirement must retain explicit shared, unique, and destruction tombstones' >&2
    exit 1
}
require_order "$registry" 'try_guest_stage2_retirement\(\)' \
    'registry\.begin_retirement\(self\.id\)' \
    'stage-2 retirement capability must be acquired before registry mutation'
require_order "$registry" 'GuestStage2Transaction::try_acquire\(\)' \
    'registry\.begin_retirement\(self\.id\)' \
    'the fallible RPC reservation must precede the irreversible registry cut'
require_order "$registry" 'transport\.execute\(retirement\.local_request\(\)' \
    'address_space\.finish_retirement\(retirement\)' \
    'every target must acknowledge before stage-2 and VMID completion'
require_order "$registry" 'drop\(transport\)' 'registry\.promote_retired\(self\.id\)' \
    'the RPC owner must be released before recovering unique VM ownership'
require_order "$registry" 'registry\.begin_destroy\(self\.id\)' 'drop\(owner\)' \
    'the registry must publish a destruction tombstone before owner drop'
require_order "$registry" 'drop\(owner\)' 'registry\.finish_destroy\(self\.id\)' \
    'the VM aggregate must be destroyed before slot generation advances'

require_order "$memory" 'self\.residency\.finish_retirement\(cut\)' \
    'identifier\.complete\(\)' \
    'residency retirement must finish before VMID reuse'
rg -q 'Stage2Identifier::Retired' "$memory" || {
    echo 'acknowledged retirement must leave GuestAddressSpace explicitly drop-safe' >&2
    exit 1
}

rg -F -q 'GuestStage2(GuestStage2Call)' "$cross_call" &&
    rg -F -q 'KernelRpcReasons::GUEST_STAGE2' "$cross_call" &&
    rg -F -q 'pub const GUEST_STAGE2: Self' "$hal_interrupt" || {
    echo 'guest stage-2 retirement requires a distinct kernel RPC payload and reason' >&2
    exit 1
}
rg -q 'try_guest_stage2_retirement' "$hal_vm" || {
    echo 'HAL must expose a typed retirement capability precheck' >&2
    exit 1
}
require_order "$memory" 'prepare_guest_stage2_retirement\(capability, &self\.stage2\)' \
    'let cut = self' \
    'stage-2 request preparation must precede the residency retirement cut'

require_order "$arch_retire" 'mrs \{saved_hcr\}, HCR_EL2' \
    'msr VTCR_EL2, \{guest_vtcr\}' \
    'local retirement must save host translation state before guest selection'
require_order "$arch_retire" 'dsb ishst' 'tlbi VMALLS12E1' \
    'stage-2 descriptor publication must precede local combined invalidation'
require_order "$arch_retire" 'tlbi VMALLS12E1' '"dsb ish",' \
    'local combined invalidation must complete before restoring host state'
require_order "$arch_retire" \
    'csel \{restore_vttbr\}, xzr, \{saved_vttbr\}, eq' \
    'msr VTTBR_EL2, \{restore_vttbr\}' \
    'the exact retiring VTTBR must be replaced by a neutral selection'
if rg -q '(saved_hcr|saved_vttbr|saved_vtcr|guest_hcr|restore_vttbr) = lateout' \
    "$arch_retire"; then
    echo 'early-written retirement asm temporaries must not overlap live inputs' >&2
    exit 1
fi
