#!/bin/sh
# Exercise dependency-ratchet cases that are easy to weaken accidentally.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/hyper-arch-boundary-test.XXXXXX")
trap 'rm -rf "$fixture"' EXIT HUP INT TERM

mkdir -p "$fixture/src/arch" "$fixture/src/kernel"
printf '%s\n' \
    'fn dispatch() {' \
    '    crate::kernel::vm::handle_guest_exit();' \
    '}' >"$fixture/src/arch/entry.rs"
printf '%s\t%s\t%s\n' \
    'src/arch/entry.rs' 'crate::kernel::vm::handle_guest_exit' '1' \
    >"$fixture/baseline.txt"

HYPER_ARCH_BOUNDARY_ROOT="$fixture" \
HYPER_ARCH_KERNEL_BASELINE="$fixture/baseline.txt" \
    sh "$root/tests/ci/check-arch-boundaries.sh"

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
