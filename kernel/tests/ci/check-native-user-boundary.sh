#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Keep native-user policy independent from selected translation mechanisms.
set -eu

root=${HYPER_NATIVE_USER_BOUNDARY_ROOT:-$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)}
cd "$root"

machine=src/kernel/mm/user_space/machine.rs
adapter=src/kernel/mm/user_space/kernel_adapter.rs
module=src/kernel/mm/user_space/mod.rs
entry=src/kernel/entry/user.rs
owner=src/kernel/process/owner.rs
hal=src/hal/selected/user.rs

reject() {
    files=$1
    pattern=$2
    message=$3
    if grep -En "$pattern" $files >/dev/null 2>&1; then
        echo "$message" >&2
        exit 1
    fi
}

require() {
    file=$1
    pattern=$2
    message=$3
    if ! grep -En "$pattern" "$file" >/dev/null 2>&1; then
        echo "$message" >&2
        exit 1
    fi
}

kernel_user_files="$machine $adapter $module $entry $owner"
reject "$kernel_user_files" \
    'CONFIG_ARCH_|target_arch|crate::arch::|TranslationKind|HostStage1|NvheStage2Only|prepare_vhe|prepare_nvhe|levels_per_leaf' \
    'kernel native-user policy must not select an architecture translation mechanism'
reject "$entry" 'mem::forget\(completion\)|cfg.*ARCH' \
    'fatal completion ownership must be abandoned by HAL, not kernel cfg policy'
require "$hal" 'struct AddressSpacePlan' \
    'HAL must own an opaque native address-space construction plan'
reject "$hal $kernel_user_files" 'AddressSpaceIdentifier|SelectedIdentifier|Stage2Vmid|NvheStage2Only|prepare_nvhe' \
    'Native address spaces must not acquire guest VMID or removed regime wrappers'
require "$machine" 'identifier:[[:space:]]*ManuallyDrop<ActiveIdentifier<HostAsid>>' \
    'kernel address-space ownership must retain its typed active ASID'
require "$machine" 'reserve::<HostAsid>\(plan\.asid_bits\(\)\)' \
    'Native ASID reservation must use the admitted hardware width'
require "$machine" 'address_space_plan\(\)' \
    'kernel machine ownership must obtain selected HAL limits'
require "$machine" 'crate::hal::user::prepare_host_address_space\(' \
    'kernel machine ownership must build through the host-stage HAL'
require "$machine" 'let \(asid, generation\) = identity\(identifier\)' \
    'Native construction must project its retained identifier and generation'
reject "$machine" 'enum[[:space:]]+(ReservedMachineIdentifier|MachineIdentifier)' \
    'kernel must not reconstruct a removed translation-regime discriminant'
require "$entry" 'failure\.abandon_with\(' \
    'fatal completion abandonment must be structurally diverging'
reject "$hal" 'pub\(crate\)[[:space:]]+fn[[:space:]]+abandon\(' \
    'HAL must not expose a normally returning completion-abandon operation'
require "$owner" 'requested[[:space:]]*==[[:space:]]*crate::hal::user::host_machine\(\)' \
    'host-machine admission must compare typed values'
reject "$owner" 'machine(\(\))?[[:space:]]+as[[:space:]]+u8' \
    'host-machine matching must not depend on parallel enum discriminants'

plan_line=$(grep -n 'let plan = crate::hal::user::address_space_plan()' "$machine" | head -n 1 | cut -d: -f1)
allocation_line=$(grep -n 'allocation_size()' "$machine" | head -n 1 | cut -d: -f1)
if [ -z "$plan_line" ] || [ -z "$allocation_line" ] || [ "$plan_line" -ge "$allocation_line" ]; then
    echo 'unsupported native-user machines must fail before kernel allocation' >&2
    exit 1
fi

abandon_body=$(sed -n '/pub(crate) fn abandon_with/,/^    }/p' "$hal")
printf '%s\n' "$abandon_body" | grep -Eq 'core::mem::forget\(completion\)' || {
    echo 'terminal completion handling must retain the armed return owner' >&2
    exit 1
}
if printf '%s\n' "$abandon_body" | grep -Eq 'drop\(completion\)'; then
    echo 'terminal completion handling must not drop the armed return owner' >&2
    exit 1
fi
forget_line=$(printf '%s\n' "$abandon_body" | grep -n -m1 'core::mem::forget(completion)' | cut -d: -f1)
stop_line=$(printf '%s\n' "$abandon_body" | grep -n -m1 'match stop(error)' | cut -d: -f1)
if [ -z "$forget_line" ] || [ -z "$stop_line" ] || [ "$forget_line" -ge "$stop_line" ]; then
    echo 'armed completion ownership must be retained before entering fail-stop' >&2
    exit 1
fi
