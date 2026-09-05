#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Exercise direct and aliased bypasses of the selected binary HAL boundary.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/hyper-arch-facade-test.XXXXXX")
trap 'rm -rf "$fixture"' EXIT HUP INT TERM

mkdir -p \
    "$fixture/src/arch" \
    "$fixture/src/hal/selected" \
    "$fixture/src/kernel" \
    "$fixture/tests/kernel"

policy_file=$fixture/src/kernel/policy.rs
selected_file=$fixture/src/hal/selected/cpu.rs

printf '%s\n' \
    '#[path = "hal/selected/mod.rs"]' \
    'mod hal;' >"$fixture/src/main.rs"
printf '%s\n' 'mod cpu;' >"$fixture/src/hal/selected/mod.rs"

check() {
    HYPER_ARCH_FACADE_ROOT="$fixture" sh "$root/tests/ci/check-arch-facades.sh"
}

expect_rejected() {
    description=$1
    if check >/dev/null 2>&1; then
        echo "$description" >&2
        exit 1
    fi
}

# Architecture internals may use their backend and the selected adapter may
# bind it. Policy and self-tests consume only crate::hal.
printf '%s\n' 'fn backend() { crate::arch::cpu::halt(); }' \
    >"$fixture/src/arch/backend.rs"
printf '%s\n' 'fn halt() { crate::arch::cpu::halt(); }' >"$selected_file"
printf '%s\n' 'fn stop() { crate::hal::cpu::halt(); }' >"$policy_file"
printf '%s\n' 'fn inspect() { crate::hal::memory::inspect(); }' \
    >"$fixture/tests/kernel/inspect.rs"
check

printf '%s\n' 'fn stop() { crate::arch::cpu::halt(); }' >"$policy_file"
expect_rejected "direct architecture paths outside the selected HAL must be rejected"

printf '%s\n' \
    'use crate::{arch as machine};' \
    'fn stop() { machine::cpu::halt(); }' >"$policy_file"
expect_rejected "grouped architecture aliases must be rejected"

printf '%s\n' \
    'use crate as root;' \
    'fn stop() { root::arch::cpu::halt(); }' >"$policy_file"
expect_rejected "crate-root aliases must not hide architecture access"

printf '%s\n' 'fn stop() { super::super::arch::cpu::halt(); }' >"$policy_file"
expect_rejected "relative architecture paths must be rejected"

printf '%s\n' \
    'use arch as machine;' \
    'fn stop() { machine::cpu::halt(); }' >"$policy_file"
expect_rejected "bare architecture imports must be rejected"

printf '%s\n' 'fn stop() { arch::cpu::halt(); }' >"$policy_file"
expect_rejected "bare architecture paths must be rejected"

printf '%s\n' 'fn inspect() { crate::arch::memory::inspect(); }' \
    >"$fixture/tests/kernel/inspect.rs"
expect_rejected "kernel self-tests must use the selected HAL"

printf '%s\n' 'fn inspect() { crate::hal::memory::inspect(); }' \
    >"$fixture/tests/kernel/inspect.rs"
printf '%s\n' 'fn stop() { crate::arch::cpu::halt(); }' \
    >"$fixture/src/hal/interrupt.rs"
expect_rejected "neutral HAL contracts must not select an architecture"

rm "$fixture/src/hal/interrupt.rs"
printf '%s\n' 'fn stop() { crate::kernel::cpu::halt(); }' >"$selected_file"
expect_rejected "the selected HAL must not call kernel policy"

printf '%s\n' \
    'use crate::{kernel as policy};' \
    'fn stop() { policy::cpu::halt(); }' >"$selected_file"
expect_rejected "selected-HAL kernel aliases must be rejected"

printf '%s\n' \
    'use kernel as policy;' \
    'fn stop() { policy::cpu::halt(); }' >"$selected_file"
expect_rejected "selected-HAL bare kernel imports must be rejected"

printf '%s\n' 'fn stop() { kernel::cpu::halt(); }' >"$selected_file"
expect_rejected "selected-HAL bare kernel paths must be rejected"

printf '%s\n' \
    'use crate as root;' \
    'fn stop() { root::kernel::cpu::halt(); }' >"$selected_file"
expect_rejected "selected-HAL crate-root aliases must not hide kernel access"

printf '%s\n' 'fn halt() { crate::arch::cpu::halt(); }' >"$selected_file"
printf '%s\n' 'mod hal;' >"$fixture/src/main.rs"
expect_rejected "the binary must keep the explicit selected-HAL path mapping"
