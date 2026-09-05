#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Keep allocator corruption handoff non-returning, allocation-free, and ready
# before coordinated crash handling is published.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
coordination=${HYPER_ALLOCATOR_CRASH_COORDINATION:-$root/src/kernel/crash/coordination.rs}
heap=${HYPER_ALLOCATOR_CRASH_HEAP:-$root/src/mm/allocator/heap.rs}
main=${HYPER_ALLOCATOR_CRASH_MAIN:-$root/src/main.rs}

fixture=$(mktemp -d "${TMPDIR:-/tmp}/hyper-allocator-invariant-check.XXXXXX")
trap 'rm -rf "$fixture"' EXIT HUP INT TERM

sed -n '/^extern "C" fn start_kernel()/,/^}/p' "$main" >"$fixture/start-kernel.rs"
sed -n '/^pub(crate) fn early_initialize(/,/^}/p' "$coordination" >"$fixture/early-initialize.rs"
sed -n '/^pub(crate) fn initialize(/,/^}/p' "$coordination" >"$fixture/initialize.rs"
sed -n '/^fn allocator_invariant_failure(/,/^}/p' "$coordination" >"$fixture/bridge.rs"
sed -n '/^fn allocator_fault(/,/^}/p' "$heap" >"$fixture/fault.rs"

require() {
    pattern=$1
    source=$2
    message=$3
    LC_ALL=C rg -q -U "$pattern" "$source" || {
        echo "$message" >&2
        exit 1
    }
}

line_of() {
    LC_ALL=C rg -n -F -m1 "$1" "$2" | cut -d: -f1 || true
}

require 'let mut boot = crate::kernel::boot::enter_runtime\(\)\?;[[:space:]]*crate::kernel::crash::early_initialize\(\)\?;[[:space:]]*crate::kernel::device::early_initialize' \
    "$fixture/start-kernel.rs" \
    'allocator crash policy must initialize immediately after runtime entry and before devices'
require 'crate::kernel::crash::early_initialize\(\)\?;[[:space:]]*crate::kernel::device::early_initialize[^;]*;[[:space:]]*crate::kernel::mm::initialize\(\)\?;' \
    "$fixture/start-kernel.rs" \
    'allocator crash policy must initialize before runtime memory'
require 'install_allocator_invariant_handler\(allocator_invariant_failure\)' \
    "$fixture/early-initialize.rs" \
    'early crash initialization must install allocator invariant policy'
if LC_ALL=C rg -q -F 'install_allocator_invariant_handler' "$fixture/initialize.rs"; then
    echo "full crash initialization must not republish allocator invariant policy" >&2
    exit 1
fi
ready_line=$(line_of 'super::state::mark_ready();' "$fixture/initialize.rs")
console_line=$(line_of 'super::console::initialize()' "$fixture/initialize.rs")
registration_line=$(line_of 'register_shared_mapping(' "$fixture/initialize.rs")
if [ -z "$ready_line" ] || [ -z "$console_line" ] || [ -z "$registration_line" ] ||
    [ "$console_line" -ge "$ready_line" ] || [ "$registration_line" -ge "$ready_line" ]; then
    echo "crash readiness must follow console and optional crash-IPI preparation" >&2
    exit 1
fi
if [ "$(LC_ALL=C rg -c -F 'super::state::mark_ready();' "$fixture/initialize.rs")" -ne 1 ]; then
    echo "crash initialization must have one final readiness publication" >&2
    exit 1
fi

require 'LAST_ALLOCATOR_FAULT\.compare_exchange[\s\S]*ALLOCATOR_INVARIANT_HANDLER\.get\(\)' \
    "$fixture/fault.rs" \
    'allocator faults must record the first code before handler dispatch'
require 'handler\(report\)' "$fixture/fault.rs" \
    'allocator invariant dispatch must transfer the typed report to a non-returning handler'
require 'fatal\(format_args!' "$fixture/bridge.rs" \
    'allocator invariant bridge must enter coordinated fatal handling'

if LC_ALL=C rg -q \
    'GLOBAL_ALLOCATOR|allocator_fault_code|statistics\(|\.stats\(|crate::(?:pr_|print)|::log::|alloc::|\b(?:Box|String|Vec)\b|\.with\(' \
    "$fixture/bridge.rs"; then
    echo "allocator invariant bridge must not allocate, query the heap, or use ordinary logging/locks" >&2
    exit 1
fi
