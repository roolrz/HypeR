#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Protect stable Thread storage and closure-bounded intrusive-queue authority.
set -eu

root=${HYPER_THREAD_TABLE_ROOT:-$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)}
cd "$root"

registry=src/kernel/task/scheduler/registry.rs
queue=src/kernel/task/scheduler/queue.rs
state=src/kernel/task/scheduler/state.rs
scheduler=src/kernel/task/scheduler/mod.rs
thread=src/kernel/task/thread.rs

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

require 'struct ThreadSlotCell\(UnsafeCell<ThreadSlot>\)' "$registry" \
    'the fixed Thread table must use address-stable private slot cells'
require 'enum ThreadTableStorage \{[\s\S]*Staged\(Box<ThreadTable>\)[\s\S]*Published\(ThreadTableCapability\)' \
    "$registry" 'Thread table publication must retain a fallible staged owner'
require 'pub fn publish_table[\s\S]*Box::leak\(table\)[\s\S]*ThreadTableStorage::Published' \
    "$registry" 'the final scheduler commit must publish permanent table backing'
require 'claim_schedule\(cpu\)[\s\S]*self\.registry\.publish_table\(\)' "$state" \
    'table publication must follow successful boot-current ownership claim'
require "impl for<'thread> FnOnce\(&'thread Thread\) -> R" "$registry" \
    'read authority must closure-bound every Thread borrow'
require "impl for<'thread> FnOnce\(&'thread mut Thread\) -> R" "$registry" \
    'write authority must closure-bound every mutable Thread borrow'
reject 'pub fn thread(_mut)?[^\n]*->[^\n]*&(mut )?Thread' "$registry" \
    'registry APIs must not expose Thread references'
reject '(Arc<Thread|FallibleArc<Thread|\*mut Thread|\*const Thread)' "$registry" \
    'stable storage must not use shared or raw Thread handles'
require 'ThreadTableWriteAuthority' "$queue" \
    'ready queue mutation must require typed table authority'
require 'control_queue_links: UnsafeCell<QueueLinks>' "$thread" \
    'global control queue links must be independent from CPU-owned schedule state'
require 'struct ThreadControlAuthority' "$registry" \
    'global control links must have a dedicated registry authority'
require 'struct ControlQueueAuthority' "$queue" \
    'waiting and terminated queues must use a dedicated control authority'
require 'pub\(super\) fn control_push[\s\S]*pub\(super\) fn control_pop[\s\S]*pub\(super\) fn control_remove' \
    "$queue" 'control queues must expose a domain-specific API'
require 'fn queue_push[\s\S]*ControlQueueAuthority::new\(registry\.control_authority\(\)\)[\s\S]*queue::control_push' \
    "$state" 'waiting and terminated insertion must use registry control authority'
require 'fn queue_pop[\s\S]*queue::control_pop[\s\S]*fn queue_remove[\s\S]*queue::control_remove' \
    "$state" 'waiting and terminated removal must remain in the control domain'
require 'preflight_insert\([\s\S]*commit_insert\(' "$queue" \
    'queue insertion must separate fallible preflight from commit'
require 'validate_neighbors\([\s\S]*preflight_residence\([\s\S]*update_previous_link\(' "$queue" \
    'queue removal must validate topology and residence before mutation'
require 'checked_add\(1\)' "$queue" 'queue insertion must preflight counter overflow'
require 'checked_sub\(1\)' "$queue" 'queue removal must preflight counter underflow'
require 'publish_ready_ownership' "$thread" \
    'ready insertion must transfer schedule ownership to the target CPU queue'
require 'fn commit_remove[\s\S]*schedule\.ready_queue_links = QueueLinks::EMPTY' "$queue" \
    'ready removal must retain CPU residence while clearing queue membership'
require 'impl ThreadQueueAuthority for ControlQueueAuthority[\s\S]*with_links_mut' \
    "$queue" 'control topology mutation must never borrow a CPU-owned schedule'
reject 'ControlQueueAuthority[\s\S]{0,1200}with_cpu_schedule' "$queue" \
    'control queue authority must remain independent of CPU scheduling domains'
require 'pub const fn real_time_len[\s\S]*pub const fn fair_len' "$queue" \
    'scheduler statistics need exact per-class ready counts'
require 'thread\.schedule_owner_cpu\(\) != Some\(cpu\)[\s\S]*ThreadState::Ready =>[\s\S]*stats\.ready \+= 1[\s\S]*topology_real_time_ready[\s\S]*topology_fair_ready' \
    "$state" 'aggregate statistics must include every CPU-owned ready entity'
require 'struct SwitchingContext' "$state" \
    'switch-tail context ownership must remain explicit beside schedule residence'
reject 'stack_statistics: thread\.kernel_stack_statistics\(\)' "$state" \
    'generic Thread observation must not scan possibly live stack memory'
require 'pub fn thread_stack_statistics[\s\S]*context_is_stopped\(id\)\?[\s\S]*scheduler\.with_thread\(id, Thread::kernel_stack_statistics\)' \
    "$scheduler" 'stack watermark scans must follow an explicit stopped-context proof'
require 'pub\(crate\) fn crash_snapshot[\s\S]*state::try_cpu_snapshot\(cpu\)' "$scheduler" \
    'crash snapshots must delegate to one non-blocking CPU-domain observation'
require 'pub\(super\) fn try_cpu_snapshot[\s\S]*CPU_SCHEDULERS\[cpu\][\s\S]*\.try_with[\s\S]*stack_statistics: None' \
    "$state" 'crash snapshots must use one try-lock and never scan the current live stack'
