#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Prove that allocator-cache publication and ownership regressions are rejected.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/hyper-allocator-cache-test.XXXXXX")
trap 'rm -rf "$fixture"' EXIT HUP INT TERM

copy_sources() {
    cp "$root/src/mm/allocator/heap/global.rs" "$fixture/global.rs"
    cp "$root/src/mm/allocator/heap.rs" "$fixture/heap.rs"
    cp "$root/src/mm/allocator/heap/local_cache.rs" "$fixture/local_cache.rs"
}

check() {
    HYPER_ALLOCATOR_CACHE_GLOBAL="$fixture/global.rs" \
        HYPER_ALLOCATOR_CACHE_HEAP="$fixture/heap.rs" \
        HYPER_ALLOCATOR_CACHE_LOCAL="$fixture/local_cache.rs" \
        sh "$root/tests/ci/check-allocator-cache-contract.sh"
}

expect_rejection() {
    if check >/dev/null 2>&1; then
        echo "$1" >&2
        exit 1
    fi
}

copy_sources
check

sed 's/caches_enabled.store(true, Ordering::Release)/caches_enabled.store(true, Ordering::Relaxed)/' \
    "$fixture/global.rs" >"$fixture/mutated.rs"
mv "$fixture/mutated.rs" "$fixture/global.rs"
expect_rejection 'relaxed allocator-cache activation publication must be rejected'

copy_sources
sed 's/pointer: NonNull<u8>/pointer: usize/' \
    "$fixture/heap.rs" >"$fixture/mutated.rs"
mv "$fixture/mutated.rs" "$fixture/heap.rs"
expect_rejection 'integer-only cached object ownership must be rejected'

copy_sources
sed 's/allocator_fault(AllocatorFault::InvalidCacheState)/loop { core::hint::spin_loop(); }/' \
    "$fixture/heap.rs" >"$fixture/mutated.rs"
mv "$fixture/mutated.rs" "$fixture/heap.rs"
expect_rejection 'cached ownership abandonment without allocator diagnostics must be rejected'

copy_sources
sed '1s|^|use alloc::vec::Vec;|' \
    "$fixture/local_cache.rs" >"$fixture/mutated.rs"
mv "$fixture/mutated.rs" "$fixture/local_cache.rs"
expect_rejection 'dynamic allocation in local magazine storage must be rejected'
