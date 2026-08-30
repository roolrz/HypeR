// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Shared lifecycle support for bare-metal kernel self-tests.

use crate::kernel::task::scheduler;

const MAX_QUIESCENCE_PASSES: usize = 4_096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct QuiescenceSnapshot {
    threads: usize,
    ready: usize,
    running: usize,
    blocked: usize,
    migrating: usize,
    idle: usize,
    idle_class_threads: usize,
    online_cpus: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum QuiescenceError {
    Scheduler(scheduler::Error),
    Timeout(QuiescenceSnapshot),
}

impl From<scheduler::Error> for QuiescenceError {
    fn from(error: scheduler::Error) -> Self {
        Self::Scheduler(error)
    }
}

/// Retires every transient test Thread and returns the stable boot population.
///
/// Worker completion publishes the worker's last shared-state access; it does
/// not prove that the entry point returned, the exit trampoline ran, or the
/// scheduler reaper dropped the stack. Quiescence is reached only when the
/// bootstrap Thread and one permanent idle Thread per online CPU remain.
pub(super) fn quiesce_workers() -> Result<scheduler::Statistics, QuiescenceError> {
    let mut last = QuiescenceSnapshot {
        threads: 0,
        ready: 0,
        running: 0,
        blocked: 0,
        migrating: 0,
        idle: 0,
        idle_class_threads: 0,
        online_cpus: crate::kernel::cpu::online_cpu_count(),
    };

    for _ in 0..MAX_QUIESCENCE_PASSES {
        scheduler::yield_now()?;
        let statistics = scheduler::statistics()?;
        let online_cpus = crate::kernel::cpu::online_cpu_count();
        last = QuiescenceSnapshot {
            threads: statistics.threads,
            ready: statistics.ready,
            running: statistics.running,
            blocked: statistics.blocked,
            migrating: statistics.migrating,
            idle: statistics.idle,
            idle_class_threads: statistics.idle_class_threads,
            online_cpus,
        };
        if statistics.ready == 0
            && statistics.blocked == 0
            && statistics.migrating == 0
            && statistics.running == 1
            && statistics.idle == online_cpus
            && statistics.idle_class_threads == online_cpus
            && statistics.threads == online_cpus.saturating_add(1)
        {
            return Ok(statistics);
        }
    }
    Err(QuiescenceError::Timeout(last))
}
