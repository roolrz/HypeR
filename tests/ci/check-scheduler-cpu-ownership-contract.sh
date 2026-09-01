#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Protect linear running-schedule ownership and the lock-independent tick path.
set -eu

root=${HYPER_SCHEDULER_CPU_OWNERSHIP_ROOT:-$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)}
cd "$root"

thread=src/kernel/task/thread.rs
state=src/kernel/task/scheduler/state.rs
registry=src/kernel/task/scheduler/registry.rs
queue=src/kernel/task/scheduler/queue.rs
scheduler=src/kernel/task/scheduler/mod.rs

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

require 'enum ScheduleOwner \{[\s\S]*Coordinator,[\s\S]*Cpu\(CpuIndex\)' \
    "$thread" 'Thread scheduling state must distinguish coordinator and CPU-domain residence'
require 'schedule: UnsafeCell<ThreadScheduleState>' \
    "$thread" 'schedule storage must remain stable across ready/current transitions'
require 'static CPU_SCHEDULERS: PerCpu<CpuSchedulerLock>' \
    "$state" 'running schedules must reside in static per-CPU locks'
require 'struct Scheduler \{[\s\S]*schedulable_cpus: CpuMask' \
    "$state" 'cross-CPU admission must have one TransitionLock-owned truth'
require 'struct CpuScheduler \{[\s\S]*current: ThreadId,[\s\S]*idle: Option<ThreadId>,[\s\S]*run_queue: CpuRunQueue,[\s\S]*switching_from: Option<SwitchingContext>[\s\S]*context_switches: u64' \
    "$state" 'per-CPU locks must own all runnable-domain state'
reject '(cpus: Vec<CpuQueueState>|cpu_slots: PerCpu)' \
    "$state" 'Scheduler must not retain a second copy of per-CPU runtime truth'
require 'struct LocalReadyQueueAuthority[^\{]*\{[\s\S]{0,120}cpu: CpuThreadTableAuthority' \
    "$queue" 'local scheduling must use a pure matching-CPU ready authority'
reject 'struct LocalReadyQueueAuthority[^\{]*\{[\s\S]{0,220}(coordinator|control)' \
    "$queue" 'local ready authority must not carry coordinator or control capability'
require 'enum LocalScheduleAttempt \{[\s\S]*NeedsCoordinator' \
    "$state" 'local scheduling must return a typed coordinator fallback'
require 'pub\(super\) fn prepare_local_yield[\s\S]{0,500}CPU_SCHEDULERS\[cpu\]\.with' \
    "$state" 'ordinary yield must begin under only the current CPU lock'
require 'pub\(super\) fn prepare_local_preemption[\s\S]{0,500}CPU_SCHEDULERS\[cpu\]\.with' \
    "$state" 'IRQ-tail preemption must begin under only the current CPU lock'
require 'enum SwitchDisposition[\s\S]*Local,[\s\S]*Coordinated' \
    "$state" 'switch tail must distinguish local and coordinated ownership'
require 'struct SwitchingContext \{[\s\S]*generation: u64,[\s\S]*disposition: SwitchDisposition' \
    "$state" 'switch completion must retain an ABA-resistant generation and disposition'
require 'fn finish_context_switch_tail\(ticket: usize\)[\s\S]*complete_local_switch_tail\(cpu, ticket\)[\s\S]*NeedsCoordinator => SCHEDULER\.with' \
    "$scheduler" 'switch tail must release the CPU lock before coordinator fallback'
require 'pub\(super\) fn complete_local_switch_tail[\s\S]*switching\.generation != ticket[\s\S]*switching\.disposition' \
    "$state" 'local switch tail must validate its exact generation before completion'
require 'switch_thread_context\([\s\S]*finish_context_switch_tail,[\s\S]*self\.ticket as usize' \
    "$state" 'the architecture boundary must carry the exact switch generation'
reject 'fn prepare_schedule[\s\S]{0,180}reap_terminated_threads' \
    "$scheduler" 'ordinary local yield must not acquire coordinator maintenance state'
require 'fn cond_resched_inner[\s\S]*prepare_local_preemption\(cpu\)[\s\S]*NeedsCoordinator => SCHEDULER\.with' \
    "$scheduler" 'preemption must use local fast path before coordinator fallback'
require 'fn with_cpu_schedule_stored[\s\S]*with_cpu_domain\(cpu,[\s\S]*Thread::schedule_owner_cpu[\s\S]*Ok\(Some\(cpu\)\)' \
    "$state" 'coordinator access must lock and revalidate the exact CPU owner'
require 'fn cpu_lock_required_for[^}]*\(Some\(cpu\), Some\(active\)\) if cpu == active\.cpu => Ok\(None\)' \
    "$state" 'matching active CPU ownership must enter the operation body without recursion'
reject 'if[^\n]*schedule_owner_cpu\([^\n]*\n([^\n]*\n){0,4}[^\n]*with_cpu_schedule_stored' \
    "$state" 'CPU-owned entry routing must use cpu_lock_required_for instead of recursive observation checks'
