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
    mkdir -p "$fixture/hal/src/hal" \
        "$fixture/src/kernel/mm/user_space" \
        "$fixture/src/kernel/entry" \
        "$fixture/src/kernel/process"
    mkdir -p "$fixture/hal/src/arch/aarch64"
    cp "$root/hal/src/arch/aarch64/context.S" "$fixture/hal/src/arch/aarch64/context.S"
    cp "$root/hal/src/hal/user.rs" "$fixture/hal/src/hal/user.rs"
    cp "$root/src/kernel/mm/user_space/machine.rs" "$fixture/src/kernel/mm/user_space/machine.rs"
    cp "$root/src/kernel/mm/user_space/kernel_adapter.rs" "$fixture/src/kernel/mm/user_space/kernel_adapter.rs"
    cp "$root/src/kernel/mm/user_space/mod.rs" "$fixture/src/kernel/mm/user_space/mod.rs"
    cp "$root/src/kernel/entry/user.rs" "$fixture/src/kernel/entry/user.rs"
    cp "$root/src/kernel/entry/services.rs" "$fixture/src/kernel/entry/services.rs"
    cp -R "$root/src/kernel/entry/services" "$fixture/src/kernel/entry/services"
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
    src/kernel/mm/user_space/machine.rs 'const BAD: &str = "HostStage1";'
inject_and_reject 'kernel entry must reject backend token forgetting' \
    src/kernel/entry/user.rs 'fn bad<T>(completion: T) { core::mem::forget(completion); }'
inject_and_reject 'service adapters must reject architecture selection' \
    src/kernel/entry/services/vfs.rs '#[cfg(target_arch = "aarch64")] const BAD: usize = 1;'
inject_and_reject 'process policy must reject discriminant casts' \
    src/kernel/process/owner.rs 'fn bad(machine: MachineAbi) -> u8 { machine as u8 }'
inject_and_reject 'kernel must not recreate the identifier selection enum' \
    src/kernel/mm/user_space/machine.rs 'enum ReservedMachineIdentifier { Host, Second }'
inject_and_reject 'HAL must reject removed translation-regime wrappers' \
    hal/src/hal/user.rs 'struct AddressSpaceIdentifier<T> { asid: T }'
inject_and_reject 'completion abandonment must not return normally' \
    hal/src/hal/user.rs 'impl CompletionFailure<'"'"'_> { pub(crate) fn abandon(self) {} }'

copy_sources
sed 's/plan.asid_bits()/16/' \
    "$fixture/src/kernel/mm/user_space/machine.rs" >"$fixture/mutated"
mv "$fixture/mutated" "$fixture/src/kernel/mm/user_space/machine.rs"
if check >/dev/null 2>&1; then
    echo 'Native ASID reservation must use the admitted hardware width' >&2
    exit 1
fi

copy_sources
sed 's/identity(identifier)/identity(unrelated_identifier)/' \
    "$fixture/src/kernel/mm/user_space/machine.rs" >"$fixture/mutated"
mv "$fixture/mutated" "$fixture/src/kernel/mm/user_space/machine.rs"
if check >/dev/null 2>&1; then
    echo 'Native page tables must use the retained ASID owner' >&2
    exit 1
fi

copy_sources
sed 's/core::mem::forget(completion);/drop(completion);/' \
    "$fixture/hal/src/hal/user.rs" >"$fixture/mutated"
mv "$fixture/mutated" "$fixture/hal/src/hal/user.rs"
if check >/dev/null 2>&1; then
    echo 'terminal completion handling must retain rather than drop its owner' >&2
    exit 1
fi

inject_and_reject 'Native entry must not truncate HCR through ESR_EL2' \
    hal/src/arch/aarch64/context.S '    msr esr_el2, x1'
