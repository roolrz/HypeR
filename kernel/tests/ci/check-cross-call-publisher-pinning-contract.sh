#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Keep the synchronous Kernel RPC publisher on one CPU until all exact target
# acknowledgements have been consumed.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
cross_call=${HYPER_CROSS_CALL_PINNING_SOURCE:-$root/src/kernel/irq/cross_call.rs}
scheduler=${HYPER_CROSS_CALL_PINNING_SCHEDULER:-$root/src/kernel/task/scheduler/mod.rs}

body=$(sed -n '/^fn execute_owned(/,/^fn next_generation()/p' "$cross_call")
if [ -z "$body" ]; then
    echo "execute_owned must remain available for publisher pinning checks" >&2
    exit 1
fi

line_of() {
    pattern=$1
    printf '%s\n' "$body" | LC_ALL=C rg -n -m1 "$pattern" | cut -d: -f1 || true
}

require_order() {
    first_pattern=$1
    second_pattern=$2
    message=$3
    first=$(line_of "$first_pattern")
    second=$(line_of "$second_pattern")
    if [ -z "$first" ] || [ -z "$second" ] || [ "$first" -ge "$second" ]; then
        echo "$message" >&2
        exit 1
    fi
}

require_order 'scheduler::preempt_disable\(\)' \
    'PUBLISHED_GENERATION\.store\(generation, Ordering::Release\)' \
    'the publisher must be pinned before publishing a generation'
require_order 'scheduler::preempt_disable\(\)' 'service_local_irq_mailbox\(\)' \
    'the publisher pin must span local mailbox service'
require_order 'service_local_irq_mailbox\(\)' 'notify_remote_targets\(' \
    'local service must remain inside the pinned protocol interval'
require_order 'notify_remote_targets\(' 'await_acknowledgements\(' \
    'remote notification must remain inside the pinned protocol interval'
require_order 'await_acknowledgements\(' \
    'PUBLISHED_GENERATION\.store\(0, Ordering::Release\)' \
    'the generation must remain published until acknowledgements complete'
require_order 'PUBLISHED_GENERATION\.store\(0, Ordering::Release\)' \
    'scheduler::preempt_enable_without_reschedule\(publisher_pin\)' \
    'publisher pin must be released only after unpublication'

if printf '%s\n' "$body" | LC_ALL=C rg -q \
    'InterruptMaskGuard|disable_local\(|local_enabled\(|preempt_enable_and_reschedule\('; then
    echo "the synchronous publisher must not mask local IRQs while waiting" >&2
    exit 1
fi

release_body=$(sed -n \
    '/^pub(crate) fn preempt_enable_without_reschedule(/,/^}/p' "$scheduler")
if [ -z "$release_body" ] ||
    ! printf '%s\n' "$release_body" | LC_ALL=C rg -q 'guard\.0\.release\(\)' ||
    printf '%s\n' "$release_body" | LC_ALL=C rg -q 'cond_resched|local_enabled|enable_local'; then
    echo "publisher pin release must be checked and compatible with nested or IRQ-masked callers" >&2
    exit 1
fi
