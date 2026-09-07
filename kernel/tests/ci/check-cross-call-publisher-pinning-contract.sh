#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Keep the complete Kernel RPC ownership transaction on one CPU. A reserved
# user-space transaction can publish two RPCs, so pinning only execute_owned is
# insufficient.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
cross_call=${HYPER_CROSS_CALL_PINNING_SOURCE:-$root/src/kernel/irq/cross_call.rs}
scheduler=${HYPER_CROSS_CALL_PINNING_SCHEDULER:-$root/src/kernel/task/scheduler/mod.rs}

owner=$(sed -n '/^impl Owner {/,/^}/p' "$cross_call")
drop_owner=$(sed -n '/^impl Drop for Owner {/,/^}/p' "$cross_call")
body=$(sed -n '/^fn execute_owned(/,/^fn next_generation()/p' "$cross_call")
if [ -z "$owner" ] || [ -z "$drop_owner" ] || [ -z "$body" ]; then
    echo "execute_owned must remain available for publisher pinning checks" >&2
    exit 1
fi

line_of() {
    section=$1
    pattern=$2
    printf '%s\n' "$section" | LC_ALL=C rg -n -m1 "$pattern" | cut -d: -f1 || true
}

require_order() {
    section=$1
    first_pattern=$2
    second_pattern=$3
    message=$4
    first=$(line_of "$section" "$first_pattern")
    second=$(line_of "$section" "$second_pattern")
    if [ -z "$first" ] || [ -z "$second" ] || [ "$first" -ge "$second" ]; then
        echo "$message" >&2
        exit 1
    fi
}

require_order "$owner" 'scheduler::preempt_disable\(\)' 'OWNER\.compare_exchange\(' \
    'the owner must be pinned before claiming the mailbox'
if ! printf '%s\n' "$owner" | LC_ALL=C rg -q 'release_owner_pin\(pin\)'; then
    echo "failed mailbox acquisition must release its pin" >&2
    exit 1
fi
require_order "$body" 'PUBLISHED_GENERATION\.store\(generation, Ordering::Release\)' \
    'service_local_irq_mailbox\(\)' \
    'the generation must be published before local mailbox service'
require_order "$body" 'service_local_irq_mailbox\(\)' 'notify_remote_targets\(' \
    'local service must remain inside the pinned protocol interval'
require_order "$body" 'notify_remote_targets\(' 'await_acknowledgements\(' \
    'remote notification must remain inside the pinned protocol interval'
require_order "$body" 'await_acknowledgements\(' \
    'PUBLISHED_GENERATION\.store\(0, Ordering::Release\)' \
    'the generation must remain published until acknowledgements complete'
require_order "$drop_owner" 'PUBLISHED_GENERATION\.store\(0, Ordering::Release\)' \
    'OWNER\.store\(false, Ordering::Release\)' \
    'payload publication must be cleared before releasing ownership'
require_order "$drop_owner" 'OWNER\.store\(false, Ordering::Release\)' \
    'release_owner_pin\(pin\)' \
    'the owner pin must be released only after mailbox ownership'

if printf '%s\n%s\n%s\n' "$owner" "$drop_owner" "$body" | LC_ALL=C rg -q \
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
