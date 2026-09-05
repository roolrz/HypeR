// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Stable VM-owned vCPU notification endpoints.
//!
//! Scheduler Thread state is the sole authority for execution location. This
//! object retains only vCPU identity, its immutable Thread binding, and saved
//! interrupt-model work which must survive descheduling and migration.

use hyper::sync::InterruptSpinLock;
use hyper::sync::PublishedOnce;

use super::reconcile::ReconcilePublication;
use crate::kernel::task::thread::ThreadId;

pub(super) struct VcpuEndpoint {
    // Drop the timer reservation before every field reachable by its raw
    // callback context. An armed reservation fail-stops here, before endpoint
    // condition state could be destroyed underneath an in-flight callback.
    timer: crate::kernel::time::ReservedTimer,
    id: u32,
    thread: PublishedOnce<ThreadId>,
    reconcile: ReconcilePublication,
    state: super::endpoint_state::EndpointState,
    wait: InterruptSpinLock<WaitState, crate::hal::irq::LocalMask>,
    waiters: crate::kernel::task::WaitQueue,
}

struct WaitState {
    publication: super::endpoint_wait::WaitPublication<crate::kernel::task::WaitTicket>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct WaitTicket {
    epoch: u64,
}

pub(super) enum PreparedWait<'a> {
    Changed,
    Park(EndpointPark<'a>),
    Completed(crate::kernel::task::WaitOutcome),
}

pub(super) struct EndpointPark<'a> {
    endpoint: &'a VcpuEndpoint,
    waiter: crate::kernel::task::WaitTicket,
    park: crate::kernel::task::scheduler::ParkToken,
}

impl EndpointPark<'_> {
    pub(super) fn complete(self) -> crate::kernel::task::WaitOutcome {
        let outcome = crate::kernel::task::scheduler::complete_park(self.park);
        self.endpoint.complete_wait(self.waiter, outcome);
        outcome
    }
}

enum LockedWait {
    Changed,
    Park {
        commit: crate::kernel::task::scheduler::ParkCommit,
        waiter: crate::kernel::task::WaitTicket,
    },
    Completed(crate::kernel::task::WaitOutcome),
}

impl VcpuEndpoint {
    pub(super) fn try_new(id: u32) -> Result<Self, crate::kernel::time::Error> {
        Ok(Self {
            timer: crate::kernel::time::ReservedTimer::try_new()?,
            id,
            thread: PublishedOnce::new(),
            reconcile: ReconcilePublication::new(),
            state: super::endpoint_state::EndpointState::unbound(),
            wait: InterruptSpinLock::new(WaitState {
                publication: super::endpoint_wait::WaitPublication::new(),
            }),
            waiters: crate::kernel::task::WaitQueue::new(),
        })
    }

    pub(super) fn bind_thread(&self, thread: ThreadId) -> Result<(), ()> {
        if thread == ThreadId::BOOTSTRAP {
            return Err(());
        }
        self.thread.publish(thread).map_err(|_| ())?;
        self.state.publish_bound().map_err(|_| ())?;
        Ok(())
    }

    pub(super) fn thread(&self) -> Option<ThreadId> {
        self.thread.get().copied()
    }

    pub(super) fn publish_reconcile(&self) -> Result<(), super::endpoint_state::StateError> {
        // Close and publication use separate atomics. A producer which already
        // observed Open may publish dirty after close; that work is inert and
        // memory-safe because the installed VM/controller remain strongly
        // owned. Future removal must stop and drain backend producers before
        // reclaiming endpoint storage; close alone is not quiescence.
        self.state.ensure_open()?;
        self.reconcile.publish();
        Ok(())
    }