require 'let local = core::ptr::NonNull::from\(&mut \*local\);[\s\S]*operation\(self, unsafe \{ &mut \*local\.as_ptr\(\) \}\)' \
    "$state" 'top-level and nested CPU-domain borrows must derive from one raw provenance'
require 'pub fn notify_one_with[\s\S]*cpu_lock_required_for\(id\)\?[\s\S]*thread\(id\)\?[\s\S]*wait_record\(\)' \
    "$state" 'wait-queue consumers must route a CPU-owned head before reading its schedule'
require 'pub fn queue_terminated_retirement[\s\S]*schedule_owner_cpu[\s\S]*QueueMembership::Terminated[\s\S]*registry\.begin_retirement' \
    "$state" 'termination must stage exact identity-directed retirement'
require 'request_user_stop\(id, reason\)[\s\S]*queue_terminated_retirement\(id\)[\s\S]*request_retirement_worker' \
    "$scheduler" 'dormant user termination must publish durable reaper work in its originating transaction'
require 'complete_incoming_switch\(cpu, ticket\)[\s\S]*completion[\s\S]{0,60}\.retirement_published[\s\S]*request_retirement_worker' \
    "$scheduler" 'coordinated exit tail must publish only bounded dedicated-reaper work'
require 'enum ThreadSlot[\s\S]*Retiring \{[\s\S]*thread: Option<Box<Thread>>' \
    "$registry" 'retiring slots must retain detached ownership until the reaper takes it'
require 'pub fn complete_retirement[\s\S]*thread: None[\s\S]*ThreadSlot::Vacant' \
    "$registry" 'a slot must remain unavailable until lock-external retirement completes'
require 'pub fn complete_retirement[\s\S]*if slot != 0 \{[\s\S]*preflight_release\(slot\)[\s\S]*ThreadSlot::Vacant[\s\S]*if slot != 0 \{[\s\S]*commit_release\(slot\)' \
    "$registry" 'bootstrap retirement must complete without making slot zero reusable'
reject 'reap_terminated_threads' "$scheduler" \
    'local yield and idle paths must not perform global retirement scans'
require 'pub\(super\) fn local_current_vcpu[\s\S]*CPU_SCHEDULERS\[cpu\]\.with' \
    "$state" 'current vCPU observation must use one local CPU authority snapshot'
require 'pub\(crate\) fn current_vcpu_if_present[\s\S]*state::local_current_vcpu\(cpu\)' \
    "$scheduler" 'IRQ-tail vCPU queries must bypass the global scheduler lock'
require 'pub fn running_vcpu_cpu[\s\S]*ThreadState::Running if current == id => Ok\(Some\(cpu\)\)[\s\S]*ThreadState::Ready[\s\S]*=> Ok\(None\)' \
    "$state" 'ready vCPUs must not be reported as currently executing'
require 'pub fn request_user_stop[\s\S]*self\.with_thread\(id,[\s\S]*request_stop\(reason\)[\s\S]*let \(state, cpu, ticket\)[\s\S]*thread\.wait_record\(\)[\s\S]*self\.resolve_wait\(ticket,[\s\S]*match state' \
    "$state" 'user stop must snapshot schedule decisions before wait resolution transfers ownership'
reject 'self\.resolve_wait\(ticket,[\s\S]{0,240}self\.thread\(id\)\?\.(state|cpu_index|wait_record)' \
    "$state" 'user stop must not re-read stored schedule after queued wait resolution publishes CPU ownership'
reject 'Ok::<_, Error>\(\(thread\.(cpu_index|wait_record)\(' \
    "$state" 'resource access must not reborrow CPU-owned stored schedule state'
reject 'FAIR_READY' "$state" \
    'Fair-ready state must not be duplicated in an atomic mirror'
require 'pub\(super\) fn account_tick[\s\S]*CPU_SCHEDULERS\[cpu\]\.with[\s\S]*local\.run_queue\.has_fair_threads\(\)' \
    "$state" 'tick accounting must read ready topology under the CPU-local scheduler lock'
require 'fn cpu_is_schedulable[^\{]*\{[^}]*self\.schedulable_cpus\.contains\(cpu\)' \
    "$state" 'placement admission must not acquire a target CPU lock'
reject 'fn cpu_is_schedulable[^\{]*\{[^}]*CPU_SCHEDULERS' \
    "$state" 'cross-CPU migration must not nest a target CPU lock for admission'
require 'fn install_current_as_idle[\s\S]*self\.schedulable_cpus = self\.schedulable_cpus\.with_cpu\(cpu\)' \
    "$state" 'idle installation must publish scheduler admission under TransitionLock'
require 'pub fn take[\s\S]*!thread\.schedule_is_coordinator_owned\(\)[\s\S]*InvalidThreadState' \
    "$registry" 'registry removal must reject ready- or current-CPU-owned scheduling state'
reject '(FallibleArc|Arc<|\*mut Thread|\*const Thread)' \
    "$state" 'CPU scheduler ownership must not use shared or raw Thread handles'
