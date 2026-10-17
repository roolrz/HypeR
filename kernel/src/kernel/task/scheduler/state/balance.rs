// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Bounded Fair placement and Ready-only automatic migration.

use hyper::cpu::CpuIndex;

use super::super::balance_policy::{improves_balance, least_loaded};
use super::super::{Error, MigrationStatus};
use super::{CPU_SCHEDULERS, ReadyOutcome, Scheduler, scheduler_invariant};
use crate::kernel::task::policy::{CpuMask, PlacementPolicy, SchedulingPolicy};
use crate::kernel::task::thread::{ExecutionKind, Thread, ThreadId, ThreadState};

const SCAN_BUDGET: usize = 64;
const MOVE_BUDGET: usize = 4;

#[derive(Clone, Copy, Default)]
struct CpuLoad {
    runnable: u32,
    realtime: u32,
}

impl Scheduler {
    /// Initial placement is separate from affinity validation: the latter may
    /// already own a source CPU lock and must never acquire remote CPU locks.
    pub(super) fn select_initial_cpu(
        &self,
        preferred: CpuIndex,
        affinity: CpuMask,
    ) -> Result<CpuIndex, Error> {
        if affinity.is_empty() {
            return Err(Error::EmptyCpuAffinity);
        }
        let loads = self.balance_loads(None)?;
        let candidates = || {
            (0..hyper::cpu::MAX_CPUS)
                .filter_map(CpuIndex::new)
                .filter(|cpu| affinity.contains(*cpu) && self.cpu_is_schedulable(*cpu))
        };
        let avoid_realtime = candidates().any(|cpu| loads[cpu.get()].realtime == 0);
        least_loaded(
            preferred.get(),
            candidates()
                .filter(|cpu| !avoid_realtime || loads[cpu.get()].realtime == 0)
                .map(|cpu| (cpu.get(), loads[cpu.get()].runnable)),
        )
        .and_then(CpuIndex::new)
        .ok_or(Error::NoRegisteredCpuInAffinity)
    }

    /// Takes only one CPU lock at a time. Exclusive coordination excludes
    /// enqueue, wake, park and retirement; local switches conserve runnable
    /// counts, so the resulting loads remain valid throughout this pass.
    fn balance_loads(
        &self,
        excluded: Option<ThreadId>,
    ) -> Result<[CpuLoad; hyper::cpu::MAX_CPUS], Error> {
        if self.active_domain.is_some() {
            return Err(Error::InvalidThreadState);
        }
        let mut loads = [CpuLoad::default(); hyper::cpu::MAX_CPUS];
        for cpu in (0..hyper::cpu::MAX_CPUS).filter_map(CpuIndex::new) {
            if !self.cpu_is_schedulable(cpu) {
                continue;
            }
            loads[cpu.get()] = CPU_SCHEDULERS[cpu].with(|slot| {
                let local = slot.as_mut().ok_or(Error::CpuNotRegistered)?;
                let current = local.current;
                let (running, realtime) =
                    local
                        .thread_authority()
                        .with_thread(current, |_thread, schedule| match schedule.state {
                            ThreadState::Running => Ok((
                                excluded != Some(current),
                                matches!(schedule.scheduling, SchedulingPolicy::Fifo { .. }),
                            )),
                            ThreadState::Idle => Ok((false, false)),
                            _ => Err(Error::InvalidThreadState),
                        })??;
                Ok::<_, Error>(CpuLoad {
                    runnable: u32::try_from(local.run_queue.len())
                        .map_err(|_| Error::QueueCorrupted)?
                        + u32::from(running),
                    realtime: u32::try_from(local.run_queue.real_time_len())
                        .map_err(|_| Error::QueueCorrupted)?
                        + u32::from(running && realtime),
                })
            })?;
        }
        Ok(loads)
    }

    pub(in crate::kernel::task::scheduler) fn balance_ready(
        &mut self,
        excluded: ThreadId,
    ) -> Result<[Option<ReadyOutcome>; MOVE_BUDGET], Error> {
        let mut outcomes = [None; MOVE_BUDGET];
        let mut loads = self.balance_loads(Some(excluded))?;
        let mut moved = 0;
        let limit = self.registry.balance_scan_limit(SCAN_BUDGET);
        for _ in 0..limit {
            let Some(id) = self
                .registry
                .next_balance_candidate(&mut self.balance_cursor)
            else {
                continue;
            };
            let Some(source) = self.registry.with_thread(id, Thread::schedule_owner_cpu)? else {
                continue;
            };
            let outcome = self.with_cpu_schedule_stored(source, |scheduler| {
                let thread = scheduler.thread(id)?;
                if id == excluded
                    || thread.state() != ThreadState::Ready
                    || thread.scheduling_policy() != SchedulingPolicy::Fair
                    || thread.placement_policy() != PlacementPolicy::Movable
                    || thread.pending_migration().is_some()
                    || !matches!(
                        thread.execution_kind(),
                        ExecutionKind::Kernel | ExecutionKind::User | ExecutionKind::Vcpu
                    )
                    || scheduler
                        .switching_from(source)?
                        .is_some_and(|handoff| handoff.thread == id)
                {
                    return Ok(None);
                }
                let target = least_loaded(
                    source.get(),
                    (0..hyper::cpu::MAX_CPUS)
                        .filter_map(CpuIndex::new)
                        .filter(|cpu| {
                            *cpu != source
                                && scheduler.cpu_is_schedulable(*cpu)
                                && thread.can_run_on(*cpu)
                                && thread.wait_record().permits_assignment(*cpu)
                                && loads[cpu.get()].realtime == 0
                                && improves_balance(
                                    loads[source.get()].runnable,
                                    loads[cpu.get()].runnable,
                                )
                        })
                        .map(|cpu| (cpu.get(), loads[cpu.get()].runnable)),
                )
                .and_then(CpuIndex::new);
                let Some(target) = target else {
                    return Ok(None);
                };
                // Keep this source lock from the Ready/context-save checks
                // through removal. Existing migration defers target enqueue
                // until this lock is released; two CPU locks never nest.
                let migration = scheduler.migrate_thread(id, target)?;
                if migration.status != MigrationStatus::Completed
                    || migration.source_reschedule.is_some()
                {
                    scheduler_invariant(Error::InvalidThreadState);
                }
                migration
                    .target_ready
                    .ok_or(Error::InvalidThreadState)
                    .map(Some)
            })?;
            if let Some(outcome) = outcome {
                loads[source.get()].runnable -= 1;
                loads[outcome.target_cpu.get()].runnable += 1;
                outcomes[moved] = Some(outcome);
                moved += 1;
                if moved == MOVE_BUDGET {
                    break;
                }
            }
        }
        Ok(outcomes)
    }
}
