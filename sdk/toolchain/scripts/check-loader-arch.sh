#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0
set -eu
repository=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
temporary=$(mktemp -d "${TMPDIR:-/tmp}/hyper-loader-arch.XXXXXX")
trap 'rm -rf "$temporary"' EXIT
for architecture in aarch64 riscv64; do
    case "$architecture" in
        aarch64) flags='-D__aarch64__' ;;
        riscv64) flags='-D__riscv -D__riscv_xlen=64' ;;
    esac
    "${HOST_CC:-clang}" -std=c17 -Wall -Wextra -Werror \
        -U__aarch64__ -U__riscv -U__riscv_xlen $flags \
        "$repository/loader/tests/architecture.c" -o "$temporary/probe"
    "$temporary/probe"
done
echo 'verified loader architecture relocation formulas and machine flags'