    pub(super) fn signal_waiter(&self) {
        self.wait.with(|state| {
            let ticket = match state.publication.signal() {
                Ok(ticket) => ticket,
                Err(super::endpoint_wait::Error::EpochExhausted) => {
                    crate::kernel::crash::fatal(format_args!("HypeR: vCPU wait epoch exhausted"))
                }
                Err(super::endpoint_wait::Error::WaiterAlreadyRegistered) => {
                    crate::kernel::crash::fatal(format_args!(
                        "HypeR: invalid endpoint signal state"
                    ))
                }
            };
            let Some(ticket) = ticket else {
                return;
            };
            let resolved =
                match crate::kernel::task::scheduler::notify_registered_fair_boundary(ticket) {
                    Ok(resolved) => resolved,
                    Err(error) => crate::kernel::crash::fatal(format_args!(
                        "HypeR: vCPU endpoint could not resolve its exact wait: {error:?}"
                    )),
                };
            // Cancellation may have won scheduler arbitration after the
            // endpoint ticket was registered. The signal still owns and
            // consumes that exact endpoint ticket; `EndpointPark::complete`
            // performs the matching resumed-side cleanup.
            let _notification_won = resolved.won;
        })
    }

    pub(super) fn wait_ticket(&self) -> WaitTicket {
        self.wait.with(|state| WaitTicket {
            epoch: state.publication.snapshot(),
        })
    }

    pub(super) fn arm_timer(
        &self,
        deadline: u64,
    ) -> Result<crate::kernel::time::ArmedReservedTimer<'_>, crate::kernel::time::Error> {
        self.timer.arm(
            deadline,
            notify_timer_waiter,
            core::ptr::from_ref(self).expose_provenance(),
        )
    }

    pub(super) fn prepare_wait(
        &self,
        ticket: WaitTicket,
    ) -> Result<PreparedWait<'_>, crate::kernel::task::scheduler::Error> {
        use crate::kernel::task::{WaitMobility, scheduler};

        let registration = scheduler::begin_wait(WaitMobility::Migratable)?;
        // SAFETY: the retained local-mask state is either dropped on this CPU
        // or transferred immediately into the committed scheduler handoff.
        let (prepared, interrupt_mask) = unsafe {
            self.wait.with_mask_retained(|state| {
                match state
                    .publication
                    .register(ticket.epoch, registration.ticket())
                {
                    Ok(super::endpoint_wait::Registration::Changed) => {
                        match scheduler::finish_wait(registration)? {
                            None => Ok(LockedWait::Changed),
                            Some(_) => Err(scheduler::Error::InvalidWaitRegistration),
                        }
                    }
                    Ok(super::endpoint_wait::Registration::Registered) => {
                        let waiter = registration.ticket();
                        match scheduler::prepare_registered_park_locked(&self.waiters, registration)
                        {
                            Ok(scheduler::PrepareWait::Park(commit)) => {
                                Ok(LockedWait::Park { commit, waiter })
                            }
                            Ok(scheduler::PrepareWait::Completed(outcome)) => {
                                complete_publication(state, waiter, outcome);
                                Ok(LockedWait::Completed(outcome))
                            }
                            Err(error) => {
                                complete_publication(
                                    state,
                                    waiter,
                                    crate::kernel::task::WaitOutcome::Cancelled,
                                );
                                Err(error)
                            }
                        }
                    }
                    Err(_) => {
                        crate::kernel::crash::fatal(format_args!(
                            "HypeR: vCPU endpoint published two wait registrations"
                        ));
                    }
                }
            })
        };
        match prepared? {
            LockedWait::Park { commit, waiter } => Ok(PreparedWait::Park(EndpointPark {
                endpoint: self,
                waiter,
                park: scheduler::retain_park_mask(commit, interrupt_mask),
            })),
            LockedWait::Changed => {
                drop(interrupt_mask);
                Ok(PreparedWait::Changed)
            }
            LockedWait::Completed(outcome) => {
                drop(interrupt_mask);
                Ok(PreparedWait::Completed(outcome))
            }
        }
    }

    fn complete_wait(
        &self,
        waiter: crate::kernel::task::WaitTicket,
        outcome: crate::kernel::task::WaitOutcome,
    ) {
        self.wait
            .with(|state| complete_publication(state, waiter, outcome));
    }

    pub(super) fn take_reconcile(&self) -> bool {
        self.reconcile.take()
    }

    pub(super) fn restore_reconcile(&self) {
        self.reconcile.restore();
    }

    pub(super) fn reconcile_pending(&self) -> bool {
        self.reconcile.pending()
    }

    pub(super) fn is_valid_for(&self, id: u32) -> bool {
        self.id == id
    }

    pub(super) fn close(
        &self,
        expected_thread: ThreadId,
        reason: super::endpoint_state::TerminalReason,
    ) -> Result<super::endpoint_state::GuestCloseOutcome, super::endpoint_state::TransitionError>
    {
        if self.thread() != Some(expected_thread) {
            return Err(super::endpoint_state::TransitionError::Corrupt);
        }
        self.state.close_guest(reason)
    }

    pub(super) fn request_stop(
        &self,
        expected_thread: ThreadId,
        reason: super::endpoint_state::AdministrativeStopReason,
    ) -> Result<super::endpoint_state::StopRequestOutcome, super::endpoint_state::StateError> {
        if self.thread() != Some(expected_thread) {
            return Err(super::endpoint_state::StateError::Corrupt);
        }
        let outcome = self.state.request_stop(reason)?;
        if outcome == super::endpoint_state::StopRequestOutcome::Published {
            // Stop and interrupt delivery share the exact endpoint wait ticket.
            // A suspended WFI continuation is made runnable so it can unwind
            // its EndpointPark and ArmedReservedTimer ownership normally.
            self.signal_waiter();
        }
        Ok(outcome)
    }

    pub(super) fn stop_requested(
        &self,
        expected_thread: ThreadId,
    ) -> Result<
        Option<super::endpoint_state::AdministrativeStopReason>,
        super::endpoint_state::StateError,
    > {
        if self.thread() != Some(expected_thread) {
            return Err(super::endpoint_state::StateError::Corrupt);
        }
        match self.state.lifecycle()? {
            super::endpoint_state::Lifecycle::StopRequested(reason) => Ok(Some(reason)),
            _ => Ok(None),
        }
    }

    pub(super) fn publish_hardware_detached(
        &self,
        expected_thread: ThreadId,
        reason: super::endpoint_state::AdministrativeStopReason,
    ) -> Result<(), super::endpoint_state::TransitionError> {
        if self.thread() != Some(expected_thread) {
            return Err(super::endpoint_state::TransitionError::Corrupt);
        }
        self.state.publish_hardware_detached(reason)
    }

    pub(super) fn publish_reaped(
        &self,
        expected_thread: ThreadId,
        reason: super::endpoint_state::ClosureReason,
    ) -> Result<(), super::endpoint_state::TransitionError> {
        if self.thread() != Some(expected_thread) {
            return Err(super::endpoint_state::TransitionError::Corrupt);
        }
        self.state.publish_reaped(reason)
    }

    pub(super) fn lifecycle(
        &self,
    ) -> Result<super::endpoint_state::Lifecycle, super::endpoint_state::StateError> {
        self.state.lifecycle()
    }

    pub(super) fn thread_absence_is_terminal(
        &self,
    ) -> Result<bool, super::endpoint_state::StateError> {
        self.state.thread_absence_is_terminal()
    }
}

