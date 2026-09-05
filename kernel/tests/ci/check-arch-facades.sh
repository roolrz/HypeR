#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Enforce the binary HAL as the only downward edge into selected machine code.
set -eu

root=${HYPER_ARCH_FACADE_ROOT:-$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)}
cd "$root"

if [ ! -f src/main.rs ] || [ ! -f src/hal/selected/mod.rs ] ||
    ! LC_ALL=C rg -q -U \
        '#\s*\[\s*path\s*=\s*"hal/selected/mod\.rs"\s*\]\s*mod\s+hal\s*;' \
        src/main.rs; then
    echo "the kernel binary must path-map src/hal/selected/mod.rs as crate::hal" >&2
    exit 1
fi

set -- src
if [ -d tests/kernel ]; then
    set -- "$@" tests/kernel
fi

# These patterns deliberately reject crate-root aliases as well as direct
# paths. Otherwise `use crate as root; root::arch::...` would make a lexical
# boundary check trivial to bypass. The binary does not need crate aliases in
# policy code, so rejecting that uncommon form keeps the rule auditable.
arch_references=$(LC_ALL=C rg -n -U --glob '*.rs' \
    '\b(?:r#)?arch\s*::|use\s+(?:self\s*::\s*)?(?:r#)?arch\b|use\s+crate\s*::\s*\{[^;}]*\b(?:r#)?arch\b|use\s+crate\s+as\s+[A-Za-z_][A-Za-z0-9_]*|use\s+crate\s*::\s*\{[^;}]*\bself\s+as\s+[A-Za-z_][A-Za-z0-9_]*|extern\s+crate\s+self\s+as\s+[A-Za-z_][A-Za-z0-9_]*' \
    "$@" || true)
arch_bypasses=$(printf '%s\n' "$arch_references" |
    sed '\#^src/arch/#d' |
    sed '\#^src/hal/selected/#d' |
    sed '\#^[^:]*:[0-9][0-9]*:[[:space:]]*//!\{0,1\}#d' |
    sed '/^$/d')

if [ -n "$arch_bypasses" ]; then
    echo "non-architecture code must reach machine mechanisms through crate::hal:" >&2
    printf '%s\n' "$arch_bypasses" >&2
    exit 1
fi

# The selected HAL is a one-way adapter. An upward edge into kernel policy
# would turn it into a second entry layer and make initialization and ownership
# cycles architecture-dependent.
kernel_bypasses=
if [ -d src/hal/selected ]; then
    kernel_bypasses=$(LC_ALL=C rg -n -U --glob '*.rs' \
        '\b(?:r#)?kernel\s*::|use\s+(?:self\s*::\s*)?(?:r#)?kernel\b|use\s+crate\s*::\s*\{[^;}]*\b(?:r#)?kernel\b|use\s+crate\s+as\s+[A-Za-z_][A-Za-z0-9_]*|use\s+crate\s*::\s*\{[^;}]*\bself\s+as\s+[A-Za-z_][A-Za-z0-9_]*|extern\s+crate\s+self\s+as\s+[A-Za-z_][A-Za-z0-9_]*|#\s*\[\s*path\s*=\s*"[^"]*kernel|include!\s*\(' \
        src/hal/selected | sed '\#^[^:]*:[0-9][0-9]*:[[:space:]]*//!\{0,1\}#d' || true)
fi

if [ -n "$kernel_bypasses" ]; then
    echo "the selected HAL must not depend on crate::kernel:" >&2
    printf '%s\n' "$kernel_bypasses" >&2
    exit 1
fi
