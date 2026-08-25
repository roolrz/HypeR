#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Protect cache-line ownership and late-secondary lifetime at the CPU_ON seam.
set -eu

root=${HYPER_SECONDARY_HANDOFF_ROOT:-$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)}
cd "$root"

source=src/kernel/cpu/smp.rs
cache_source=src/hal/selected/cache.rs
fixture=$(mktemp -d "${TMPDIR:-/tmp}/hyper-secondary-handoff-check.XXXXXX")
trap 'rm -rf "$fixture"' EXIT HUP INT TERM

sed -n '/^pub fn initialize(/,/^}/p' "$source" >"$fixture/initialize.rs"
sed -n '/^extern "C" fn enter_clean_idle(/,/^}/p' "$source" >"$fixture/enter-clean-idle.rs"

require_in() {
    checked_source=$1
    pattern=$2
    message=$3
    if ! rg -q -U "$pattern" "$checked_source"; then
        echo "$message" >&2
        exit 1
    fi
}

require() {
    require_in "$source" "$1" "$2"
}

if rg -q -U 'try_box\([^)]*SecondaryBootParameters' "$source"; then
    echo 'secondary boot parameters must not use cache-line-sharing slab storage' >&2
    exit 1
fi

require \
    'struct SecondaryBootHandoff \{[^}]*parameters: NonNull<crate::hal::cpu::SecondaryBootParameters>[^}]*_block: PageBlock' \
    'a secondary handoff must retain a dedicated buddy block and typed record pointer'
require \
    'PublicationLayout::new\([[:space:]]*physical_start,[[:space:]]*block_start,[[:space:]]*block_size,[[:space:]]*parameter_size,[[:space:]]*align_of::<crate::hal::cpu::SecondaryBootParameters>\(\),[[:space:]]*line_size,[[:space:]]*\)' \
    'secondary handoffs must use the checked physical-first publication layout'
require \
    'let context = layout\.physical_address\(\)\.get\(\);[[:space:]]*let parameters_address = layout[[:space:]]*\.virtual_address\(\)[[:space:]]*\.as_usize\(\)[[:space:]]*\.ok_or\(Error::InvalidAddress\)\?;[[:space:]]*let published_size = layout\.published_size\(\);' \
    'firmware, Rust, and cache-publication ranges must come from the same validated layout'
require \
    'publish_data_range\(parameters_address, published_size\)' \
    'the initialized dedicated record must be cache-published before CPU_ON'
require_in "$fixture/initialize.rs" \
    'boot_parameters\.mark_observable\(\);[^}]*cpu_on\(' \
    'failure-path retention must be armed before firmware can observe the context'
require \
    'let records = core::mem::take\(&mut self\.records\);[[:space:]]*core::mem::forget\(records\);' \
    'possibly observed handoffs must not be returned to the allocator on failure'
require_in "$fixture/initialize.rs" \
    'online\.load\(Ordering::Acquire\)' \
    'the boot CPU must acquire secondary handoff consumption'
require_in "$fixture/enter-clean-idle.rs" \
    'ONLINE\[cpu_index\]\.store\(true, Ordering::Release\)' \
    'secondaries must release-publish completion after consuming the handoff'
require_in "$fixture/initialize.rs" \
    '(?s)online\.load\(Ordering::Acquire\).*?boot_parameters\.release\(\);' \
    'the all-online path must reclaim dedicated handoff storage'

if ! rg -q -U \
    '!valid_page_subdivision\(data_line_size\(\)\)[^}]*!valid_page_subdivision\(instruction_line_size\(\)\)[^}]*CacheError::InvalidLineSize' \
    "$cache_source"; then
    echo 'selected cache geometry must keep both line sizes within page ownership' >&2
    exit 1
fi

if ! rg -q \
    'page_ownership_supports_line\(line_size, page_size\)' \
    "$cache_source"; then
    echo 'selected cache admission must use the neutral page-ownership proof' >&2
    exit 1
fi
