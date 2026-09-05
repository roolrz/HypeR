// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Per-CPU software timers multiplexed onto one architectural comparator.

use hyper::cpu::{CpuIndex, PerCpu};
use hyper::sync::InterruptSpinLock;
use hyper::time::{
    OwnedDeadlineQueue, PendingTimer, ReservedTimerCallbacks, ReservedTimerNode, TimerQueueError,
};

use crate::kernel::task::scheduler;

pub use hyper::time::{QueueStats, TimerCallback, TimerEvent, TimerHandle, TimerMode};

/// Bound one interrupt's callback work without imposing a timer storage cap.
const MAX_CALLBACKS_PER_INTERRUPT: usize = 64;

type TimerLock = InterruptSpinLock<ProcessorTimers, crate::hal::irq::LocalMask>;

static TIMERS: PerCpu<TimerLock> =
    PerCpu::new([const { TimerLock::new(ProcessorTimers::new()) }; hyper::cpu::MAX_CPUS]);

struct ProcessorTimers {
    initialized: bool,
    queue: OwnedDeadlineQueue,
}

type ReservationLock = InterruptSpinLock<ReservationState, crate::hal::irq::LocalMask>;

enum ReservationState {
    Idle {
        node: Option<ReservedTimerNode>,
        completed_generation: u64,
    },
    Arming {
        generation: u64,
    },
    Queued {
        generation: u64,
        handle: TimerHandle,
        notify: fn(usize),
        notify_context: usize,
    },
    Firing {
        generation: u64,
        handle: TimerHandle,
        notify: fn(usize),
        notify_context: usize,
    },
    Completed {
        generation: u64,
        handle: TimerHandle,
        node: Option<ReservedTimerNode>,
    },
}

/// A single preallocated one-shot timer which can be rearmed without allocation.
pub(crate) struct ReservedTimer {
    state: ReservationLock,
}

impl ReservedTimer {
    pub(crate) fn try_new() -> Result<Self, super::Error> {
        let node = ReservedTimerNode::try_new()?;
        Ok(Self {
            state: ReservationLock::new(ReservationState::Idle {
                node: Some(node),
                completed_generation: 0,
            }),
        })
    }

