// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Normal-context retirement service for detached kernel resources.
//!
//! Producers publish work without allocating or acquiring scheduler policy
//! locks. The dedicated worker drains subsystem retirement before final
//! kernel-object destruction, and the shared sticky-work protocol closes the
//! queue-empty-to-sleep race for both sources.

use core::sync::atomic::{AtomicBool, Ordering};

use hyper::sync::{DeferredWork, WorkDisposition};

const REAP_BATCH: usize = 16;

static WORK: DeferredWork = DeferredWork::new();
static WAKE: crate::kernel::sync::Completion = crate::kernel::sync::Completion::new();
static WORKER_PUBLISHED: AtomicBool = AtomicBool::new(false);
static IRQ_PROMPTS_READY: AtomicBool = AtomicBool::new(false);

/// Unforgeable proof that finalizers execute on the dedicated reaper Thread.
pub(crate) struct ReaperAccess {
    _private: (),
}

/// Creates and publishes the sole normal-context retirement worker.
pub(crate) fn initialize() -> Result<(), crate::kernel::task::scheduler::Error> {
    let worker = crate::kernel::task::scheduler::kthread_create("kreaper", worker_entry, 0)?;
    if !WORK.claim_initial_worker()
        || WORKER_PUBLISHED
            .compare_exchange(false, true, Ordering::Release, Ordering::Relaxed)
            .is_err()
    {
        return Err(crate::kernel::task::scheduler::Error::AlreadyInitialized);
    }
    // The initial worker owns every request published before this point.
    // Clear their unissued prompt before the worker can drain and sleep, or a
    // stale IRQ_PROMPTED bit could suppress the first post-startup notifier.
    let _ = WORK.consume_prompt();
    if !crate::kernel::task::scheduler::thread_ready(worker)? {
        return Err(crate::kernel::task::scheduler::Error::InvalidThreadState);
    }
    Ok(())
}

/// Publishes retirement work and prompts IRQ-tail wakeup when required.
///
/// Requests made before worker publication remain sticky. The initial worker
/// owns their first drain, so early object construction rollback needs no
/// scheduler or interrupt dependency.
pub(crate) fn request() {
    let elected = WORK.request();
    if !elected || !IRQ_PROMPTS_READY.load(Ordering::Acquire) {
        return;
    }
    match crate::kernel::cpu::current_index() {
        Some(cpu) => crate::kernel::irq::reschedule::notify(cpu),
        // IRQ prompting is enabled only after bootstrap CPU identity and the
        // interrupt route exist. A plain event would wake a WFI but would not
        // consume the durable prompt or wake the scheduler Thread.
        None => crate::hal::cpu::halt(),
    }
}

/// Opens hardware prompting after the interrupt subsystem is operational.
///
/// Work published between scheduler and IRQ initialization remains in the
/// sticky state. This transition consumes any elected early prompt and, when
/// the worker does not already own wake responsibility, completes its wait
/// directly from normal boot context.
pub(crate) fn enable_irq_prompts() {
    if !WORKER_PUBLISHED.load(Ordering::Acquire)
        || IRQ_PROMPTS_READY
            .compare_exchange(false, true, Ordering::Release, Ordering::Relaxed)
            .is_err()
    {
        crate::hal::cpu::halt();
    }
    if WORK.consume_prompt()
        && WORK.claim_notification()
        && let Err(error) = WAKE.complete()
    {
        crate::kernel::crash::fatal(format_args!(
            "HypeR: initial kernel reaper wake failed: {error:?}"
        ));
    }
}

/// Converts one durable hardware prompt into a scheduler-safe worker wake.
pub(crate) fn service_irq_prompt() {
    if !WORK.consume_prompt() || !WORK.claim_notification() {
        return;
    }
    if let Err(error) = WAKE.complete() {
        crate::kernel::crash::fatal(format_args!("HypeR: kernel reaper wake failed: {error:?}"));
    }
}

extern "C" fn worker_entry(_argument: usize) {
    let mut access = ReaperAccess { _private: () };
    let mut prefer_objects = false;
    loop {
        WORK.begin_batch();
        let mut more_work = false;
        for _ in 0..REAP_BATCH {
            let result = if prefer_objects {
                reap_object_or_thread(&mut access)
            } else {
                reap_thread_or_object(&mut access)
            };
            prefer_objects = !prefer_objects;
            match result {
                Ok(ReapStep::Reaped { more }) => more_work = more,
                Ok(ReapStep::Empty) => {
                    more_work = false;
                    break;
                }
                Err(error) => crate::kernel::crash::fatal(format_args!(
                    "HypeR: kernel reaper failed: {error:?}"
                )),
            }
        }
        match WORK.finish_batch(more_work) {
            WorkDisposition::Continue => {
                if let Err(error) = crate::kernel::task::scheduler::cond_resched() {
                    crate::kernel::crash::fatal(format_args!(
                        "HypeR: kernel reaper reschedule failed: {error:?}"
                    ));
                }
            }
            WorkDisposition::Wait => {
                if let Err(error) = WAKE.wait() {
                    crate::kernel::crash::fatal(format_args!(
                        "HypeR: kernel reaper wait failed: {error:?}"
                    ));
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReapStep {
    Reaped { more: bool },
    Empty,
}

fn reap_thread_or_object(
    access: &mut ReaperAccess,
) -> Result<ReapStep, crate::kernel::task::scheduler::Error> {
    match crate::kernel::task::scheduler::reap_one_thread(access)? {
        Some(more) => Ok(ReapStep::Reaped {
            more: more || crate::kernel::object::final_reap_pending(access),
        }),
        None => Ok(reap_one_object(access)),
    }
}

fn reap_object_or_thread(
    access: &mut ReaperAccess,
) -> Result<ReapStep, crate::kernel::task::scheduler::Error> {
    if crate::kernel::object::reap_one_final_object(access) {
        return Ok(ReapStep::Reaped {
            more: crate::kernel::object::final_reap_pending(access)
                || crate::kernel::task::scheduler::retirement_pending(access),
        });
    }
    match crate::kernel::task::scheduler::reap_one_thread(access)? {
        Some(more) => Ok(ReapStep::Reaped { more }),
        None => Ok(ReapStep::Empty),
    }
}

fn reap_one_object(access: &mut ReaperAccess) -> ReapStep {
    if crate::kernel::object::reap_one_final_object(access) {
        ReapStep::Reaped {
            more: crate::kernel::object::final_reap_pending(access),
        }
    } else {
        ReapStep::Empty
    }
}