fn notify_timer_waiter(context: usize) {
    let pointer = core::ptr::with_exposed_provenance::<VcpuEndpoint>(context);
    // SAFETY: the armed timer borrows this stable installed endpoint until its
    // exact callback completes or cancellation returns its reserved node.
    let endpoint = unsafe { &*pointer };
    endpoint.signal_waiter();
}

fn complete_publication(
    state: &mut WaitState,
    waiter: crate::kernel::task::WaitTicket,
    outcome: crate::kernel::task::WaitOutcome,
) {
    if let Err(error) = state.publication.complete(
        waiter,
        outcome == crate::kernel::task::WaitOutcome::Notified,
    ) {
        crate::kernel::crash::fatal(format_args!(
            "HypeR: vCPU endpoint wait completion mismatch: {error:?}"
        ));
    }
}

#[cfg(feature = "kernel-self-test")]
mod self_test {
    use hyper::cpu::CpuIndex;
    use hyper::sync::PublishedOnce;
    use hyper::sync::atomic::{AtomicUsize, Ordering};

    use super::{PreparedWait, VcpuEndpoint};
    use crate::kernel::sync::Semaphore;
    use crate::kernel::task::WaitOutcome;
    use crate::kernel::task::scheduler::{self, CpuMask};

    static ENDPOINT: PublishedOnce<VcpuEndpoint> = PublishedOnce::new();
    static PHASE: AtomicUsize = AtomicUsize::new(0);
    static OUTCOMES: AtomicUsize = AtomicUsize::new(0);
    static DONE: Semaphore = Semaphore::new(0);

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) enum Error {
        Allocation,
        Scheduler(scheduler::Error),
        Sleep(crate::kernel::task::SleepError),
        State(usize),
    }

    impl From<scheduler::Error> for Error {
        fn from(error: scheduler::Error) -> Self {
            Self::Scheduler(error)
        }
    }

    impl From<crate::kernel::task::SleepError> for Error {
        fn from(error: crate::kernel::task::SleepError) -> Self {
            Self::Sleep(error)
        }
    }

    pub(crate) fn run() -> Result<(), Error> {
        let prepared_endpoint = VcpuEndpoint::try_new(0).map_err(|_| Error::Allocation)?;
        ENDPOINT
            .publish(prepared_endpoint)
            .map_err(|_| Error::State(1))?;
        PHASE.store(0, Ordering::Release);
        OUTCOMES.store(0, Ordering::Release);

        let worker = scheduler::kthread_create_with_affinity(
            "wait/vcpu-endpoint",
            worker,
            0,
            CpuMask::single(CpuIndex::BOOT),
        )?;
        scheduler::thread_ready(worker)?;

        wait_for_phase(1)?;
        endpoint()?.signal_waiter();
        wait_for_phase(2)?;
        if !scheduler::cancel_waiter(&endpoint()?.waiters, worker)? {
            return Err(Error::State(2));
        }
        // The endpoint still owns its exact ticket. This later signal must be
        // a benign scheduler loser and must let resumed cleanup consume the
        // already-cleared endpoint publication.
        endpoint()?.signal_waiter();
        wait_for_done()?;
        if OUTCOMES.load(Ordering::Acquire) != 0b11 {
            return Err(Error::State(3));
        }
        Ok(())
    }

    fn endpoint() -> Result<&'static VcpuEndpoint, Error> {
        ENDPOINT.get().ok_or(Error::State(4))
    }

    fn wait_for_phase(expected: usize) -> Result<(), Error> {
        if crate::kernel::task::wait_for_test_progress(
            crate::kernel::task::TEST_PROGRESS_TIMEOUT_NS,
            || Ok::<_, Error>(PHASE.load(Ordering::Acquire) == expected),
        )? {
            Ok(())
        } else {
            Err(Error::State(5))
        }
    }

    fn wait_for_done() -> Result<(), Error> {
        if crate::kernel::task::wait_for_test_progress(
            crate::kernel::task::TEST_PROGRESS_TIMEOUT_NS,
            || Ok::<_, Error>(DONE.try_acquire()),
        )? {
            Ok(())
        } else {
            Err(Error::State(6))
        }
    }

    extern "C" fn worker(_argument: usize) {
        for (round, expected) in [WaitOutcome::Notified, WaitOutcome::Cancelled]
            .into_iter()
            .enumerate()
        {
            let outcome = match wait_once(round + 1) {
                Ok(outcome) => outcome,
                Err(code) => {
                    OUTCOMES.store(0x100 + code, Ordering::Release);
                    let _ = DONE.release();
                    return;
                }
            };
            if outcome != expected {
                OUTCOMES.store(0x200 + round, Ordering::Release);
                let _ = DONE.release();
                return;
            }
            OUTCOMES.fetch_or(1 << round, Ordering::AcqRel);
        }
        if DONE.release().is_err() {
            OUTCOMES.store(0x300, Ordering::Release);
        }
    }

    fn wait_once(phase: usize) -> Result<WaitOutcome, usize> {
        let endpoint = endpoint().map_err(|_| 1usize)?;
        let ticket = endpoint.wait_ticket();
        match endpoint.prepare_wait(ticket).map_err(|_| 2usize)? {
            PreparedWait::Changed => Err(3),
            PreparedWait::Completed(outcome) => Ok(outcome),
            PreparedWait::Park(park) => {
                PHASE.store(phase, Ordering::Release);
                Ok(park.complete())
            }
        }
    }
}

#[cfg(feature = "kernel-self-test")]
pub(crate) use self_test::{Error as WaitSelfTestError, run as run_wait_self_test};
