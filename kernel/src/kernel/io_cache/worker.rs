// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Normal-context growth and pressure reclaim of clean file pages.

use core::sync::atomic::{AtomicBool, Ordering};

use hyper::sync::{DeferredWork, WorkDisposition};

use crate::kernel::mm::cache_memory;
use crate::kernel::mm::reclaim::{ReclaimGuard, Target};
use crate::kernel::task::scheduler;

mod request;
mod request_state;

const RECLAIM_BATCH: usize = 64;
const MINIMUM_SLOTS: usize = 64;

static WORK: DeferredWork = DeferredWork::new();
static WAKE: crate::kernel::sync::Completion = crate::kernel::sync::Completion::new();
static READY: AtomicBool = AtomicBool::new(false);
static EMERGENCY_RECLAIM: AtomicBool = AtomicBool::new(false);
static DRAINING: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "kernel-self-test")]
static TEST_PRESSURE: AtomicBool = AtomicBool::new(false);

pub(crate) fn reclaim(target: Target) -> Option<ReclaimGuard> {
    request::reclaim(target)
}

/// Starts after IRQ and time initialization; early requests remain sticky.
pub(crate) fn initialize() -> Result<(), scheduler::Error> {
    let worker = scheduler::kthread_create("kio-reclaim", worker_entry, 0)?;
    if !WORK.claim_initial_worker()
        || READY
            .compare_exchange(false, true, Ordering::Release, Ordering::Relaxed)
            .is_err()
    {
        return Err(scheduler::Error::AlreadyInitialized);
    }
    let _ = WORK.consume_prompt();
    if !scheduler::thread_ready(worker)? {
        return Err(scheduler::Error::InvalidThreadState);
    }
    Ok(())
}

/// May run under allocator or caller locks: publish work without allocating,
/// freeing payloads, or entering scheduler/cache synchronization.
pub(crate) fn request() {
    if !WORK.request() || !READY.load(Ordering::Acquire) {
        return;
    }
    if let Some(cpu) = crate::kernel::cpu::current_index() {
        crate::kernel::irq::reschedule::notify(cpu);
    }
}

pub(crate) fn service_irq_prompt() {
    if !READY.load(Ordering::Acquire) || !WORK.consume_prompt() || !WORK.claim_notification() {
        return;
    }
    if let Err(error) = WAKE.complete() {
        crate::kernel::crash::fatal(format_args!("HypeR: I/O reclaim wake failed: {error:?}"));
    }
}

/// A detached reader may hold the last physical page after worker eviction.
/// Its eventual release must let a pressure-closed cache resume admissions.
pub(crate) fn page_released() {
    if DRAINING.load(Ordering::Acquire) {
        request();
    }
}

/// A load admitted before pressure may publish after the worker went to sleep.
/// Prompt another pass even when no table resize is awaiting its completion.
pub(crate) fn load_finished() {
    if DRAINING.load(Ordering::Acquire) {
        request();
    }
}

/// A bounded fallback after an ordinary allocation fails. It neither waits for
/// the worker nor takes filesystem locks. Only the concrete system payload is
/// admitted here: its final file-record release performs bounded atomic
/// accounting and memory deallocation, with no backend or scheduler callbacks.
pub(crate) fn try_reclaim(requested_pages: usize) -> usize {
    request();
    if EMERGENCY_RECLAIM
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        return 0;
    }
    let freed = crate::kernel::vfs::file_cache().map_or(0, |cache| {
        cache.try_reclaim_batch(requested_pages.clamp(1, RECLAIM_BATCH))
    });
    EMERGENCY_RECLAIM.store(false, Ordering::Release);
    freed
}

#[cfg(feature = "kernel-self-test")]
pub(crate) fn permanent_worker_count_for_test() -> usize {
    usize::from(READY.load(Ordering::Acquire))
}

#[cfg(feature = "kernel-self-test")]
pub(crate) fn pressure_for_test(enabled: bool) {
    TEST_PRESSURE.store(enabled, Ordering::Release);
    request();
}

#[cfg(feature = "kernel-self-test")]
pub(crate) fn draining_for_test() -> bool {
    DRAINING.load(Ordering::Acquire)
}