    pub(crate) fn arm(
        &self,
        deadline: u64,
        notify: fn(usize),
        notify_context: usize,
    ) -> Result<ArmedReservedTimer<'_>, super::Error> {
        let (cpu_pin, cpu) = pin_current_timer_cpu()?;
        let result = self.arm_on_pinned_cpu(cpu, deadline, notify, notify_context);
        release_timer_cpu_pin(cpu_pin);
        result
    }

    fn arm_on_pinned_cpu(
        &self,
        cpu: CpuIndex,
        deadline: u64,
        notify: fn(usize),
        notify_context: usize,
    ) -> Result<ArmedReservedTimer<'_>, super::Error> {
        TIMERS[cpu].with(|timers| ensure_initialized(timers))?;
        let (node, generation) = self.state.with(|state| {
            let ReservationState::Idle {
                node,
                completed_generation,
            } = state
            else {
                return Err(super::Error::TimerQueue(TimerQueueError::InvalidHandle));
            };
            let generation = completed_generation
                .checked_add(1)
                .ok_or(super::Error::TimerQueue(TimerQueueError::InvalidHandle))?;
            let node = node
                .take()
                .ok_or(super::Error::TimerQueue(TimerQueueError::InvalidHandle))?;
            *state = ReservationState::Arming { generation };
            Ok((node, generation))
        })?;
        let context = core::ptr::from_ref(self).expose_provenance();
        let pending = node.prepare(
            deadline,
            ReservedTimerCallbacks {
                callback: reserved_expire,
                context,
                claim: claim_reserved,
                claim_context: context,
                recycle: recycle_reserved,
                recycle_context: context,
            },
        );
        TIMERS[cpu].with(|timers| {
            let previous = timers.queue.next_deadline();
            let handle = timers.queue.insert_reserved(pending);
            self.state.with(|state| match state {
                ReservationState::Arming {
                    generation: pending_generation,
                } if *pending_generation == generation => {
                    *state = ReservationState::Queued {
                        generation,
                        handle,
                        notify,
                        notify_context,
                    };
                }
                _ => crate::kernel::crash::fatal(format_args!(
                    "HypeR: reserved timer lost its arming ownership"
                )),
            });
            if timers.queue.next_deadline() != previous
                && let Err(error) = program_next(&timers.queue)
            {
                let node = match timers.queue.cancel_reserved(handle) {
                    Ok(node) => node,
                    Err(_) => crate::kernel::crash::fatal(format_args!(
                        "HypeR: reserved timer arm rollback failed"
                    )),
                };
                self.restore_cancelled(generation, handle, node);
                return Err(error);
            }
            Ok(ArmedReservedTimer {
                reservation: self,
                handle,
                generation,
                armed: true,
            })
        })
    }

    fn restore_cancelled(&self, generation: u64, handle: TimerHandle, node: ReservedTimerNode) {
        self.state.with(|state| match state {
            ReservationState::Queued {
                generation: current,
                handle: current_handle,
                ..
            } if *current == generation && *current_handle == handle => {
                *state = ReservationState::Idle {
                    node: Some(node),
                    completed_generation: generation,
                };
            }
            _ => crate::kernel::crash::fatal(format_args!(
                "HypeR: reserved timer cancellation ownership mismatch"
            )),
        });
    }

    fn claim_callback(&self, event: TimerEvent) {
        self.state.with(|state| match state {
            ReservationState::Queued {
                generation,
                handle,
                notify,
                notify_context,
            } if *handle == event.handle => {
                *state = ReservationState::Firing {
                    generation: *generation,
                    handle: *handle,
                    notify: *notify,
                    notify_context: *notify_context,
                };
            }
            _ => crate::kernel::crash::fatal(format_args!("HypeR: stale reserved timer callback")),
        })
    }

    fn notify_callback(&self, event: TimerEvent) {
        let (notify, context) = self.state.with(|state| match state {
            ReservationState::Firing {
                handle,
                notify,
                notify_context,
                ..
            } if *handle == event.handle => (*notify, *notify_context),
            _ => crate::kernel::crash::fatal(format_args!(
                "HypeR: reserved timer callback lost its firing ownership"
            )),
        });
        notify(context);
    }

    fn recycle(&self, node: ReservedTimerNode) {
        self.state.with(|state| {
            let ReservationState::Firing {
                generation, handle, ..
            } = state
            else {
                crate::kernel::crash::fatal(format_args!(
                    "HypeR: reserved timer recycled outside callback ownership"
                ));
            };
            let generation = *generation;
            let handle = *handle;
            // The callback has completed, but its linear ArmedReservedTimer
            // still owns this generation. Only exact retirement may return
            // the node to Idle and permit rearming.
            *state = ReservationState::Completed {
                generation,
                handle,
                node: Some(node),
            };
        });
    }

    fn retire(&self, handle: TimerHandle, generation: u64) -> Result<(), super::Error> {
        let owner = CpuIndex::new(handle.queue_id()).ok_or(super::Error::InvalidCpuIndex)?;
        let cancelled = TIMERS[owner].with(|timers| {
            ensure_initialized(timers)?;
            let previous = timers.queue.next_deadline();
            match timers.queue.cancel_reserved(handle) {
                Ok(node) => {
                    self.restore_cancelled(generation, handle, node);
                    // This CPU classification is made while the queue's
                    // InterruptSpinLock masks local IRQs, so IRQ-tail
                    // migration cannot intervene before comparator update.
                    if owner == current_cpu()? && timers.queue.next_deadline() != previous {
                        program_next(&timers.queue)?;
                    }
                    Ok::<bool, super::Error>(true)
                }
                Err(TimerQueueError::InvalidHandle) => Ok::<bool, super::Error>(false),
                Err(error) => Err(error.into()),
            }
        })?;
        if cancelled {
            return Ok(());
        }
        loop {
            let complete = self.state.with(|state| match state {
                ReservationState::Firing {
                    generation: current,
                    handle: current_handle,
                    ..
                } if *current == generation && *current_handle == handle => false,
                ReservationState::Completed {
                    generation: current,
                    handle: current_handle,
                    node,
                } if *current == generation && *current_handle == handle => {
                    let node = match node.take() {
                        Some(node) => node,
                        None => crate::kernel::crash::fatal(format_args!(
                            "HypeR: completed reserved timer lost its node"
                        )),
                    };
                    *state = ReservationState::Idle {
                        node: Some(node),
                        completed_generation: generation,
                    };
                    true
                }
                _ => crate::kernel::crash::fatal(format_args!(
                    "HypeR: reserved timer vanished without exact callback ownership"
                )),
            });
            if complete {
                return Ok(());
            }
            core::hint::spin_loop();
        }
    }
}

