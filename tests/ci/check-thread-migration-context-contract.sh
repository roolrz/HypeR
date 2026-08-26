#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Protect the machine context handoff required by cross-CPU Thread migration.
set -eu

root=${HYPER_MIGRATION_CONTEXT_ROOT:-$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)}
cd "$root"

function_body() {
    source=$1
    symbol=$2
    sed -n "/^$symbol:/,/^[[:space:]]*\.size[[:space:]]*$symbol/p" "$source"
}

line_matching() {
    content=$1
    pattern=$2
    printf '%s\n' "$content" | awk -v pattern="$pattern" '$0 ~ pattern { print NR; exit }'
}

require_ordered_handoff() {
    source=$1
    symbol=$2
    saved_stack=$3
    normalized_state=$4
    saved_interrupt_state=$5
    incoming_stack_load=$6
    incoming_stack_install=$7
    completion=$8
    irq_restore=$9
    forbidden_live_read=${10}
    architecture=${11}

    body=$(function_body "$source" "$symbol")
    stack_line=$(line_matching "$body" "$saved_stack")
    normalized_line=$(line_matching "$body" "$normalized_state")
    state_line=$(line_matching "$body" "$saved_interrupt_state")
    incoming_load_line=$(line_matching "$body" "$incoming_stack_load")
    incoming_install_line=$(line_matching "$body" "$incoming_stack_install")
    completion_line=$(line_matching "$body" "$completion")
    restore_line=$(line_matching "$body" "$irq_restore")

    if [ -z "$body" ] || [ -z "$stack_line" ] || [ -z "$normalized_line" ] ||
        [ -z "$state_line" ] ||
        [ -z "$incoming_load_line" ] || [ -z "$incoming_install_line" ] ||
        [ -z "$completion_line" ] || [ -z "$restore_line" ]; then
        echo "$architecture context switch lacks the migration handoff contract" >&2
        exit 1
    fi
    if [ "$stack_line" -ge "$incoming_load_line" ] ||
        [ "$normalized_line" -gt "$state_line" ] ||
        { [ "$normalized_line" -ne "$state_line" ] &&
            [ "$state_line" -ne $((normalized_line + 1)) ]; } ||
        [ "$state_line" -ge "$incoming_load_line" ] ||
        [ "$incoming_load_line" -gt "$incoming_install_line" ] ||
        [ "$incoming_install_line" -ge "$completion_line" ] ||
        [ "$completion_line" -ge "$restore_line" ]; then
        echo "$architecture must save supplied IRQ state, complete on the incoming stack, then restore IRQs" >&2
        exit 1
    fi
    if printf '%s\n' "$body" | awk -v pattern="$forbidden_live_read" '$0 ~ pattern { found = 1 } END { exit !found }'; then
        echo "$architecture context switch must not snapshot the already-masked live IRQ state" >&2
        exit 1
    fi
}

require_ordered_handoff \
    src/arch/aarch64/context.S \
    aarch64_switch_context \
    'str[[:space:]]+x2,.*THREAD_CONTEXT_SP_OFFSET' \
    'str[[:space:]]+x2,.*THREAD_CONTEXT_DAIF_OFFSET' \
    'str[[:space:]]+x2,.*THREAD_CONTEXT_DAIF_OFFSET' \
    'ldr[[:space:]]+x2,.*THREAD_CONTEXT_SP_OFFSET' \
    'mov[[:space:]]+sp,[[:space:]]*x2' \
    'blr[[:space:]]+x3' \
    'msr[[:space:]]+daif,[[:space:]]*x2' \
    'mrs[[:space:]]+[^,]+,[[:space:]]*daif' \
    AArch64

require_ordered_handoff \
    src/arch/riscv64/context.S \
    riscv64_switch_context \
    'sd[[:space:]]+sp,.*THREAD_CONTEXT_SP_OFFSET' \
    'andi[[:space:]]+t0,[[:space:]]*a2,[[:space:]]*2' \
    'sd[[:space:]]+t0,.*THREAD_CONTEXT_SIE_OFFSET' \
    'ld[[:space:]]+sp,.*THREAD_CONTEXT_SP_OFFSET' \
    'ld[[:space:]]+sp,.*THREAD_CONTEXT_SP_OFFSET' \
    'jalr[[:space:]]+a3' \
    'csr(si|ci)[[:space:]]+sstatus' \
    'csrr[[:space:]]+[^,]+,[[:space:]]*sstatus' \
    RISC-V

riscv_body=$(function_body src/arch/riscv64/context.S riscv64_switch_context)
if ! printf '%s\n' "$riscv_body" | rg -q '^[[:space:]]*csrsi[[:space:]]+sstatus' ||
    ! printf '%s\n' "$riscv_body" | rg -q '^[[:space:]]*csrci[[:space:]]+sstatus'; then
    echo 'RISC-V must restore both enabled and disabled incoming SIE states' >&2
    exit 1
fi

require_ordered_handoff \
    src/arch/x86_64/context.S \
    x86_64_switch_context \
    'movq[[:space:]]+%rsp,.*THREAD_CONTEXT_RSP_OFFSET' \
    'andq[[:space:]]+[$]0x200,[[:space:]]*%rdx' \
    'movq[[:space:]]+%rdx,.*THREAD_CONTEXT_IF_OFFSET' \
    'movq[[:space:]]+THREAD_CONTEXT_RSP_OFFSET.*,[[:space:]]*%rsp' \
    'movq[[:space:]]+THREAD_CONTEXT_RSP_OFFSET.*,[[:space:]]*%rsp' \
    'call[[:space:]]+[*]%rcx' \
    '^[[:space:]]*(sti|cli)[[:space:]]*$' \
    '^[[:space:]]*pushfq[[:space:]]*$' \
    x86-64


x86_body=$(function_body src/arch/x86_64/context.S x86_64_switch_context)
if ! printf '%s\n' "$x86_body" | rg -q '^[[:space:]]*sti[[:space:]]*$' ||
    ! printf '%s\n' "$x86_body" | rg -q '^[[:space:]]*cli[[:space:]]*$'; then
    echo 'x86-64 must restore both enabled and disabled incoming IF states' >&2
    exit 1
fi

x86_completion=$(line_matching "$x86_body" 'call[[:space:]]+[*]%rcx')
x86_timer=$(line_matching "$x86_body" 'call[[:space:]]+x86_64_prepare_context_interrupt_enable')
x86_enable=$(line_matching "$x86_body" '^[[:space:]]*sti[[:space:]]*$')
if [ -z "$x86_timer" ] || [ "$x86_completion" -ge "$x86_timer" ] ||
    [ "$x86_timer" -ge "$x86_enable" ]; then
    echo 'x86-64 must prepare a deferred timer after switch completion and before restoring IF' >&2
    exit 1
fi
