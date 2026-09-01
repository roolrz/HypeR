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
    'CONFIG_ARCH_|target_arch|crate::arch::|TranslationKind|VheHostStage1|NvheStage2Only|prepare_vhe|prepare_nvhe|levels_per_leaf' \
    'kernel native-user policy must not select an architecture translation mechanism'
reject "$entry" 'mem::forget\(completion\)|cfg.*ARCH' \
    'fatal completion ownership must be abandoned by HAL, not kernel cfg policy'
reject "$owner" 'as[[:space:]]+u8' \
    'host-machine matching must not depend on parallel enum discriminants'

require "$hal" 'struct AddressSpacePlan' \
    'HAL must own an opaque native address-space construction plan'
require "$hal" 'struct AddressSpaceIdentifier<HostStage, SecondStage>' \
    'HAL must bind typed identifier ownership to the selected plan'
require "$hal" 'enum SelectedIdentifier<HostStage, SecondStage>' \
    'HAL must keep the selected identifier namespace private'
reject "$hal" 'plan:[[:space:]]*AddressSpacePlan' \
    'a reserved identifier must have one regime discriminant, not an independent plan copy'
reject "$hal" 'identifier:[[:space:]]*u16' \
    'HAL must not erase the selected identifier before hierarchy construction'
reject "$machine" 'enum[[:space:]]+(ReservedMachineIdentifier|MachineIdentifier)' \
    'kernel must not reconstruct the private selected identifier variant'
require "$machine" 'identifier\.try_map\(' \
    'identifier activation and retirement must preserve the selected variant'
require "$machine" 'address_space_plan\(\)' \
    'kernel machine ownership must obtain one selected HAL construction plan'
require "$machine" 'identifier\.prepare_address_space' \
    'kernel machine ownership must build through the plan-bound identifier'
require "$entry" 'failure\.abandon_with\(' \
    'fatal completion abandonment must be structurally diverging'
reject "$hal" 'pub\(crate\)[[:space:]]+fn[[:space:]]+abandon\(' \
    'HAL must not expose a normally returning completion-abandon operation'
require "$owner" 'requested[[:space:]]*==[[:space:]]*crate::hal::user::host_machine\(\)' \
    'host-machine admission must compare typed values'

plan_line=$(grep -n 'let plan = crate::hal::user::address_space_plan()' "$machine" | head -n 1 | cut -d: -f1)
allocation_line=$(grep -n 'allocation_size()' "$machine" | head -n 1 | cut -d: -f1)
if [ -z "$plan_line" ] || [ -z "$allocation_line" ] || [ "$plan_line" -ge "$allocation_line" ]; then
    echo 'unsupported native-user machines must fail before kernel allocation' >&2
    exit 1
fi

reserve_body=$(sed -n '/pub(crate) fn reserve_identifier/,/^    }/p' "$hal" | tr '\n' ' ')
printf '%s\n' "$reserve_body" | grep -Eq \
    'TranslationKind::VheHostStage1.*SelectedIdentifier::HostStage\(reserve_host' &&
    printf '%s\n' "$reserve_body" | grep -Eq \
        'TranslationKind::NvheStage2Only.*SelectedIdentifier::SecondStage\(reserve_stage2' || {
    echo 'machine regime selection must reserve the matching identifier namespace' >&2
    exit 1
}

prepare_body=$(sed -n '/pub(crate) unsafe fn prepare_address_space/,/^    }/p' "$hal" | tr '\n' ' ')
printf '%s\n' "$prepare_body" | grep -Eq \
    'SelectedIdentifier::HostStage\(token\).*host_identity\(token\).*prepare_vhe_address_space' &&
    printf '%s\n' "$prepare_body" | grep -Eq \
        'SelectedIdentifier::SecondStage\(token\).*second_identity\(token\).*prepare_nvhe_address_space' || {
    echo 'one selected identifier variant must choose both identity and hierarchy builder' >&2
    exit 1
}

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
