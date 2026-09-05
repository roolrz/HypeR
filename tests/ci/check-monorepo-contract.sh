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

grep -F -x 'hyper-abi = { path = "../sdk/abi" }' kernel/Cargo.toml >/dev/null ||
    fail "the kernel must consume the in-tree Native ABI crate"

if git ls-files --stage | awk '$1 == "160000" { found = 1 } END { exit !found }'; then
    fail "the source tree must not contain Git submodules"
fi

if git ls-files | grep -E '(^|/)components\.lock$' >/dev/null; then
    fail "single-tree components must not retain cross-repository revision locks"
fi

if rg -n 'github\.com/roolrz/HypeR-(ABI|Build|Kernel|Lib|Toolchain|Utils)' \
    --glob '!tests/ci/check-monorepo-contract.sh' . >/dev/null; then
    fail "source or documentation still depends on an abandoned component repository"
fi

if rg -n '#[[:space:]]*include[[:space:]]*[<"](\.\./|sdk/)' app >/dev/null; then
    fail "Native applications must include only installed SDK interfaces"
fi
if rg -n '^[[:space:]]*[A-Za-z0-9_-]+[[:space:]]*=[[:space:]]*\{[^}]*path[[:space:]]*=' \
    app --glob Cargo.toml >/dev/null; then
    fail "Native Rust applications must not use source-tree path dependencies"
fi
if rg -n 'hyper[_-]sys' app --glob Cargo.toml --glob '*.rs' >/dev/null; then
    fail "Native applications must use safe OS bindings rather than raw syscalls"
fi

for misplaced in \
    Cargo.toml \
    Kconfig \
    build.rs \
    configs \
    src \
    tests/host \
    tests/image \
    tests/kernel \
    tools/guest \
    tools/kallsyms \
    tools/kconfig; do
    [ ! -e "$misplaced" ] || fail "kernel-owned path remains at repository root: $misplaced"
done

for required in \
    kernel/.cargo/config.toml \
    kernel/Makefile \
    kernel/src/lib.rs \
    kernel/configs/qemu_aarch64_defconfig \
    kernel/docs/architecture.md \
    kernel/tests/ci/run.sh \
    kernel/tests/host/Cargo.toml \
    kernel/tools/guest/README.md \
    kernel/tools/kconfig/Cargo.toml \
    sdk/abi/include/hyper/native.h \
    sdk/lib/include/hyper/startup.h \
    sdk/rust/hyper-os/Cargo.toml \
    sdk/rust/hyper-rt/Cargo.toml \
    sdk/rust/hyper-sys/Cargo.toml \
    sdk/toolchain/bin/hyper-cargo \
    sdk/toolchain/bin/hyper-clang \
    app/init/Cargo.toml \
    app/init/src/main.rs; do
    [ -f "$required" ] || fail "missing monorepo component: $required"
done
