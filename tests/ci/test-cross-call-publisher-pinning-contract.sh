#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Prove that publisher migration-window regressions are rejected.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/hyper-cross-call-pinning-test.XXXXXX")
trap 'rm -rf "$fixture"' EXIT HUP INT TERM

cp "$root/src/kernel/irq/cross_call.rs" "$fixture/cross_call.rs"
cp "$root/src/kernel/task/scheduler/mod.rs" "$fixture/scheduler.rs"

check() {
    HYPER_CROSS_CALL_PINNING_SOURCE="$fixture/cross_call.rs" \
        HYPER_CROSS_CALL_PINNING_SCHEDULER="$fixture/scheduler.rs" \
        sh "$root/tests/ci/check-cross-call-publisher-pinning-contract.sh"
}

expect_rejection() {
    if check >/dev/null 2>&1; then
        echo "$1" >&2
        exit 1
    fi
}

check

sed 's/scheduler::preempt_disable()/scheduler::preempt_disable_removed()/' \
    "$fixture/cross_call.rs" >"$fixture/modified.rs"
mv "$fixture/modified.rs" "$fixture/cross_call.rs"
expect_rejection "removing the publisher pin must be rejected"

cp "$root/src/kernel/irq/cross_call.rs" "$fixture/cross_call.rs"
sed '/^    service_local_irq_mailbox();$/i\
    let _ = crate::kernel::task::scheduler::preempt_enable_and_reschedule(publisher_pin);' \
    "$fixture/cross_call.rs" >"$fixture/modified.rs"
mv "$fixture/modified.rs" "$fixture/cross_call.rs"
expect_rejection "releasing the publisher pin before local service must be rejected"

cp "$root/src/kernel/irq/cross_call.rs" "$fixture/cross_call.rs"
sed 's/preempt_enable_without_reschedule(publisher_pin)/drop(publisher_pin)/' \
    "$fixture/cross_call.rs" >"$fixture/modified.rs"
mv "$fixture/modified.rs" "$fixture/cross_call.rs"
expect_rejection "unchecked publisher pin release must be rejected"

cp "$root/src/kernel/irq/cross_call.rs" "$fixture/cross_call.rs"
sed 's/preempt_enable_without_reschedule(publisher_pin)/preempt_enable_and_reschedule(publisher_pin)/' \
    "$fixture/cross_call.rs" >"$fixture/modified.rs"
mv "$fixture/modified.rs" "$fixture/cross_call.rs"
expect_rejection "publisher completion must not schedule under an outer owner"

cp "$root/src/kernel/task/scheduler/mod.rs" "$fixture/scheduler.rs"
sed '/guard\.0\.release().map(|_| ()).map_err(Into::into)/a\
    let _ = cond_resched();' "$fixture/scheduler.rs" >"$fixture/modified.rs"
mv "$fixture/modified.rs" "$fixture/scheduler.rs"
expect_rejection "the checked release seam must remain scheduling-free"