impl Drop for ReservedTimer {
    fn drop(&mut self) {
        let idle = self
            .state
            .with(|state| matches!(state, ReservationState::Idle { node: Some(_), .. }));
        if !idle {
            crate::hal::cpu::halt()
        }
    }
}

#[must_use = "an armed reserved timer must be retired exactly once"]
pub(crate) struct ArmedReservedTimer<'a> {
    reservation: &'a ReservedTimer,
    handle: TimerHandle,
    generation: u64,
    armed: bool,
}

impl ArmedReservedTimer<'_> {
    pub(crate) fn retire(mut self) -> Result<(), super::Error> {
        self.reservation.retire(self.handle, self.generation)?;
        self.armed = false;
        Ok(())
    }
}

impl Drop for ArmedReservedTimer<'_> {
    fn drop(&mut self) {
        if self.armed {
            crate::hal::cpu::halt()
        }
    }
}

fn reserved_expire(event: TimerEvent, context: usize) {
    let pointer = core::ptr::with_exposed_provenance::<ReservedTimer>(context);
    // SAFETY: an armed reserved node borrows its stable reservation until the
    // callback is recycled or exact cancellation returns the node.
    let reservation = unsafe { &*pointer };
    reservation.notify_callback(event);
}

fn claim_reserved(event: TimerEvent, context: usize) {
    let pointer = core::ptr::with_exposed_provenance::<ReservedTimer>(context);
    // SAFETY: queue ownership retains this stable reservation while claiming
    // expiry under the queue lock.
    let reservation = unsafe { &*pointer };
    reservation.claim_callback(event);
}

fn recycle_reserved(node: ReservedTimerNode, context: usize) {
    let pointer = core::ptr::with_exposed_provenance::<ReservedTimer>(context);
    // SAFETY: the same reserved node carries the reservation pointer installed
    // by `arm`, and recycling is invoked exactly once after its callback.
    let reservation = unsafe { &*pointer };
    reservation.recycle(node);
}

impl ProcessorTimers {
    const fn new() -> Self {
        Self {
            initialized: false,
            queue: OwnedDeadlineQueue::new(),
        }
    }
}

pub(super) fn initialize_local() -> Result<(), super::Error> {
    let cpu = current_cpu()?;
    TIMERS[cpu].with(|timers| {
        if timers.initialized {
            return Err(super::Error::TimerQueueAlreadyInitialized);
        }
        crate::hal::time::disable_local_timer();
        timers.queue.initialize_id(cpu.get())?;
        timers.initialized = true;
        Ok(())
    })
}

pub fn schedule_at(
    deadline: u64,
    mode: TimerMode,
    callback: TimerCallback,
    context: usize,
) -> Result<TimerHandle, super::Error> {
    let pending = PendingTimer::try_new(deadline, mode, callback, context)?;
    // Allocation must precede pinning: allocator slow paths may reach a
    // scheduling point, while the selected queue and comparator are CPU-local.
    let (cpu_pin, cpu) = pin_current_timer_cpu()?;
    if let Err(error) = TIMERS[cpu].with(|timers| ensure_initialized(timers)) {
        release_timer_cpu_pin(cpu_pin);
        return Err(error);
    }
    let (result, retired, rollback_failed) = TIMERS[cpu].with(|timers| {
        let previous = timers.queue.next_deadline();
        let handle = timers.queue.insert(pending);
        if timers.queue.next_deadline() != previous
            && let Err(error) = program_next(&timers.queue)
        {
            let retired = match timers.queue.cancel(handle) {
                Ok(retired) => Some(retired),
                // The exact handle was inserted while this lock was held. If
                // rollback cannot find it, returning would abandon a possibly
                // live callback context. Report the invariant only after the
                // timer lock and local interrupt mask have been released.
                Err(_) => return (Err(error), None, true),
            };
            return (Err(error), retired, false);
        }
        (Ok(handle), None, false)
    });
    release_timer_cpu_pin(cpu_pin);
    // Queue retirement may enter the allocator and therefore happens only
    // after the CPU-local comparator transaction is complete and unpinned.
    drop(retired);
    if rollback_failed {
        // The caller that supplied `context` cannot observe a return, so its
        // callback owner remains live while coordinated crash handling stops
        // every CPU that could consume the corrupt timer queue.
        crate::kernel::crash::fatal(format_args!("HypeR: exact timer insertion rollback failed"));
    }
    result
}

