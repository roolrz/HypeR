// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Targeted reschedule-IPI delivery and completion proof.

use hyper::cpu::CpuIndex;
use hyper::sync::atomic::{AtomicBool, Ordering};

use crate::kernel::task::scheduler::{self, CpuMask, ThreadPriority};

const DELIVERY_TIMEOUT_NS: u64 = 1_000_000_000;

static REMOTE_WORKER_RAN: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Error {
    InvalidTarget,
    NotificationUnavailable,
    Quiescence(super::support::QuiescenceError),
    Scheduler(scheduler::Error),
    Time(crate::kernel::time::Error),
    Timeout(&'static str),
}

impl From<scheduler::Error> for Error {
    fn from(error: scheduler::Error) -> Self {
        Self::Scheduler(error)
    }
}

impl From<super::support::QuiescenceError> for Error {
    fn from(error: super::support::QuiescenceError) -> Self {
        Self::Quiescence(error)
    }
}

impl From<crate::kernel::time::Error> for Error {
    fn from(error: crate::kernel::time::Error) -> Self {
        Self::Time(error)
    }
}

pub(super) fn run() -> Result<(), Error> {
    if crate::kernel::cpu::online_cpu_count() <= 1 {
        crate::println!("HypeR test: targeted reschedule IPI skipped (one CPU online)");
        return Ok(());
    }

    let target = CpuIndex::new(1).ok_or(Error::InvalidTarget)?;
    let baseline = crate::kernel::irq::reschedule_delivery_count_for_test(target);
    REMOTE_WORKER_RAN.store(false, Ordering::Release);

    let worker = scheduler::kthread_create_fifo_with_affinity(
        "reschedule-ipi-remote",
        remote_worker,
        0,
        ThreadPriority::HIGHEST,
        CpuMask::single(target),
    )?;
    scheduler::thread_ready(worker)?;
    wait_until("remote worker did not run", || {
        REMOTE_WORKER_RAN.load(Ordering::Acquire)
    })?;

    let first_delivery = crate::kernel::irq::reschedule_delivery_count_for_test(target);
    if first_delivery <= baseline {
        return Err(Error::Timeout("remote worker ran without SGI dispatch"));
    }

    // A second delivery after the handler ran proves that the first SGI was
    // completed and deactivated; an active SGI cannot be delivered again.
    if !crate::hal::irq::notify_reschedule(target) {
        return Err(Error::NotificationUnavailable);
    }
    wait_until("completed SGI was not delivered again", || {
        crate::kernel::irq::reschedule_delivery_count_for_test(target) > first_delivery
    })?;
    super::support::quiesce_workers()?;

    crate::println!("HypeR test: targeted reschedule IPI delivery and EOI passed");
    Ok(())
}

fn wait_until(description: &'static str, condition: impl FnMut() -> bool) -> Result<(), Error> {
    if crate::kernel::time::spin_wait_until(DELIVERY_TIMEOUT_NS, condition)? {
        Ok(())
    } else {
        Err(Error::Timeout(description))
    }
}

extern "C" fn remote_worker(_argument: usize) {
    REMOTE_WORKER_RAN.store(true, Ordering::Release);
}
