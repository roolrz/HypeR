#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Prove that every machine context migration ratchet rejects its unsafe form.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/hyper-migration-context-test.XXXXXX")
trap 'rm -rf "$fixture"' EXIT HUP INT TERM

mkdir -p "$fixture/src/arch/aarch64" "$fixture/src/arch/riscv64" "$fixture/src/arch/x86_64"

restore_fixture() {
    cp "$root/src/arch/aarch64/context.S" "$fixture/src/arch/aarch64/context.S"
    cp "$root/src/arch/riscv64/context.S" "$fixture/src/arch/riscv64/context.S"
    cp "$root/src/arch/x86_64/context.S" "$fixture/src/arch/x86_64/context.S"
}

check() {
    HYPER_MIGRATION_CONTEXT_ROOT="$fixture" \
        sh "$root/tests/ci/check-thread-migration-context-contract.sh"
}

mutate() {
    description=$1
    source=$2
    expression=$3
    restore_fixture
    before=$(cksum "$fixture/$source")
    sed "$expression" "$fixture/$source" >"$fixture/mutated"
    mv "$fixture/mutated" "$fixture/$source"
    after=$(cksum "$fixture/$source")
    if [ "$before" = "$after" ]; then
        echo "mutation did not change $source: $description" >&2
        exit 1
    fi
    if check >/dev/null 2>&1; then
        echo "$description" >&2
        exit 1
    fi
}

restore_fixture
check

mutate 'AArch64 live DAIF snapshots must be rejected' \
    src/arch/aarch64/context.S \
    '/str[[:space:]]*x2, \[x0, #THREAD_CONTEXT_DAIF_OFFSET\]/i\
    mrs     x2, daif'
mutate 'AArch64 missing incoming completion must be rejected' \
    src/arch/aarch64/context.S \
    's/blr[[:space:]]*x3/nop/'
mutate 'AArch64 completion before source stack save must be rejected' \
    src/arch/aarch64/context.S \
    's/str[[:space:]]*x2, \[x0, #THREAD_CONTEXT_SP_OFFSET\]/nop/'
mutate 'AArch64 completion off the incoming stack must be rejected' \
    src/arch/aarch64/context.S \
    's/mov[[:space:]]*sp, x2/nop/'

mutate 'RISC-V live sstatus snapshots must be rejected' \
    src/arch/riscv64/context.S \
    '/sd[[:space:]]*t0, THREAD_CONTEXT_SIE_OFFSET(a0)/i\
    csrr t0, sstatus'
mutate 'RISC-V missing incoming completion must be rejected' \
    src/arch/riscv64/context.S \
    's/jalr[[:space:]]*a3/nop/'
mutate 'RISC-V unnormalized saved interrupt state must be rejected' \
    src/arch/riscv64/context.S \
    's/andi[[:space:]]*t0, a2, 2/mv t0, a2/'
mutate 'RISC-V normalized interrupt state clobber must be rejected' \
    src/arch/riscv64/context.S \
    '/andi[[:space:]]*t0, a2, 2/a\
    mv t0, zero'
mutate 'RISC-V missing disabled SIE restore must be rejected' \
    src/arch/riscv64/context.S \
    's/csrci[[:space:]]*sstatus, 2/nop/'
mutate 'RISC-V completion before source stack save must be rejected' \
    src/arch/riscv64/context.S \
    's/sd[[:space:]]*sp, THREAD_CONTEXT_SP_OFFSET(a0)/nop/'
mutate 'RISC-V completion off the incoming stack must be rejected' \
    src/arch/riscv64/context.S \
    's/ld[[:space:]]*sp, THREAD_CONTEXT_SP_OFFSET(s0)/nop/'

mutate 'x86-64 live RFLAGS snapshots must be rejected' \
    src/arch/x86_64/context.S \
    '/movq[[:space:]]*%rdx, THREAD_CONTEXT_IF_OFFSET(%rdi)/i\
    pushfq'
mutate 'x86-64 missing incoming completion must be rejected' \
    src/arch/x86_64/context.S \
    's/call[[:space:]]*\*%rcx/nop/'
mutate 'x86-64 unnormalized saved interrupt state must be rejected' \
    src/arch/x86_64/context.S \
    's/andq[[:space:]]*\$0x200, %rdx/nop/'
mutate 'x86-64 normalized interrupt state clobber must be rejected' \
    src/arch/x86_64/context.S \
    '/andq[[:space:]]*\$0x200, %rdx/a\
    xorq %rdx, %rdx'
mutate 'x86-64 missing disabled IF restore must be rejected' \
    src/arch/x86_64/context.S \
    's/^[[:space:]]*cli[[:space:]]*$/    nop/'
mutate 'x86-64 completion before source stack save must be rejected' \
    src/arch/x86_64/context.S \
    's/movq[[:space:]]*%rsp, THREAD_CONTEXT_RSP_OFFSET(%rdi)/nop/'
mutate 'x86-64 completion off the incoming stack must be rejected' \
    src/arch/x86_64/context.S \
    's/movq[[:space:]]*THREAD_CONTEXT_RSP_OFFSET(%r12), %rsp/nop/'
mutate 'x86-64 restored IF without timer preparation must be rejected' \
    src/arch/x86_64/context.S \
    's/call[[:space:]]*x86_64_prepare_context_interrupt_enable/nop/'
