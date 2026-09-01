#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Protect machine-resource addresses from outer scheduler reborrows.
set -eu

root=${HYPER_THREAD_RESOURCE_ALIAS_ROOT:-$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)}
cd "$root"

thread=src/kernel/task/thread.rs
state=src/kernel/task/scheduler/state.rs
scheduler=src/kernel/task/scheduler/mod.rs
user_entry=src/kernel/entry/user.rs

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

require 'struct Thread \{[\s\S]*resources: Box<ThreadResources>' \
    "$thread" 'Thread resources must occupy a private stable allocation'
require 'struct ThreadResources \{[\s\S]*context: UnsafeCell<crate::hal::context::ThreadContext>' \
    "$thread" 'assembly-owned ThreadContext must be reached through UnsafeCell'
require 'Vcpu\(Box<UnsafeCell<VcpuExecution>>\)' \
    "$thread" 'repeated current-vCPU queries must not mint overlapping mutable references'
require 'User\(Box<UnsafeCell<crate::kernel::process::UserExecution>>\)' \
    "$thread" 'repeated current-user queries must not retag the stable payload'
require 'fn allocate_resources[\s\S]*try_box\(ThreadResources' \
    "$thread" 'resource allocation must be explicit and fallible'
require 'fn allocation_size\(\) -> usize \{[\s\S]*size_of::<Self>\(\)[\s\S]*size_of::<ThreadResources>\(\)' \
    "$thread" 'resource accounting must include the private allocation'
require 'fn context_pointer\(&self\)[\s\S]*self\.resources\.context\.get\(\)' \
    "$thread" 'context pointers must come from the dedicated cell without a Rust reference'
require 'fn vcpu_execution_pointer\(&self\)[\s\S]*ThreadExecution::Vcpu\(execution\) => Some\(execution\.get\(\)\)' \
    "$thread" 'vCPU pointers must come from the payload cell without an exclusive borrow'
require 'fn user_execution_pointer[\s\S]*NonNull::new\(execution\.get\(\)\)' \
    "$thread" 'user payload pointers must come from the payload cell without an exclusive borrow'
require 'struct CurrentUser \{[\s\S]*execution: NonNull<crate::kernel::process::UserExecution>' \
    "$scheduler" 'current-user capability must retain only the cell-derived stable pointer'
require 'pub fn current_vcpu[\s\S]*vcpu_execution_pointer\(\)' \
    "$state" 'current-vCPU lookup must not derive a long-lived pointer from &mut'
require 'pub fn current_user[\s\S]*user_execution_pointer\(\)' \
    "$state" 'current-user lookup must not derive a long-lived pointer from &mut'
require 'fn prepare_switch[\s\S]*context_pointer\(\)[\s\S]*context_pointer\(\)\.cast_const\(\)' \
    "$state" 'context switch pointers must use the stable cell address'
reject 'fn context_mut\(' "$thread" \
    'ThreadContext must not be exposed through a repeatable mutable reference'
reject 'fn (vcpu|user)_execution_mut\(' "$thread" \
    'execution payloads must not expose repeatable whole-object mutable borrows'
reject 'execution\.as_mut\(\)' "$user_entry" \
    'native-user entry must not turn its shared-derived owner pointer into &mut'
reject 'self\.resources[[:space:]]*=' "$thread" \
    'published Thread resource allocation must never be replaced'
reject 'mem::replace\(&mut self\.resources[[:space:]]*[,)]' "$thread" \
    'published Thread resource allocation must never be extracted by replacement'