extern "C" fn worker_entry(_: usize) {
    let mut draining = false;
    let mut demand = None;
    loop {
        WORK.begin_batch();
        let more = service_batch(&mut draining, &mut demand);
        match WORK.finish_batch(more) {
            WorkDisposition::Continue => {
                if let Err(error) = scheduler::cond_resched() {
                    crate::kernel::crash::fatal(format_args!(
                        "HypeR: I/O reclaim reschedule failed: {error:?}"
                    ));
                }
            }
            WorkDisposition::Wait => {
                if let Err(error) = WAKE.wait() {
                    crate::kernel::crash::fatal(format_args!(
                        "HypeR: I/O reclaim wait failed: {error:?}"
                    ));
                }
            }
        }
    }
}

struct Demand {
    request: request_state::Request<Target>,
    cursor: super::ReclaimCursor,
}

fn service_batch(draining: &mut bool, demand: &mut Option<Demand>) -> bool {
    let Some(cache) = crate::kernel::vfs::file_cache() else {
        return false;
    };
    if service_demand(cache, demand) {
        return true;
    }
    let Some(memory) = cache_memory::availability() else {
        return false;
    };
    let watermarks = cache_memory::watermarks(memory.managed_pages);
    let free_pages = memory
        .free_pages
        .saturating_sub(memory.pending_cache_metadata_pages);
    if free_pages <= watermarks.stop_pages {
        *draining = true;
    } else if free_pages >= watermarks.resume_pages {
        *draining = false;
    }
    #[cfg(feature = "kernel-self-test")]
    if TEST_PRESSURE.load(Ordering::Acquire) {
        *draining = true;
    }
    DRAINING.store(*draining, Ordering::Release);
    cache.set_admission(!*draining);
    let retired_records = crate::kernel::vfs::reclaim_file_records(RECLAIM_BATCH);
    let usage = cache.usage();
    if *draining {
        cache.acknowledge_growth();
        cache.cancel_maintenance();
        let detached = cache.reclaim_batch(RECLAIM_BATCH);
        // Do not spin on reader-pinned or loading pages. Their eventual
        // retirement or the next allocator/cache request prompts a new pass.
        if detached != 0 || retired_records != 0 {
            return true;
        }
        let target = MINIMUM_SLOTS;
        if target < usage.capacity {
            let _ = cache.maintain_capacity(target);
        }
        return false;
    }
    if usage.admission_paused {
        return false;
    }
    if usage.growth_requested {
        let target = usage
            .capacity
            .checked_mul(2)
            .map(|capacity| capacity.max(MINIMUM_SLOTS));
        if let Some(target) = target {
            match cache.maintain_capacity(target) {
                Ok(super::Maintenance::Busy) => return false,
                Ok(_) | Err(_) => cache.acknowledge_growth(),
            }
        } else {
            cache.acknowledge_growth();
        }
    } else if usage.maintenance_pending {
        cache.cancel_maintenance();
    }
    false
}

/// A scheduled request is independent of percentage watermarks. Each pass
/// examines at most 64 slots, and a finite sweep never waits for pins/loaders.
fn service_demand(
    cache: &super::FileDataCache<crate::kernel::vfs::CachePage>,
    demand: &mut Option<Demand>,
) -> bool {
    let Some(pending) = request::pending() else {
        *demand = None;
        return false;
    };
    if demand
        .as_ref()
        .is_none_or(|active| active.request != pending)
    {
        *demand = Some(Demand {
            request: pending,
            cursor: super::ReclaimCursor::new(),
        });
    }
    let Some(active) = demand.as_mut() else {
        return false;
    };
    if physical_target_ready(pending.target) {
        request::complete(pending.generation);
        *demand = None;
        return true;
    }
    let scanned = cache.reclaim_scan(&mut active.cursor, RECLAIM_BATCH, |page| {
        match pending.target {
            Target::PhysicalOrder(_) => true,
            Target::Domain(domain) => page.owner().charges_domain(domain),
        }
    });
    if scanned.finished || physical_target_ready(pending.target) {
        if scanned.finished {
            // Dead weak headers and magazine-held objects can prevent buddy
            // coalescing even after all eligible file pages were detached.
            let domain = match pending.target {
                Target::Domain(domain) => Some(domain),
                Target::PhysicalOrder(_) => None,
            };
            crate::kernel::vfs::reclaim_idle_records(domain);
            crate::kernel::mm::allocator::GLOBAL_ALLOCATOR.reclaim_local_caches();
        }
        request::complete(pending.generation);
        *demand = None;
    }
    true
}

fn physical_target_ready(target: Target) -> bool {
    let Target::PhysicalOrder(order) = target else {
        return false;
    };
    cache_memory::availability().is_some_and(|memory| {
        memory
            .largest_free_order
            .is_some_and(|available| available >= order)
    })
}
