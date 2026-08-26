#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Preserve the controller enable/disable commit point through IRQ policy.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
hal=${HYPER_IRQ_TRANSITION_HAL:-$root/src/hal/interrupt.rs}
gic=${HYPER_IRQ_TRANSITION_GIC:-$root/src/drivers/interrupt/gicv3.rs}
kernel=${HYPER_IRQ_TRANSITION_KERNEL:-$root/src/kernel/irq/interrupt.rs}
cross_call=${HYPER_IRQ_TRANSITION_CROSS_CALL:-$root/src/kernel/irq/cross_call.rs}

LC_ALL=C rg -q -U \
    'pub enum InterruptTransitionError<Error>\s*\{[^}]*NotApplied\(Error\),[^}]*AppliedOrUnknown\(Error\),' \
    "$hal" || {
    echo "HAL interrupt transitions must distinguish pre-write and ambiguous failures" >&2
    exit 1
}

LC_ALL=C rg -q -U \
    'write_u32\([^;]*GICD_ISENABLER[^;]*;[^}]*wait_for_write\([^)]*\)[^;]*map_err\(InterruptTransitionError::AppliedOrUnknown\)' \
    "$gic" || {
    echo "GIC enable completion failures must be classified after the command write" >&2
    exit 1
}

LC_ALL=C rg -q -U \
    'write_u32\([^;]*GICD_ICENABLER[^;]*;[^}]*wait_for_write\([^)]*\)[^;]*map_err\(InterruptTransitionError::AppliedOrUnknown\)' \
    "$gic" || {
    echo "GIC disable completion failures must be classified after the command write" >&2
    exit 1
}

LC_ALL=C rg -q -U \
    'fn resolve_transition[^}]*AppliedOrUnknown\(error\)[^}]*kernel::crash::fatal' \
    "$kernel" || {
    echo "ambiguous controller transitions must enter coordinated fail-stop" >&2
    exit 1
}

LC_ALL=C rg -q 'with_transition_state' "$kernel" || {
    echo "IRQ registry transitions must preserve ambiguity until the lock is released" >&2
    exit 1
}

resolve_body=$(sed -n '/^fn resolve_transition/,/^}$/p' "$kernel")
if printf '%s\n' "$resolve_body" | LC_ALL=C rg -q 'INTERRUPTS|with_state'; then
    echo "coordinated transition fail-stop must not run under the IRQ registry lock" >&2
    exit 1
fi

if ! LC_ALL=C rg -q 'APPLIED_OR_UNKNOWN' "$cross_call" ||
    ! LC_ALL=C rg -q 'ambiguous_cpu' "$cross_call"; then
    echo "replicated local transitions must preserve ambiguous remote completion" >&2
    exit 1
fi