pub fn schedule_after(
    nanoseconds: u64,
    callback: TimerCallback,
    context: usize,
) -> Result<TimerHandle, super::Error> {
    let deadline = super::deadline_after(nanoseconds)?;
    schedule_at(deadline, TimerMode::OneShot, callback, context)
}

pub fn schedule_periodic(
    first_deadline: u64,
    interval_ticks: u64,
    callback: TimerCallback,
    context: usize,
) -> Result<TimerHandle, super::Error> {
    schedule_at(
        first_deadline,
        TimerMode::Periodic {
            interval: interval_ticks,
        },
        callback,
        context,
    )
}

/// Cancels a timer on the queue identified by its handle.
///
/// A Thread may resume on a different CPU from the one on which it armed a
/// timer. Cancellation therefore follows the handle back to its source queue.
/// Only the source CPU can update its architectural comparator: remote
/// cancellation may leave an obsolete earlier deadline programmed, causing at
/// most one harmless timer interrupt which then programs the next deadline.
pub fn cancel(handle: TimerHandle) -> Result<(), super::Error> {
    let owner = CpuIndex::new(handle.queue_id()).ok_or(super::Error::InvalidCpuIndex)?;
    let (cpu_pin, current) = pin_current_timer_cpu()?;
    let cancelled = TIMERS[owner].with(|timers| {
        ensure_initialized(timers)?;
        let previous = timers.queue.next_deadline();
        let retired = timers.queue.cancel(handle)?;
        let result = if owner == current && timers.queue.next_deadline() != previous {
            program_next(&timers.queue)
        } else {
            Ok(())
        };
        Ok::<_, super::Error>((result, retired))
    });
    release_timer_cpu_pin(cpu_pin);
    let (result, retired) = cancelled?;
    drop(retired);
    result
}

pub fn local_statistics() -> Option<QueueStats> {
    let (cpu_pin, cpu) = pin_current_timer_cpu().ok()?;
    let statistics = TIMERS[cpu].with(|timers| timers.initialized.then(|| timers.queue.stats()));
    release_timer_cpu_pin(cpu_pin);
    statistics
}

pub(super) fn handle_interrupt() -> Result<usize, super::Error> {
    let cpu = current_cpu()?;
    crate::hal::time::mask_local_timer();
    let mut callbacks = 0;
    while callbacks < MAX_CALLBACKS_PER_INTERRUPT {
        let expired = TIMERS[cpu].with(|timers| {
            ensure_initialized(timers)?;
            Ok::<_, super::Error>(timers.queue.pop_expired(super::monotonic_ticks()))
        })?;
        let Some(expired) = expired else {
            break;
        };
        expired.invoke();
        callbacks += 1;
    }
    TIMERS[cpu].with(|timers| {
        ensure_initialized(timers)?;
        program_next(&timers.queue)
    })?;
    Ok(callbacks)
}

fn current_cpu() -> Result<CpuIndex, super::Error> {
    crate::kernel::cpu::current_index().ok_or(super::Error::InvalidCpuIndex)
}

/// Pins the calling continuation before selecting one CPU-local timer queue.
fn pin_current_timer_cpu() -> Result<(scheduler::PreemptionGuard, CpuIndex), super::Error> {
    let cpu_pin = scheduler::preempt_disable().map_err(super::Error::Preemption)?;
    match current_cpu() {
        Ok(cpu) => Ok((cpu_pin, cpu)),
        Err(error) => {
            release_timer_cpu_pin(cpu_pin);
            Err(error)
        }
    }
}

/// Ends a timer CPU-local transaction without adding a scheduling point.
fn release_timer_cpu_pin(cpu_pin: scheduler::PreemptionGuard) {
    if scheduler::preempt_enable_without_reschedule(cpu_pin).is_err() {
        crate::kernel::crash::fatal(format_args!(
            "HypeR: timer CPU-local preemption pin release failed"
        ));
    }
}

fn ensure_initialized(timers: &ProcessorTimers) -> Result<(), super::Error> {
    if timers.initialized {
        Ok(())
    } else {
        Err(super::Error::TimerQueueNotInitialized)
    }
}

fn program_next(queue: &OwnedDeadlineQueue) -> Result<(), super::Error> {
    match queue.next_deadline() {
        Some(deadline) => crate::hal::time::program_deadline(deadline)?,
        None => crate::hal::time::disable_local_timer(),
    }
    Ok(())
}

impl From<TimerQueueError> for super::Error {
    fn from(error: TimerQueueError) -> Self {
        Self::TimerQueue(error)
    }
}
