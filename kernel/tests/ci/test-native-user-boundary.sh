#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Prove the native-user dependency contract rejects representative regressions.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/hyper-native-user-boundary.XXXXXX")
trap 'rm -rf "$fixture"' EXIT HUP INT TERM

copy_sources() {
    rm -rf "$fixture/src"
    mkdir -p "$fixture/src/hal/selected" \
        "$fixture/src/kernel/mm/user_space" \
        "$fixture/src/kernel/entry" \
        "$fixture/src/kernel/process"
    cp "$root/src/hal/selected/user.rs" "$fixture/src/hal/selected/user.rs"
    cp "$root/src/kernel/mm/user_space/machine.rs" "$fixture/src/kernel/mm/user_space/machine.rs"
    cp "$root/src/kernel/mm/user_space/kernel_adapter.rs" "$fixture/src/kernel/mm/user_space/kernel_adapter.rs"
    cp "$root/src/kernel/mm/user_space/mod.rs" "$fixture/src/kernel/mm/user_space/mod.rs"
    cp "$root/src/kernel/entry/user.rs" "$fixture/src/kernel/entry/user.rs"
    cp "$root/src/kernel/process/owner.rs" "$fixture/src/kernel/process/owner.rs"
}

check() {
    HYPER_NATIVE_USER_BOUNDARY_ROOT="$fixture" \
        sh "$root/tests/ci/check-native-user-boundary.sh"
}

inject_and_reject() {
    description=$1
    file=$2
    injection=$3
    copy_sources
    printf '\n%s\n' "$injection" >>"$fixture/$file"
    if check >/dev/null 2>&1; then
        echo "$description" >&2
        exit 1
    fi
}

copy_sources
check
inject_and_reject 'kernel must reject target-specific selection' \
    src/kernel/mm/user_space/machine.rs '#[cfg(CONFIG_ARCH_AARCH64)] const BAD: usize = 1;'
inject_and_reject 'kernel must reject VHE mechanism policy' \
    src/kernel/mm/user_space/machine.rs 'const BAD: &str = "VheHostStage1";'
inject_and_reject 'kernel entry must reject backend token forgetting' \
    src/kernel/entry/user.rs 'fn bad<T>(completion: T) { core::mem::forget(completion); }'
inject_and_reject 'process policy must reject discriminant casts' \
    src/kernel/process/owner.rs 'fn bad(machine: MachineAbi) -> u8 { machine as u8 }'
inject_and_reject 'kernel must not recreate the identifier selection enum' \
    src/kernel/mm/user_space/machine.rs 'enum ReservedMachineIdentifier { Host, Second }'
inject_and_reject 'HAL must reject raw identifier construction seams' \
    src/hal/selected/user.rs 'impl AddressSpacePlan { pub(crate) unsafe fn prepare_address_space(self, identifier: u16) {} }'
inject_and_reject 'completion abandonment must not return normally' \
    src/hal/selected/user.rs 'impl CompletionFailure<'"'"'_> { pub(crate) fn abandon(self) {} }'

copy_sources
sed 's/SelectedIdentifier::HostStage(reserve_host/SelectedIdentifier::SecondStage(reserve_host/' \
    "$fixture/src/hal/selected/user.rs" >"$fixture/mutated"
mv "$fixture/mutated" "$fixture/src/hal/selected/user.rs"
if check >/dev/null 2>&1; then
    echo 'VHE selection must not reserve the second-stage namespace' >&2
    exit 1
fi

copy_sources
sed 's/core::mem::forget(completion);/drop(completion);/' \
    "$fixture/src/hal/selected/user.rs" >"$fixture/mutated"
mv "$fixture/mutated" "$fixture/src/hal/selected/user.rs"
if check >/dev/null 2>&1; then
    echo 'terminal completion handling must retain rather than drop its owner' >&2
    exit 1
fi
