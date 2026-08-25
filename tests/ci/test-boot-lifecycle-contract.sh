#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Exercise the boot lifecycle ordering and topology publication contract.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/hyper-boot-lifecycle-test.XXXXXX")
trap 'rm -rf "$fixture"' EXIT HUP INT TERM

copy_sources() {
    rm -rf "$fixture/src"
    mkdir -p "$fixture/src/kernel/mm" "$fixture/src/kernel/cpu"
    cp "$root/src/main.rs" "$fixture/src/main.rs"
    cp "$root/src/kernel/mm/mod.rs" "$fixture/src/kernel/mm/mod.rs"
    cp "$root/src/kernel/cpu/smp.rs" "$fixture/src/kernel/cpu/smp.rs"
}

check() {
    HYPER_BOOT_LIFECYCLE_ROOT="$fixture" \
        sh "$root/tests/ci/check-boot-lifecycle-contract.sh"
}

mutate() {
    description=$1
    source_file=$2
    expression=$3
    copy_sources
    sed "$expression" "$fixture/$source_file" >"$fixture/mutated"
    mv "$fixture/mutated" "$fixture/$source_file"
    if check >/dev/null 2>&1; then
        echo "$description" >&2
        exit 1
    fi
}

swap_calls() {
    description=$1
    first=$2
    second=$3
    copy_sources
    sed -e "s/$first/__BOOT_LIFECYCLE_SWAP__/" \
        -e "s/$second/$first/" \
        -e "s/__BOOT_LIFECYCLE_SWAP__/$second/" \
        "$fixture/src/main.rs" >"$fixture/mutated"
    mv "$fixture/mutated" "$fixture/src/main.rs"
    if check >/dev/null 2>&1; then
        echo "$description" >&2
        exit 1
    fi
}

copy_sources
check

swap_calls 'memory must precede scheduler initialization' \
    'crate::kernel::mm::initialize' 'crate::kernel::task::initialize'
swap_calls 'SMP must precede address-space sealing' \
    'crate::kernel::cpu::initialize' 'crate::kernel::mm::seal_address_space'
swap_calls 'address-space sealing must precede platform drivers' \
    'crate::kernel::mm::seal_address_space' 'crate::kernel::device::platform_device_initialize'
swap_calls 'platform drivers must precede full VM initialization' \
    'crate::kernel::device::platform_device_initialize' 'crate::kernel::vm::initialize'
mutate 'address-space sealing without the stage-1 lock was accepted' \
    src/kernel/mm/mod.rs 's/stack::serialize_stage1_mutation/stack::without_stage1_serialization/'
mutate 'relaxed FrozenTopology publication was accepted' \
    src/kernel/cpu/smp.rs 's/next_cpu_index, Ordering::Release/next_cpu_index, Ordering::Relaxed/'
mutate 'removing the FrozenTopology capability was accepted' \
    src/kernel/cpu/smp.rs 's/frozen_topology/frozen_topology_removed/'
