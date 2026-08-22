// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Per-CPU preemption requests and entry-depth accounting.
//!
//! This module owns only the policy state used to decide whether a safe
//! scheduling point should consult the scheduler. It never switches context.
//! Remote CPUs may publish coalesced requests, while only the owning CPU may
//! change its disable and IRQ nesting depths.

use core::marker::PhantomData;

use hyper::cpu::{CpuIndex, PerCpu};
use hyper::sync::atomic::{AtomicU32, Ordering};

use super::reschedule::PendingReschedule;

const OFFLINE: u32 = 0;
const PREPARING: u32 = 1;
const ONLINE: u32 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    AlreadyOnline,
    Offline,
    InvalidCpu,
    DisableDepthOverflow,
    DisableDepthUnderflow,
    IrqDepthOverflow,
    IrqDepthUnderflow,
    WrongCpu,
}

impl Error {
    pub(crate) const fn description(self) -> &'static str {
        match self {
            Self::AlreadyOnline => "per-CPU preemption state was initialized twice",
            Self::Offline => "per-CPU preemption state is offline",
            Self::InvalidCpu => "invalid logical CPU for preemption accounting",
            Self::DisableDepthOverflow => "preemption-disable depth overflow",
            Self::DisableDepthUnderflow => "preemption-disable depth underflow",
            Self::IrqDepthOverflow => "IRQ nesting overflow",
            Self::IrqDepthUnderflow => "IRQ nesting underflow",
            Self::WrongCpu => "preemption guard completed on the wrong CPU",
        }
    }
}

struct PreemptionState {
    lifecycle: AtomicU32,
    disable_depth: AtomicU32,
    irq_depth: AtomicU32,
    pending: PendingReschedule,
}

impl PreemptionState {
    const fn new() -> Self {
        Self {
            lifecycle: AtomicU32::new(OFFLINE),
            disable_depth: AtomicU32::new(0),
            irq_depth: AtomicU32::new(0),
            pending: PendingReschedule::new(),
        }
    }
}

static PREEMPTION: PerCpu<PreemptionState> =
    PerCpu::new([const { PreemptionState::new() }; hyper::cpu::MAX_CPUS]);

pub(crate) fn prepare_cpu(cpu: CpuIndex) -> Result<CpuPreemptionReservation, Error> {
    let state = &PREEMPTION[cpu];
    state
        .lifecycle
        .compare_exchange(OFFLINE, PREPARING, Ordering::Acquire, Ordering::Relaxed)
        .map_err(|_| Error::AlreadyOnline)?;
    Ok(CpuPreemptionReservation {
        cpu,
        committed: false,
        not_send: PhantomData,
    })
}

/// Rollback capability for one unpublished per-CPU preemption state.
pub(crate) struct CpuPreemptionReservation {
    cpu: CpuIndex,
    committed: bool,
    not_send: PhantomData<*mut ()>,
}

impl CpuPreemptionReservation {
    /// Publishes the zero-depth state after the scheduler CPU record exists.
    pub(crate) fn commit(mut self) {
        PREEMPTION[self.cpu]
            .lifecycle
            .store(ONLINE, Ordering::Release);
        self.committed = true;
    }
}

impl Drop for CpuPreemptionReservation {
    fn drop(&mut self) {
        if !self.committed {
            PREEMPTION[self.cpu]
                .lifecycle
                .store(OFFLINE, Ordering::Release);
        }
    }
}

/// Publishes a reschedule request to `cpu`.
///
/// Release ordering makes scheduler queue publication visible before the
/// target CPU observes the request with Acquire ordering. Requests coalesce;
/// clearing occurs only while the scheduler lock serializes the decision.
pub(crate) fn request(cpu: CpuIndex) -> Result<(), Error> {
    let state = &PREEMPTION[cpu];
    ensure_online(state)?;
    state.pending.publish();
    Ok(())
}

pub(crate) fn pending(cpu: CpuIndex) -> Result<bool, Error> {
    let state = &PREEMPTION[cpu];
    ensure_online(state)?;
    Ok(state.pending.is_pending())
}

/// Takes all requests observed while the scheduler lock is held.
///
/// A concurrent request after this exchange remains pending for the next safe
/// point. A request observed by the exchange already has its ready-queue state
/// serialized by the same scheduler lock.
pub(super) fn take_pending_locked(cpu: CpuIndex) -> Result<bool, Error> {
    let state = &PREEMPTION[cpu];
    ensure_online(state)?;
    Ok(state.pending.take())
}

