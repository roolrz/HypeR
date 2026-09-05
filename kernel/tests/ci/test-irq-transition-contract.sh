#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Prove that IRQ transition commit-point regressions are rejected.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/hyper-irq-transition-test.XXXXXX")
trap 'rm -rf "$fixture"' EXIT HUP INT TERM

cp "$root/src/hal/interrupt.rs" "$fixture/hal.rs"
cp "$root/src/drivers/interrupt/gicv3.rs" "$fixture/gic.rs"
cp "$root/src/kernel/irq/interrupt.rs" "$fixture/kernel.rs"
cp "$root/src/kernel/irq/cross_call.rs" "$fixture/cross_call.rs"

check() {
    HYPER_IRQ_TRANSITION_HAL="$fixture/hal.rs" \
        HYPER_IRQ_TRANSITION_GIC="$fixture/gic.rs" \
        HYPER_IRQ_TRANSITION_KERNEL="$fixture/kernel.rs" \
        HYPER_IRQ_TRANSITION_CROSS_CALL="$fixture/cross_call.rs" \
        sh "$root/tests/ci/check-irq-transition-contract.sh"
}

expect_rejection() {
    if check >/dev/null 2>&1; then
        echo "$1" >&2
        exit 1
    fi
}

check

sed 's/AppliedOrUnknown(Error)/NotAppliedAgain(Error)/' \
    "$fixture/hal.rs" >"$fixture/modified.rs"
mv "$fixture/modified.rs" "$fixture/hal.rs"
expect_rejection "HAL transition phase collapse must be rejected"

cp "$root/src/hal/interrupt.rs" "$fixture/hal.rs"
sed 's/InterruptTransitionError::AppliedOrUnknown/InterruptTransitionError::NotApplied/' \
    "$fixture/gic.rs" >"$fixture/modified.rs"
mv "$fixture/modified.rs" "$fixture/gic.rs"
expect_rejection "post-write GIC timeout classified as pre-write must be rejected"

cp "$root/src/drivers/interrupt/gicv3.rs" "$fixture/gic.rs"
sed '/^[[:space:]]*fn disable(/,/^[[:space:]]*fn acknowledge(/ s/InterruptTransitionError::AppliedOrUnknown/InterruptTransitionError::NotApplied/' \
    "$fixture/gic.rs" >"$fixture/modified.rs"
mv "$fixture/modified.rs" "$fixture/gic.rs"
expect_rejection "post-write GIC disable timeout classified as pre-write must be rejected"

cp "$root/src/drivers/interrupt/gicv3.rs" "$fixture/gic.rs"
sed 's/APPLIED_OR_UNKNOWN/APPLIED_UNKNOWN_REMOVED/g' \
    "$fixture/cross_call.rs" >"$fixture/modified.rs"
mv "$fixture/modified.rs" "$fixture/cross_call.rs"
expect_rejection "remote ambiguity loss must be rejected"
