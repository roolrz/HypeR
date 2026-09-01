#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Prove that allocator-to-crash handoff regressions are rejected.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/hyper-allocator-invariant-test.XXXXXX")
trap 'rm -rf "$fixture"' EXIT HUP INT TERM

cp "$root/src/kernel/crash/coordination.rs" "$fixture/coordination.rs"
cp "$root/src/mm/allocator/heap.rs" "$fixture/heap.rs"
cp "$root/src/main.rs" "$fixture/main.rs"

check() {
    HYPER_ALLOCATOR_CRASH_COORDINATION="$fixture/coordination.rs" \
        HYPER_ALLOCATOR_CRASH_HEAP="$fixture/heap.rs" \
        HYPER_ALLOCATOR_CRASH_MAIN="$fixture/main.rs" \
        sh "$root/tests/ci/check-allocator-invariant-contract.sh"
}

expect_rejection() {
    if check >/dev/null 2>&1; then
        echo "$1" >&2
        exit 1
    fi
}

check

sed 's/install_allocator_invariant_handler/install_allocator_handler_later/' \
    "$fixture/coordination.rs" >"$fixture/modified.rs"
mv "$fixture/modified.rs" "$fixture/coordination.rs"
expect_rejection 'missing allocator invariant handler installation must be rejected'

cp "$root/src/kernel/crash/coordination.rs" "$fixture/coordination.rs"
sed -e 's/crate::kernel::crash::early_initialize()/__ALLOCATOR_CRASH_SWAP__()/' \
    -e 's/crate::kernel::mm::initialize()/crate::kernel::crash::early_initialize()/' \
    -e 's/__ALLOCATOR_CRASH_SWAP__()/crate::kernel::mm::initialize()/' \
    "$fixture/main.rs" >"$fixture/modified.rs"
mv "$fixture/modified.rs" "$fixture/main.rs"
expect_rejection 'allocator crash policy initialized after runtime memory must be rejected'

cp "$root/src/main.rs" "$fixture/main.rs"
sed 's/super::state::mark_ready()/super::state::mark_not_ready()/' \
    "$fixture/coordination.rs" >"$fixture/modified.rs"
mv "$fixture/modified.rs" "$fixture/coordination.rs"
expect_rejection 'full crash initialization without its final readiness publication must be rejected'

cp "$root/src/kernel/crash/coordination.rs" "$fixture/coordination.rs"
sed 's/fatal(format_args!/crate::kernel::mm::statistics(); fatal(format_args!/' \
    "$fixture/coordination.rs" >"$fixture/modified.rs"
mv "$fixture/modified.rs" "$fixture/coordination.rs"
expect_rejection 'allocator statistics in the corruption bridge must be rejected'

cp "$root/src/kernel/crash/coordination.rs" "$fixture/coordination.rs"
sed 's/handler(report)/ignore_allocator_corruption(report)/' \
    "$fixture/heap.rs" >"$fixture/modified.rs"
mv "$fixture/modified.rs" "$fixture/heap.rs"
expect_rejection 'dropping allocator invariant dispatch must be rejected'
