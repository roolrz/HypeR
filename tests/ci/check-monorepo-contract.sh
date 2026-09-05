#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Keep the Native SDK and its consumers in one coherent source revision.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
cd "$root"

fail() {
    echo "check-monorepo-contract.sh: $1" >&2
    exit 1
}

grep -F -x 'hyper-abi = { path = "sdk/abi" }' Cargo.toml >/dev/null ||
    fail "the kernel must consume the in-tree Native ABI crate"

if git ls-files --stage | awk '$1 == "160000" { found = 1 } END { exit !found }'; then
    fail "the source tree must not contain Git submodules"
fi

if git ls-files | grep -E '(^|/)components\.lock$' >/dev/null; then
    fail "single-tree components must not retain cross-repository revision locks"
fi

if rg -n 'github\.com/roolrz/HypeR-(ABI|Build|Lib|Toolchain|Utils)' \
    --glob '!tests/ci/check-monorepo-contract.sh' . >/dev/null; then
    fail "source or documentation still depends on an abandoned component repository"
fi

if rg -n '#[[:space:]]*include[[:space:]]*[<"](\.\./|sdk/)' system >/dev/null; then
    fail "Native system applications must include only installed SDK interfaces"
fi

for required in \
    sdk/abi/include/hyper/native.h \
    sdk/lib/include/hyper/startup.h \
    sdk/toolchain/bin/hyper-clang \
    system/apps/init/main.c; do
    [ -f "$required" ] || fail "missing monorepo component: $required"
done
