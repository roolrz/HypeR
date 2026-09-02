#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Keep the runtime boot phases explicit and prevent identity mappings from
# being retired before secondary CPUs leave their physical trampoline.
set -eu

root=${HYPER_BOOT_LIFECYCLE_ROOT:-$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)}
cd "$root"

fixture=$(mktemp -d "${TMPDIR:-/tmp}/hyper-boot-lifecycle-check.XXXXXX")
trap 'rm -rf "$fixture"' EXIT HUP INT TERM
sed -n '/^extern "C" fn start_kernel()/,/^}/p' src/main.rs >"$fixture/start-kernel.rs"
sed -n '/^    pub(crate) fn install(self)/,/^    }/p' \
    src/kernel/vm/registry.rs >"$fixture/vm-install.rs"

require() {
    pattern=$1
    source=$2
    message=$3
    LC_ALL=C rg -q -U "$pattern" "$source" || {
        echo "$message" >&2
        exit 1
    }
}

require_order() {
    source=$1
    first_pattern=$2
    second_pattern=$3
    message=$4
    first_line=$(LC_ALL=C rg -n -m1 "$first_pattern" "$source" | cut -d: -f1 || true)
    second_line=$(LC_ALL=C rg -n -m1 "$second_pattern" "$source" | cut -d: -f1 || true)
    if [ -z "$first_line" ] || [ -z "$second_line" ] || [ "$first_line" -ge "$second_line" ]; then
        echo "$message" >&2
        exit 1
    fi
}

previous=0
for call in \
    'crate::kernel::crash::early_initialize' \
    'crate::kernel::device::early_initialize' \
    'crate::kernel::mm::initialize' \
    'crate::kernel::debug::initialize' \
    'crate::kernel::task::initialize' \
    'crate::kernel::irq::initialize' \
    'crate::kernel::crash::initialize' \
    'crate::kernel::time::initialize' \
    'crate::kernel::cpu::initialize' \
    'crate::kernel::mm::activate_local_allocator_caches' \
    'crate::kernel::mm::seal_address_space' \
    'crate::kernel::device::platform_device_initialize' \
    'crate::kernel::vm::initialize' \
    'crate::kernel::vm::start_default'; do
    line=$(LC_ALL=C rg -n -F -m1 "$call" "$fixture/start-kernel.rs" | cut -d: -f1 || true)
    if [ -z "$line" ] || [ "$line" -le "$previous" ]; then
        echo "boot lifecycle call is missing or out of order: $call" >&2
        exit 1
    fi
    previous=$line
done

require 'pub\(crate\) fn seal_address_space\(\)[^{]*\{[[:space:]]*stack::serialize_stage1_mutation\(\|\| \{' \
    src/kernel/mm/mod.rs 'address-space sealing must share the runtime stage-1 mutation lock'
require 'PARTICIPATING_CPU_COUNT[[:space:]]*\.compare_exchange\([[:space:]]*0,[[:space:]]*next_cpu_index,[[:space:]]*Ordering::Release' \
    src/kernel/cpu/smp.rs 'SMP must release-publish its immutable participating CPU count'
require 'pub\(crate\) fn frozen_topology\(\) -> Option<FrozenTopology>' \
    src/kernel/cpu/smp.rs 'late per-CPU work must consume an explicit FrozenTopology capability'

require_order "$fixture/vm-install.rs" 'registry\.validate_install' \
    'activate_identifier_for_install' \
    'VM installation must validate its registry reservation before VMID activation'
require_order "$fixture/vm-install.rs" 'activate_identifier_for_install' 'let Self \{' \
    'PreparedVm must remain intact until every fallible installation step completes'
destructure_line=$(LC_ALL=C rg -n -m1 'let Self \{' "$fixture/vm-install.rs" | cut -d: -f1 || true)
if [ -z "$destructure_line" ]; then
    echo 'VM installation must have one explicit infallible publication tail' >&2
    exit 1
fi
if tail -n "+$destructure_line" "$fixture/vm-install.rs" | LC_ALL=C rg -q '\?'; then
    echo 'VM installation must not perform a fallible operation after destructuring PreparedVm' >&2
    exit 1
fi
