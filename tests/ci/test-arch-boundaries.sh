#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Exercise dependency-ratchet cases that are easy to weaken accidentally.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/hyper-arch-boundary-test.XXXXXX")
trap 'rm -rf "$fixture"' EXIT HUP INT TERM

mkdir -p "$fixture/src/arch/aarch64" "$fixture/src/kernel"
printf '%s\n' \
    'fn bootstrap() {' \
    '    let _ = crate::kernel::boot::ProtocolInputs::new();' \
    '    crate::kernel::boot::prepare_boot_environment();' \
    '}' >"$fixture/src/arch/aarch64/mod.rs"
printf '%s\t%s\t%s\n%s\t%s\t%s\n' \
    'src/arch/aarch64/mod.rs' 'crate::kernel::boot::ProtocolInputs::new' '1' \
    'src/arch/aarch64/mod.rs' 'crate::kernel::boot::prepare_boot_environment' '1' \
    >"$fixture/baseline.txt"

HYPER_ARCH_BOUNDARY_ROOT="$fixture" \
HYPER_ARCH_KERNEL_BASELINE="$fixture/baseline.txt" \
    sh "$root/tests/ci/check-arch-boundaries.sh"

printf '%s\n' 'fn hidden_log() { crate::println!("entry"); }' \
    >"$fixture/src/arch/hidden_log.rs"
if HYPER_ARCH_BOUNDARY_ROOT="$fixture" \
    HYPER_ARCH_KERNEL_BASELINE="$fixture/baseline.txt" \
    sh "$root/tests/ci/check-arch-boundaries.sh" >/dev/null 2>&1; then
    echo "architecture logging macros must not hide kernel dependencies" >&2
    exit 1
fi
rm "$fixture/src/arch/hidden_log.rs"

printf '%s\n' \
    'use crate::println as machine_log;' \
    'fn hidden_log() { machine_log!("entry"); }' \
    >"$fixture/src/arch/hidden_log.rs"
if HYPER_ARCH_BOUNDARY_ROOT="$fixture" \
    HYPER_ARCH_KERNEL_BASELINE="$fixture/baseline.txt" \
    sh "$root/tests/ci/check-arch-boundaries.sh" >/dev/null 2>&1; then
    echo "aliased root logging macros must not hide kernel dependencies" >&2
    exit 1
fi
rm "$fixture/src/arch/hidden_log.rs"

printf '%s\n' \
    'use crate::{pr_info as machine_log};' \
    'fn hidden_log() { machine_log!("entry"); }' \
    >"$fixture/src/arch/hidden_log.rs"
if HYPER_ARCH_BOUNDARY_ROOT="$fixture" \
    HYPER_ARCH_KERNEL_BASELINE="$fixture/baseline.txt" \
    sh "$root/tests/ci/check-arch-boundaries.sh" >/dev/null 2>&1; then
    echo "grouped root logging macros must not hide kernel dependencies" >&2
    exit 1
fi
rm "$fixture/src/arch/hidden_log.rs"

mkdir -p "$fixture/src/kernel/time"
printf '%s\n' \
    'fn poll() { crate::arch::vm::handle_virtual_timer_interrupt(); }' \
    >"$fixture/src/kernel/time/tick.rs"
if HYPER_ARCH_BOUNDARY_ROOT="$fixture" \
    HYPER_ARCH_KERNEL_BASELINE="$fixture/baseline.txt" \
    sh "$root/tests/ci/check-arch-boundaries.sh" >/dev/null 2>&1; then
    echo "host timekeeping must not own architecture VM policy" >&2
    exit 1
fi
rm "$fixture/src/kernel/time/tick.rs"

printf '%s\n' \
    'use crate::kernel::vm::{handle_guest_exit, hidden_policy};' \
    >"$fixture/src/arch/entry.rs"
if HYPER_ARCH_BOUNDARY_ROOT="$fixture" \
    HYPER_ARCH_KERNEL_BASELINE="$fixture/baseline.txt" \
    sh "$root/tests/ci/check-arch-boundaries.sh" >/dev/null 2>&1; then
    echo "grouped kernel imports must not bypass the architecture boundary" >&2
    exit 1
fi

printf '%s\n' \
    'fn dispatch() {' \
    '    crate::kernel::irq::interrupt::dispatch();' \
    '}' >"$fixture/src/arch/renamed_entry.rs"
printf '%s\t%s\t%s\n' \
    'src/arch/renamed_entry.rs' 'crate::kernel::irq::interrupt::dispatch' '1' \
    >"$fixture/baseline.txt"
if HYPER_ARCH_BOUNDARY_ROOT="$fixture" \
    HYPER_ARCH_KERNEL_BASELINE="$fixture/baseline.txt" \
    sh "$root/tests/ci/check-arch-boundaries.sh" >/dev/null 2>&1; then
    echo "exception entry must not bypass the named kernel entry adapters" >&2
    exit 1
fi

printf '%s\n' \
    'fn dispatch() {' \
    '    crate :: kernel::vm::handle_guest_sync();' \
    '}' >"$fixture/src/arch/renamed_entry.rs"
if HYPER_ARCH_BOUNDARY_ROOT="$fixture" \
    HYPER_ARCH_KERNEL_BASELINE="$fixture/baseline.txt" \
    sh "$root/tests/ci/check-arch-boundaries.sh" >/dev/null 2>&1; then
    echo "spaced kernel paths must not bypass the architecture boundary" >&2
    exit 1
fi

printf '%s\n' \
    'use crate::kernel;' \
    'fn dispatch() { kernel::vm::handle_guest_sync(); }' \
    >"$fixture/src/arch/renamed_entry.rs"
if HYPER_ARCH_BOUNDARY_ROOT="$fixture" \
    HYPER_ARCH_KERNEL_BASELINE="$fixture/baseline.txt" \
    sh "$root/tests/ci/check-arch-boundaries.sh" >/dev/null 2>&1; then
    echo "a root kernel import must not bypass the architecture boundary" >&2
    exit 1
fi

printf '%s\n' \
    'use crate::kernel::irq::interrupt as policy;' \
    'fn dispatch() { policy::dispatch(); }' \
    >"$fixture/src/arch/renamed_entry.rs"
if HYPER_ARCH_BOUNDARY_ROOT="$fixture" \
    HYPER_ARCH_KERNEL_BASELINE="$fixture/baseline.txt" \
    sh "$root/tests/ci/check-arch-boundaries.sh" >/dev/null 2>&1; then
    echo "a sensitive policy module alias must not bypass the architecture boundary" >&2
    exit 1
fi
