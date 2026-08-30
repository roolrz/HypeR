// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! User-thread identity, join completion, and scheduler-owned execution.

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicU64, Ordering};

use hyper::mm::FallibleArc;
use hyper::sync::InterruptSpinLock;

use super::lifecycle::{LifecycleError, TerminalReason, UserThreadLifecycle, UserThreadPhase};
use super::owner::{Process, ProcessThreadMembership};
use crate::kernel::accounting::CommittedCharge;
use crate::kernel::mm::user_space::NativeAddressSpace;
use crate::kernel::sync::Completion;
use crate::kernel::task::thread::ThreadId;

type ThreadLock<T> = InterruptSpinLock<T, crate::hal::irq::LocalMask>;

struct ThreadControl {
    lifecycle: UserThreadLifecycle,
    membership: Option<ProcessThreadMembership>,
    execution_charge: Option<CommittedCharge>,
    next_run_generation: u64,
    prepared_run: Option<RunIdentity>,
    active_run: Option<RunIdentity>,
}

struct UserThreadInner {
    process: Process,
    scheduler_id: AtomicU64,
    control: ThreadLock<ThreadControl>,
    joined: Completion,
    _metadata_charge: CommittedCharge,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RunIdentity {
    generation: u64,
    image_generation: u64,
    cpu: hyper::cpu::CpuIndex,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RunAdmissionError {
    AdmissionClosed,
    AlreadyAdmitted,
    GenerationExhausted,
    InvalidCpu,
    StaleImage,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct UserThreadSnapshot {
    pub(crate) scheduler_id: Option<ThreadId>,
    pub(crate) phase: UserThreadPhase,
    pub(crate) terminal: Option<TerminalReason>,
}

/// Observer and operation owner which outlives scheduler reclamation.
pub(crate) struct UserThread {
    inner: FallibleArc<UserThreadInner>,
}

impl UserThread {
    pub(super) const fn allocation_size() -> usize {
        FallibleArc::<UserThreadInner>::allocation_size()
    }
    pub(super) fn try_prepared(
        process: Process,
        metadata_charge: CommittedCharge,
    ) -> Result<Self, ()> {
        let inner = UserThreadInner {
            process,
            scheduler_id: AtomicU64::new(0),
            control: ThreadLock::new(ThreadControl {
                lifecycle: UserThreadLifecycle::prepared(),
                membership: None,
                execution_charge: None,
                next_run_generation: 1,
                prepared_run: None,
                active_run: None,
            }),
            joined: Completion::new(),
            _metadata_charge: metadata_charge,
        };
        FallibleArc::try_new(inner)
            .map(|inner| Self { inner })
            .map_err(|_| ())
    }

    pub(crate) fn process(&self) -> &Process {
        &self.inner.process
    }

    pub(crate) fn scheduler_id(&self) -> Option<ThreadId> {
        let raw = self.inner.scheduler_id.load(Ordering::Acquire);
        (raw != 0).then(|| ThreadId::from_process_publication(raw))
    }

    pub(crate) fn snapshot(&self) -> UserThreadSnapshot {
        self.inner.control.with(|control| UserThreadSnapshot {
            scheduler_id: self.scheduler_id(),
            phase: control.lifecycle.phase(),
            terminal: control.lifecycle.terminal(),
        })
    }

    pub(crate) fn request_stop(&self, reason: TerminalReason) -> bool {
        self.inner
            .control
            .with(|control| control.lifecycle.request_terminal(reason))
    }

    pub(crate) fn ready(&self) -> Result<bool, super::owner::ProcessError> {
        let id = self
            .scheduler_id()
            .ok_or(super::owner::ProcessError::Lifecycle(
                LifecycleError::AdmissionClosed,
            ))?;
        self.inner
            .process
            .with_run_admission(self.inner.process.image_generation(), || {
                crate::kernel::task::scheduler::ready_user_thread(id)
            })
            .map_err(|_| super::owner::ProcessError::Lifecycle(LifecycleError::AdmissionClosed))?
            .map_err(Into::into)
    }

    pub(in crate::kernel) fn mark_runnable(&self) -> Result<(), LifecycleError> {
        self.inner
            .control
            .with(|control| control.lifecycle.mark_runnable())
    }

    /// Reserves one exact architecture resume before machine preparation.
    pub(crate) fn prepare_run(
        &self,
        pin: crate::kernel::task::scheduler::UserRunGuard,
        expected_image_generation: u64,
    ) -> Result<
        PreparedUserRun,
        (
            crate::kernel::task::scheduler::UserRunGuard,
            RunAdmissionError,
        ),
    > {
        let Some(cpu) = crate::kernel::cpu::current_index() else {
            return Err((pin, RunAdmissionError::InvalidCpu));
        };
        let identity =
            match self
                .inner
                .process
                .with_run_admission(expected_image_generation, || {
                    self.inner.control.with(|control| {
                        if control.lifecycle.phase() != UserThreadPhase::Runnable {
                            return Err(RunAdmissionError::AdmissionClosed);
                        }
                        if control.prepared_run.is_some() || control.active_run.is_some() {
                            return Err(RunAdmissionError::AlreadyAdmitted);
                        }
                        let generation = control.next_run_generation;
                        control.next_run_generation = generation
                            .checked_add(1)
                            .ok_or(RunAdmissionError::GenerationExhausted)?;
                        let identity = RunIdentity {
                            generation,
                            image_generation: expected_image_generation,
                            cpu,
                        };
                        control.prepared_run = Some(identity);
                        Ok(identity)
                    })
                }) {
                Ok(Ok(identity)) => identity,
                Ok(Err(error)) | Err(error) => return Err((pin, error)),
            };
        Ok(PreparedUserRun {
            thread: self.clone(),
            identity,
            armed: true,
            pin: Some(pin),
        })
    }

    pub(crate) fn join(&self) -> Result<TerminalReason, crate::kernel::sync::Error> {
        self.inner.joined.wait()?;
        match self.snapshot().terminal {
            Some(reason) => Ok(reason),
            None => crate::hal::cpu::halt(),
        }
    }

    pub(crate) fn try_join(&self) -> Option<TerminalReason> {
        if !self.inner.joined.try_wait() {
            return None;
        }
        match self.snapshot().terminal {
            Some(reason) => Some(reason),
            None => crate::hal::cpu::halt(),
        }
    }

    pub(super) fn publish(
        &self,
        id: ThreadId,
        membership: ProcessThreadMembership,
        resource_charge: CommittedCharge,
    ) {
        if id == ThreadId::BOOTSTRAP || self.inner.scheduler_id.load(Ordering::Relaxed) != 0 {
            crate::hal::cpu::halt();
        }
        self.inner.control.with(|control| {
            if control.membership.is_some()
                || control.execution_charge.is_some()
                || control.lifecycle.publish().is_err()
            {
                crate::hal::cpu::halt();
            }
            control.membership = Some(membership);
            control.execution_charge = Some(resource_charge);
        });
        // The ID is the publication word for every control field initialized
        // above. Acquire observers of a nonzero ID see complete ownership.
        self.inner.scheduler_id.store(id.get(), Ordering::Release);
    }

    fn complete_scheduler_detach(&self) {
        let (membership, charge, terminal) = self.inner.control.with(|control| {
            if control.prepared_run.is_some() || control.active_run.is_some() {
                crate::hal::cpu::halt();
            }
            let terminal = match control.lifecycle.detach() {
                Ok(terminal) => terminal,
                Err(_) => crate::hal::cpu::halt(),
            };
            let membership = match control.membership.take() {
                Some(membership) => membership,
                None => crate::hal::cpu::halt(),
            };
            let charge = match control.execution_charge.take() {
                Some(charge) => charge,
                None => crate::hal::cpu::halt(),
            };
            (membership, charge, terminal)
        });
        membership.detach(terminal);
        drop(charge);
        if self.inner.joined.complete_all().is_err() {
            crate::hal::cpu::halt();
        }
    }
}

impl Clone for UserThread {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

/// Scheduler-owned payload for a runnable native user context.
///
/// The address-space clone is dropped before Process membership completion,
/// so observing zero active Process Threads also proves that no scheduler
/// payload can retain the machine root.
pub(crate) struct UserExecution {
    thread: UserThread,
    address_space: Option<FallibleArc<NativeAddressSpace>>,
    context: UnsafeCell<crate::hal::user::UserContext>,
    armed: bool,
}

impl UserExecution {
    pub(super) const fn allocation_size() -> usize {
        core::mem::size_of::<Self>()
    }

    pub(super) fn try_new(
        thread: UserThread,
        address_space: FallibleArc<NativeAddressSpace>,
        context: crate::hal::user::UserContext,
    ) -> Result<alloc::boxed::Box<Self>, ()> {
        hyper::mm::try_box(Self {
            thread,
            address_space: Some(address_space),
            context: UnsafeCell::new(context),
            armed: false,
        })
        .map_err(|_| ())
    }

    pub(crate) const fn thread(&self) -> &UserThread {
        &self.thread
    }

    pub(crate) fn address_space(&self) -> &NativeAddressSpace {
        match self.address_space.as_ref() {
            Some(address_space) => address_space,
            None => crate::hal::cpu::halt(),
        }
    }

    /// Returns the stopped register owner to the fixed native-user trampoline.
    ///
    /// # Safety
    ///
    /// The caller must own the scheduler's current-user pin and one admitted
    /// run generation. No other code may access the context until the returned
    /// architecture capability is consumed or discarded.
    pub(crate) unsafe fn context_ptr(&self) -> *mut crate::hal::user::UserContext {
        self.context.get()
    }

    pub(crate) fn stop_requested(&self) -> bool {
        self.thread.snapshot().phase == UserThreadPhase::StopRequested
    }

    pub(crate) fn arm_after_process_publication(&mut self) {
        if self.armed {
            crate::hal::cpu::halt();
        }
        self.armed = true;
    }

    /// Completes scheduler-detach ownership outside the scheduler lock.
    pub(crate) fn complete_detach(mut self) {
        if !self.armed {
            crate::hal::cpu::halt();
        }
        self.armed = false;
        drop(self.address_space.take());
        self.thread.complete_scheduler_detach();
    }
}

impl Drop for UserExecution {
    fn drop(&mut self) {
        if self.armed {
            // Reclamation must call complete_detach after releasing scheduler
            // ownership. Drop cannot acquire Process/completion lock graphs.
            crate::hal::cpu::halt();
        }
    }
}

#[must_use = "publish or abort the prepared user run"]
pub(crate) struct PreparedUserRun {
    thread: UserThread,
    identity: RunIdentity,
    armed: bool,
    pin: Option<crate::kernel::task::scheduler::UserRunGuard>,
}

impl PreparedUserRun {
    pub(crate) fn pin(&self) -> &crate::kernel::task::scheduler::UserRunGuard {
        match self.pin.as_ref() {
            Some(pin) => pin,
            None => crate::hal::cpu::halt(),
        }
    }

    /// Publishes one CPU-affine run only after HAL preparation succeeds.
    pub(crate) fn commit(
        mut self,
    ) -> Result<
        ActiveUserRun,
        (
            crate::kernel::task::scheduler::UserRunGuard,
            RunAdmissionError,
        ),
    > {
        if crate::kernel::cpu::current_index() != Some(self.identity.cpu) {
            return Err((self.abort_inner(), RunAdmissionError::InvalidCpu));
        }
        let publication =
            self.thread
                .inner
                .process
                .with_run_admission(self.identity.image_generation, || {
                    self.thread.inner.control.with(|control| {
                        if control.lifecycle.phase() != UserThreadPhase::Runnable {
                            return Err(RunAdmissionError::AdmissionClosed);
                        }
                        if control.prepared_run != Some(self.identity)
                            || control.active_run.is_some()
                        {
                            return Err(RunAdmissionError::AlreadyAdmitted);
                        }
                        control.prepared_run = None;
                        control.active_run = Some(self.identity);
                        Ok(())
                    })
                });
        if let Err(error) | Ok(Err(error)) = publication {
            return Err((self.abort_inner(), error));
        }
        self.armed = false;
        Ok(ActiveUserRun {
            thread: self.thread.clone(),
            identity: self.identity,
            armed: true,
            pin: self.pin.take(),
        })
    }

    pub(crate) fn abort(mut self) -> crate::kernel::task::scheduler::UserRunGuard {
        self.abort_inner()
    }

    fn abort_inner(&mut self) -> crate::kernel::task::scheduler::UserRunGuard {
        self.thread.inner.control.with(|control| {
            if control.prepared_run != Some(self.identity) {
                crate::hal::cpu::halt();
            }
            control.prepared_run = None;
        });
        self.armed = false;
        match self.pin.take() {
            Some(pin) => pin,
            None => crate::hal::cpu::halt(),
        }
    }
}

impl Drop for PreparedUserRun {
    fn drop(&mut self) {
        if self.armed {
            crate::hal::cpu::halt();
        }
    }
}

#[must_use = "complete the active user run before scheduler detach"]
pub(crate) struct ActiveUserRun {
    thread: UserThread,
    identity: RunIdentity,
    armed: bool,
    pin: Option<crate::kernel::task::scheduler::UserRunGuard>,
}

impl ActiveUserRun {
    pub(crate) const fn generation(&self) -> u64 {
        self.identity.generation
    }

    pub(crate) fn binding(&self) -> hyper::hal::user::UserRunBinding {
        match hyper::hal::user::UserRunBinding::new(
            self.thread.scheduler_id().map_or(0, ThreadId::get),
            self.identity.image_generation,
            self.identity.generation,
        ) {
            Some(binding) => binding,
            None => crate::hal::cpu::halt(),
        }
    }

    pub(crate) fn pin(&self) -> &crate::kernel::task::scheduler::UserRunGuard {
        match self.pin.as_ref() {
            Some(pin) => pin,
            None => crate::hal::cpu::halt(),
        }
    }

    /// Ends machine-active CPU affinity after architecture publication and
    /// translation have both been closed. The durable generation remains
    /// occupied across syscall dispatch, blocking, and migration.
    pub(crate) fn stop_after_machine_exit(
        mut self,
        proof: crate::kernel::mm::user_space::StoppedNativeRun,
    ) -> (StoppedUserRun, crate::kernel::task::scheduler::UserRunGuard) {
        if crate::kernel::cpu::current_index() != Some(self.identity.cpu) {
            crate::hal::cpu::halt();
        }
        if proof.binding() != self.binding() {
            crate::hal::cpu::halt();
        }
        self.armed = false;
        let pin = match self.pin.take() {
            Some(pin) => pin,
            None => crate::hal::cpu::halt(),
        };
        (
            StoppedUserRun {
                thread: self.thread.clone(),
                identity: self.identity,
                armed: true,
            },
            pin,
        )
    }
}

impl Drop for ActiveUserRun {
    fn drop(&mut self) {
        if self.armed {
            crate::hal::cpu::halt();
        }
    }
}

/// Durable ownership of one stopped architecture generation.
///
/// This token is deliberately not CPU-affine. A syscall may block and resume
/// on another CPU, but architecture return ownership must be resolved before
/// this logical generation is acknowledged.
#[must_use = "stopped user generation must follow architecture completion"]
pub(crate) struct StoppedUserRun {
    thread: UserThread,
    identity: RunIdentity,
    armed: bool,
}

impl StoppedUserRun {
    pub(crate) fn binding(&self) -> hyper::hal::user::UserRunBinding {
        match hyper::hal::user::UserRunBinding::new(
            match self.thread.scheduler_id() {
                Some(id) => id.get(),
                None => 0,
            },
            self.identity.image_generation,
            self.identity.generation,
        ) {
            Some(binding) => binding,
            None => crate::hal::cpu::halt(),
        }
    }

    /// Acknowledges that the architecture return capability was consumed.
    pub(crate) fn acknowledge_architecture_exit(mut self) {
        self.thread.inner.control.with(|control| {
            if control.active_run != Some(self.identity) {
                crate::hal::cpu::halt();
            }
            control.active_run = None;
        });
        self.armed = false;
    }
}

impl Drop for StoppedUserRun {
    fn drop(&mut self) {
        if self.armed {
            crate::hal::cpu::halt();
        }
    }
}