pub(crate) fn can_reschedule(cpu: CpuIndex) -> Result<bool, Error> {
    let state = &PREEMPTION[cpu];
    ensure_online(state)?;
    Ok(state.disable_depth.load(Ordering::Acquire) == 0
        && state.irq_depth.load(Ordering::Acquire) == 0)
}

/// Disables asynchronous preemption on the calling CPU.
///
/// Dropping the guard only restores accounting. Call
/// `scheduler::preempt_enable_and_reschedule` when the release point is also a
/// deliberate scheduling point.
pub(crate) fn disable() -> Result<PreemptionGuard, Error> {
    let cpu = current_cpu()?;
    let state = &PREEMPTION[cpu];
    ensure_online(state)?;
    increment(&state.disable_depth, Error::DisableDepthOverflow)?;
    Ok(PreemptionGuard {
        cpu,
        active: true,
        not_send: PhantomData,
    })
}

pub(crate) struct PreemptionGuard {
    cpu: CpuIndex,
    active: bool,
    not_send: PhantomData<*mut ()>,
}

impl PreemptionGuard {
    /// Releases one nesting level and reports whether preemption became
    /// enabled on this CPU.
    pub(crate) fn release(mut self) -> Result<bool, Error> {
        self.release_inner()
    }

    fn release_inner(&mut self) -> Result<bool, Error> {
        if !self.active {
            return Ok(false);
        }
        if current_cpu()? != self.cpu {
            return Err(Error::WrongCpu);
        }
        let previous = decrement(
            &PREEMPTION[self.cpu].disable_depth,
            Error::DisableDepthUnderflow,
        )?;
        self.active = false;
        Ok(previous == 1)
    }
}

impl Drop for PreemptionGuard {
    fn drop(&mut self) {
        if let Err(error) = self.release_inner() {
            crate::pr_crit!("HypeR: invalid preemption guard release: {error:?}");
            crate::arch::cpu::halt()
        }
    }
}

/// Records one normal maskable-interrupt entry on the calling CPU.
pub(crate) fn enter_irq() -> Result<IrqGuard, Error> {
    let cpu = current_cpu()?;
    let state = &PREEMPTION[cpu];
    ensure_online(state)?;
    increment(&state.irq_depth, Error::IrqDepthOverflow)?;
    Ok(IrqGuard {
        cpu,
        active: true,
        not_send: PhantomData,
    })
}

pub(crate) struct IrqGuard {
    cpu: CpuIndex,
    active: bool,
    not_send: PhantomData<*mut ()>,
}

impl IrqGuard {
    /// Completes IRQ accounting without entering the scheduler.
    pub(crate) fn complete(mut self) -> Result<(), Error> {
        self.complete_inner().map(|_| ())
    }

    fn complete_inner(&mut self) -> Result<bool, Error> {
        if !self.active {
            return Ok(false);
        }
        if current_cpu()? != self.cpu {
            return Err(Error::WrongCpu);
        }
        let previous = decrement(&PREEMPTION[self.cpu].irq_depth, Error::IrqDepthUnderflow)?;
        self.active = false;
        Ok(previous == 1)
    }
}

impl Drop for IrqGuard {
    fn drop(&mut self) {
        if let Err(error) = self.complete_inner() {
            crate::pr_crit!("HypeR: invalid IRQ accounting release: {error:?}");
            crate::arch::cpu::halt()
        }
    }
}

fn increment(counter: &AtomicU32, overflow: Error) -> Result<(), Error> {
    let mut current = counter.load(Ordering::Relaxed);
    loop {
        let next = current.checked_add(1).ok_or(overflow)?;
        match counter.compare_exchange_weak(current, next, Ordering::AcqRel, Ordering::Relaxed) {
            Ok(_) => return Ok(()),
            Err(observed) => current = observed,
        }
    }
}

/// Returns the value before decrementing it.
fn decrement(counter: &AtomicU32, underflow: Error) -> Result<u32, Error> {
    let mut current = counter.load(Ordering::Relaxed);
    loop {
        let next = current.checked_sub(1).ok_or(underflow)?;
        match counter.compare_exchange_weak(current, next, Ordering::AcqRel, Ordering::Relaxed) {
            Ok(_) => return Ok(current),
            Err(observed) => current = observed,
        }
    }
}

fn ensure_online(state: &PreemptionState) -> Result<(), Error> {
    (state.lifecycle.load(Ordering::Acquire) == ONLINE)
        .then_some(())
        .ok_or(Error::Offline)
}

fn current_cpu() -> Result<CpuIndex, Error> {
    crate::kernel::cpu::current_index().ok_or(Error::InvalidCpu)
}
