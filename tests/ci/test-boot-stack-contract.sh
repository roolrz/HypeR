#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Exercise bootstrap-stack source ratchets against decoy declarations.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/hyper-boot-stack-test.XXXXXX")
trap 'rm -rf "$fixture"' EXIT HUP INT TERM

mkdir -p \
    "$fixture/src/arch/aarch64/memory" \
    "$fixture/src/arch/riscv64/memory" \
    "$fixture/src/arch/x86_64"

write_linkers() {
    for linker in \
        "$fixture/src/arch/aarch64/linker.ld" \
        "$fixture/src/arch/riscv64/linker.ld" \
        "$fixture/src/arch/x86_64/linker.ld"; do
        printf '%s\n' 'BOOT_STACK_SIZE = 256K;' >"$linker"
    done
}

write_sources() {
    printf '%s\n' 'pub(super) const KERNEL_STACK_PAGES: usize = 64;' \
        >"$fixture/src/arch/aarch64/memory/page_table.rs"
    printf '%s\n' 'const KERNEL_STACK_PAGES: usize = 64;' \
        >"$fixture/src/arch/riscv64/memory/page_table.rs"
    printf '%s\n' 'const STACK_PAGES: usize = 64;' \
        >"$fixture/src/arch/x86_64/memory.rs"
}

check() {
    HYPER_BOOT_STACK_ROOT="$fixture" sh "$root/tests/ci/check-boot-stack-contract.sh"
}

write_linkers
write_sources
check

printf '%s\n' '// const KERNEL_STACK_PAGES: usize = 64;' \
    'const KERNEL_STACK_PAGES: usize = 16;' \
    >"$fixture/src/arch/riscv64/memory/page_table.rs"
if check >/dev/null 2>&1; then
    echo "commented stack constants must not satisfy the source ratchet" >&2
    exit 1
fi

write_sources
printf '%s\n' 'BOOT_STACK_SIZE = 256K;' 'BOOT_STACK_SIZE = 256K;' \
    >"$fixture/src/arch/x86_64/linker.ld"
if check >/dev/null 2>&1; then
    echo "duplicate linker stack declarations must be rejected" >&2
    exit 1
fi
