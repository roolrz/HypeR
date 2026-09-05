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
const PROCESS_RETRY_NS: u64 = 10_000_000;

static WORK: DeferredWork = DeferredWork::new();
static WAKE: crate::kernel::sync::Completion = crate::kernel::sync::Completion::new();
static WORKER_PUBLISHED: AtomicBool = AtomicBool::new(false);
static IRQ_PROMPTS_READY: AtomicBool = AtomicBool::new(false);
static PROCESS_RETRY_DUE: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "kernel-self-test")]
static TEST_WORKER: hyper::sync::InterruptSpinLock<
    Option<crate::kernel::task::thread::ThreadId>,
    crate::hal::irq::LocalMask,
> = hyper::sync::InterruptSpinLock::new(None);
#[cfg(feature = "kernel-self-test")]
static TEST_RETRY_CPU: core::sync::atomic::AtomicUsize =
    core::sync::atomic::AtomicUsize::new(usize::MAX);

// The current process-retirement integration case needs Native user mappings,
// so only the AArch64 self-test binary calls these architecture-neutral hooks.
#[cfg(feature = "kernel-self-test")]
#[allow(dead_code)]
pub(crate) fn set_affinity_for_test(
    affinity: crate::kernel::task::scheduler::CpuMask,
) -> Result<(), crate::kernel::task::scheduler::Error> {
    let worker = TEST_WORKER
        .with(|worker| *worker)
        .ok_or(crate::kernel::task::scheduler::Error::NotInitialized)?;
    crate::kernel::task::scheduler::set_thread_affinity(worker, affinity)?;
    TEST_RETRY_CPU.store(usize::MAX, Ordering::Release);
    Ok(())
}

#[cfg(feature = "kernel-self-test")]
#[allow(dead_code)]
pub(crate) fn retry_observed_on_for_test(cpu: hyper::cpu::CpuIndex) -> bool {
    TEST_RETRY_CPU.load(Ordering::Acquire) == cpu.get()
}

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
    #[cfg(feature = "kernel-self-test")]
    TEST_WORKER.with(|slot| *slot = Some(worker));
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
    // Reserve one node for the worker lifetime: retry under memory pressure
    // must not require another allocation. Its queue follows timer ownership,
    // never a hard-coded processor identity.
    let retry_timer = match crate::kernel::time::ReservedTimer::try_new() {
        Ok(timer) => timer,
        Err(error) => crate::kernel::crash::fatal(format_args!(
            "HypeR: reaper timer reservation failed: {error:?}"
        )),
    };
    let mut armed_retry: Option<crate::kernel::time::ArmedReservedTimer<'_>> = None;
    loop {
        // Unrelated retirement work may wake the worker while a Process retry
        // remains delayed. Preserve that timer so a steady producer stream
        // cannot postpone the older Process indefinitely.
        if PROCESS_RETRY_DUE.swap(false, Ordering::AcqRel) {
            let timer = match armed_retry.take() {
                Some(timer) => timer,
                None => crate::kernel::crash::fatal(format_args!(
                    "HypeR: Process retry expired without timer ownership"
                )),
            };
            if let Err(error) = timer.retire() {
                crate::kernel::crash::fatal(format_args!(
                    "HypeR: reaper timer retirement failed: {error:?}"
                ));
            }
            crate::kernel::process::promote_delayed_retirements(&mut access);
        }
        WORK.begin_batch();
        crate::kernel::process::reap_one_process(&mut access);
        let mut subsystem_work = false;
        for _ in 0..REAP_BATCH {
            let result = if prefer_objects {
                reap_object_or_thread(&mut access)
            } else {
                reap_thread_or_object(&mut access)
            };
            prefer_objects = !prefer_objects;
            match result {
                Ok(ReapStep::Reaped { more }) => subsystem_work = more,
                Ok(ReapStep::Empty) => {
                    subsystem_work = false;
                    break;
                }
                Err(error) => crate::kernel::crash::fatal(format_args!(
                    "HypeR: kernel reaper failed: {error:?}"
                )),
            }
        }
        let process_work = crate::kernel::process::retirement_work(&access);
        if process_work.delayed && armed_retry.is_none() {
            let deadline = match crate::kernel::time::deadline_after(PROCESS_RETRY_NS) {
                Ok(deadline) => deadline,
                Err(error) => crate::kernel::crash::fatal(format_args!(
                    "HypeR: reaper retry deadline failed: {error:?}"
                )),
            };
            armed_retry = Some(match retry_timer.arm(deadline, retry_expired, 0) {
                Ok(timer) => timer,
                Err(error) => crate::kernel::crash::fatal(format_args!(
                    "HypeR: reaper retry timer failed: {error:?}"
                )),
            });
        }
        let more_work = subsystem_work || process_work.ready;
        // Arm before relinquishing work ownership. An expiry or producer racing
        // this handoff sets durable work and cannot be lost before WAKE.wait.
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

fn retry_expired(_context: usize) {
    #[cfg(feature = "kernel-self-test")]
    TEST_RETRY_CPU.store(
        crate::kernel::cpu::current_index().map_or(usize::MAX, |cpu| cpu.get()),
        Ordering::Release,
    );
    PROCESS_RETRY_DUE.store(true, Ordering::Release);
    request();
}
