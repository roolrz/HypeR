#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Preserve the allocation-free magazine and one-way publication boundary.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
global=${HYPER_ALLOCATOR_CACHE_GLOBAL:-$root/src/mm/allocator/heap/global.rs}
heap=${HYPER_ALLOCATOR_CACHE_HEAP:-$root/src/mm/allocator/heap.rs}
local_cache=${HYPER_ALLOCATOR_CACHE_LOCAL:-$root/src/mm/allocator/heap/local_cache.rs}

require() {
    pattern=$1
    source=$2
    message=$3
    LC_ALL=C rg -q -U "$pattern" "$source" || {
        echo "$message" >&2
        exit 1
    }
}

require 'const CACHE_LIMITS: \[usize; CACHED_CLASS_COUNT\] = \[[0-9, ]+\];' \
    "$global" 'allocator magazines must retain compile-time capacity bounds'
require 'caches_enabled\.store\(true, Ordering::Release\)' \
    "$global" 'allocator cache activation must use Release publication'
require 'caches_enabled\.load\(Ordering::Acquire\)' \
    "$global" 'allocator cache consumers must acquire activation publication'
require 'struct CachedObject \{[\s\S]*pointer: NonNull<u8>' \
    "$heap" 'cached slab ownership must retain pointer provenance'
require 'impl Drop for CachedObject[\s\S]*allocator_fault\(AllocatorFault::InvalidCacheState\)' \
    "$heap" 'abandoned cached ownership must fail-stop'
require 'pub\(super\) struct Magazine<T> \{[\s\S]*entries: \[Option<T>; MAGAZINE_STORAGE\]' \
    "$local_cache" 'magazine storage must remain fixed and allocation-free'

if LC_ALL=C rg -q \
    'alloc::|\b(?:Box|String|Vec)\b|crate::(?:kernel|log)|::log::|println!|print!' \
    "$local_cache"; then
    echo 'local allocator magazines must not allocate, log, or call kernel policy' >&2
    exit 1
fi
