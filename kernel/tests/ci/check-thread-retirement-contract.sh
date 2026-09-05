#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Protect scheduler-detached resource retirement and quiescence observation.
set -eu

root=${HYPER_THREAD_RETIREMENT_ROOT:-$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)}
cd "$root"

scheduler=src/kernel/task/scheduler/mod.rs
state=src/kernel/task/scheduler/state.rs
registry=src/kernel/task/scheduler/registry.rs
reaper=src/kernel/reaper.rs
support=tests/kernel/support.rs
progress=src/kernel/task/test_progress.rs

require() {
    pattern=$1
    file=$2
    message=$3
    rg -q -U "$pattern" "$file" || {
        echo "$message" >&2
        exit 1
    }
}

reject() {
    pattern=$1
    file=$2
    message=$3
    if rg -q -U "$pattern" "$file"; then
        echo "$message" >&2
        exit 1
    fi
}

line_first() {
    rg -n "$2" "$1" | sed -n '1s/:.*//p'
}

require_order() {
    first=$(line_first "$1" "$2")
    second=$(line_first "$1" "$3")
    if [ -z "$first" ] || [ -z "$second" ] || [ "$first" -ge "$second" ]; then
        echo "$4" >&2
        exit 1
    fi
}

statistics=$(mktemp "${TMPDIR:-/tmp}/hyper-retirement-statistics.XXXXXX")
direct_stop=$(mktemp "${TMPDIR:-/tmp}/hyper-retirement-direct-stop.XXXXXX")
switch_tail=$(mktemp "${TMPDIR:-/tmp}/hyper-retirement-switch-tail.XXXXXX")
worker=$(mktemp "${TMPDIR:-/tmp}/hyper-retirement-worker.XXXXXX")
trap 'rm -f "$statistics" "$direct_stop" "$switch_tail" "$worker"' EXIT HUP INT TERM
sed -n '/^pub fn statistics(/,/^}/p' "$scheduler" >"$statistics"
sed -n '/^pub(in crate::kernel) fn request_user_thread_stop(/,/^fn prepare_boxed_thread(/p' \
    "$scheduler" >"$direct_stop"
sed -n '/^extern "C" fn finish_context_switch_tail(/,/^}/p' \
    "$scheduler" >"$switch_tail"
sed -n '/^pub(crate) fn reap_one_thread(/,/^fn retire_detached_thread(/p' \
    "$scheduler" >"$worker"

require 'SCHEDULER\.with[\s\S]*queue_terminated_retirement\(id\)\?[\s\S]*crate::kernel::reaper::request\(\)' \
    "$direct_stop" 'direct stop must stage ownership before publishing durable reaper work'
require 'complete_incoming_switch\(cpu, ticket\)\?[\s\S]*retirement_published[\s\S]*crate::kernel::reaper::request\(\)' \
    "$switch_tail" 'switch tail must stage ownership before publishing durable reaper work'
reject '(retire_detached_thread|drop\(thread\)|complete_detach|complete_vcpu_reap)' \
    "$switch_tail" 'switch-tail callbacks must not run blocking resource teardown'
require 'take_retirement\(\)[\s\S]*retire_detached_thread\(thread\)[\s\S]*complete_retirement\(id\)[\s\S]*drop\(retirement\)' \
    "$worker" 'the dedicated worker must destroy resources before releasing slot and epoch ownership'
require 'begin_retirement\(id\)[\s\S]*ResourceRetirement::begin\(\)[\s\S]*retirements\.push' \
    "$state" 'detachment, retirement epoch, and queue publication must share the transition lock'
require 'Retiring \{[\s\S]*thread: Some\(thread\)[\s\S]*fn take_retiring[\s\S]*thread\.take\(\)[\s\S]*fn complete_retirement[\s\S]*thread: None[\s\S]*ThreadSlot::Vacant' \
    "$registry" 'registry slots must remain Retiring across lock-external destruction'
require 'fn vcpu_reaped[\s\S]*thread_registry_status\(thread\)[\s\S]*ThreadRegistryStatus::Retiring\([\s\S]*ExecutionKind::Vcpu[\s\S]*Ok\(false\)[\s\S]*ThreadRegistryStatus::Absent => Ok\(true\)' \
    "$scheduler" 'vCPU reaping must remain false throughout the complete Retiring phase'
reject 'reap_terminated_threads' "$scheduler" \
    'targeted retirement must not regress to a scheduler-wide hot-path scan'
reject 'hyper::log::|DeferredDrain|DrainDisposition' "$scheduler" \
    'scheduler retirement must depend on neutral deferred-work synchronization'
require 'DeferredWork[\s\S]*WorkDisposition' "$reaper" \
    'the central reaper must use the shared neutral deferred-work protocol'
require 'fetch_update\(Ordering::Relaxed, Ordering::Relaxed,[\s\S]*count < THREAD_CAPACITY' \
    "$scheduler" 'retirement admission must reject counter overflow'
require 'fetch_update\(Ordering::Release, Ordering::Relaxed,[\s\S]*count\.checked_sub\(1\)' \
    "$scheduler" 'retirement completion must release-publish and reject underflow'
require_order "$statistics" 'SCHEDULER\.with' \
    'RETIREMENTS_IN_PROGRESS\.load\(Ordering::Acquire\)' \
    'scheduler population must be observed before detached retirement completion'
require 'wait_for_test_progress[\s\S]*statistics\.retirements_in_progress == 0' \
    "$support" 'kernel-test quiescence must include detached retirement in its timed progress predicate'
require 'deadline_after\(timeout_nanoseconds\)[\s\S]*sleep_ms\(1\)[\s\S]*deadline_reached' \
    "$progress" 'kernel-test progress waits must block remote-owner observers under a monotonic deadline'
