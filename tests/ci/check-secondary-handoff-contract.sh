#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Protect cache-line ownership and late-secondary lifetime at the CPU_ON seam.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
cd "$root"

source=src/kernel/cpu/smp.rs
cache_source=src/hal/selected/cache.rs

require() {
    pattern=$1
    message=$2
    if ! rg -q -U "$pattern" "$source"; then
        echo "$message" >&2
        exit 1
    fi
}

if rg -q -U 'try_box\([^)]*SecondaryBootParameters' "$source"; then
    echo 'secondary boot parameters must not use cache-line-sharing slab storage' >&2
    exit 1
fi

require \
    'struct SecondaryBootHandoff \{[^}]*parameters: NonNull<crate::hal::cpu::SecondaryBootParameters>[^}]*_block: PageBlock' \
    'a secondary handoff must retain a dedicated buddy block and typed record pointer'
require \
    'let context =[^;]*align_up_u64\(physical_start, physical_alignment\)[^;]*;[[:space:]]*let offset = context[^;]*;[[:space:]]*let parameters_address = block_start' \
    'the firmware context must be physically aligned before deriving its linear alias'
require \
    'if !parameters_address\.is_multiple_of\(placement_alignment\)' \
    'the typed linear alias must be validated independently of physical alignment'
require \
    'if published_end > block_end' \
    'cache-line-rounded publication must remain inside the owned buddy block'
require \
    'publish_data_range\(parameters_address, parameter_size\)' \
    'the initialized dedicated record must be cache-published before CPU_ON'
require \
    'boot_parameters\.mark_observable\(\);[^}]*cpu_on\(' \
    'failure-path retention must be armed before firmware can observe the context'
require \
    'let records = core::mem::take\(&mut self\.records\);[[:space:]]*core::mem::forget\(records\);' \
    'possibly observed handoffs must not be returned to the allocator on failure'
require \
    'online\.load\(Ordering::Acquire\)' \
    'the boot CPU must acquire secondary handoff consumption'
require \
    'ONLINE\[cpu_index\]\.store\(true, Ordering::Release\)' \
    'secondaries must release-publish completion after consuming the handoff'
require \
    'boot_parameters\.release\(\);' \
    'the all-online path must reclaim dedicated handoff storage'

if ! rg -q -U \
    '!valid_page_subdivision\(data_line_size\(\)\)[^}]*!valid_page_subdivision\(instruction_line_size\(\)\)[^}]*CacheError::InvalidLineSize' \
    "$cache_source"; then
    echo 'selected cache geometry must keep both line sizes within page ownership' >&2
    exit 1
fi

if ! rg -q \
    'line_size != 0 && line_size\.is_power_of_two\(\) && line_size <= page_size' \
    "$cache_source"; then
    echo 'page-local cache publication requires nonzero power-of-two lines no larger than a page' >&2
    exit 1
fi
