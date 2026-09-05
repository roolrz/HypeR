// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Grouped process stop and membership ownership.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use hyper::mm::FallibleArc;
use hyper::sync::InterruptSpinLock;

use super::lifecycle::StopDispatchProgress;
use super::{Process, ProcessStopReport, TerminalReason};
use crate::kernel::accounting::{
    CommittedCharge, ResourceAmount, ResourceDomain, ResourceDomainId, ResourceError, ResourceKind,
};

type GroupLock<T> = InterruptSpinLock<T, crate::hal::irq::LocalMask>;

static NEXT_TASK_GROUP_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TaskGroupId(u64);

impl TaskGroupId {
    pub(crate) const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GroupPhase {
    Active,
    Stopping,
    Retired,
}

struct MemberRecord {
    active: AtomicBool,
    process: GroupLock<Option<Process>>,
    next: GroupLock<Option<FallibleArc<MemberRecord>>>,
    _metadata_charge: CommittedCharge,
}

struct GroupState {
    phase: GroupPhase,
    stop_generation: u64,
    pending_members: usize,
    active_members: usize,
    head: Option<FallibleArc<MemberRecord>>,
    _metadata_charge: CommittedCharge,
}

struct TaskGroupInner {
    id: TaskGroupId,
    domain_id: ResourceDomainId,
    domain: ResourceDomain,
    object_published: AtomicBool,
    state: GroupLock<GroupState>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TaskGroupError {
    Allocation,
    CounterOverflow,
    GenerationExhausted,
    Inactive,
    MembersRemain,
    Resource(ResourceError),
}

impl From<ResourceError> for TaskGroupError {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TaskGroupStopReport {
    pub(crate) generation: u64,
    pub(crate) newly_requested: bool,
    pub(crate) dispatched_members: usize,
    pub(crate) incomplete_members: usize,
}

/// Shared grouped-lifecycle authority.
///
/// Active membership deliberately forms a cycle through each Process. Only
/// acknowledged Process retirement removes that edge. Abandoning teardown
/// therefore retains owners instead of freeing an address space still usable
/// by a scheduler context.
pub(crate) struct TaskGroup {
    inner: FallibleArc<TaskGroupInner>,
}

impl TaskGroup {
    pub(crate) fn try_new(domain: &ResourceDomain) -> Result<Self, TaskGroupError> {
        let amount = ResourceAmount::ZERO
            .with(ResourceKind::KernelObjects, 1)
            .with(
                ResourceKind::KernelMemoryBytes,
                u64::try_from(FallibleArc::<TaskGroupInner>::allocation_size())
                    .map_err(|_| TaskGroupError::Allocation)?,
            );
        let metadata_charge = domain.reserve(amount)?.commit();
        let id = allocate_group_id()?;
        let inner = TaskGroupInner {
            id,
            domain_id: domain.id(),
            domain: domain.clone(),
            object_published: AtomicBool::new(false),
            state: GroupLock::new(GroupState {
                phase: GroupPhase::Active,
                stop_generation: 0,
                pending_members: 0,
                active_members: 0,
                head: None,
                _metadata_charge: metadata_charge,
            }),
        };
        Ok(Self {
            inner: FallibleArc::try_new(inner).map_err(|_| TaskGroupError::Allocation)?,
        })
    }

    pub(crate) fn id(&self) -> TaskGroupId {
        self.inner.id
    }

    pub(crate) fn domain_id(&self) -> ResourceDomainId {
        self.inner.domain_id
    }

    pub(crate) fn resource_domain(&self) -> ResourceDomain {
        self.inner.domain.clone()
    }

    pub(super) fn claim_object_publication(&self) -> bool {
        self.inner
            .object_published
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    pub(super) fn abort_object_publication(&self) {
        if self
            .inner
            .object_published
            .compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            group_invariant_violation();
        }
    }

    pub(super) fn prepare_membership(&self) -> Result<PreparedTaskGroupMembership, TaskGroupError> {
        let record_charge_bytes = u64::try_from(FallibleArc::<MemberRecord>::allocation_size())
            .map_err(|_| TaskGroupError::Allocation)?;
        let record_charge = self
            .inner
            .domain
            .reserve(
                ResourceAmount::ZERO
                    .with(ResourceKind::KernelObjects, 1)
                    .with(ResourceKind::KernelMemoryBytes, record_charge_bytes),
            )?
            .commit();
        let record = FallibleArc::try_new(MemberRecord {
            active: AtomicBool::new(false),
            process: GroupLock::new(None),
            next: GroupLock::new(None),
            _metadata_charge: record_charge,
        })
        .map_err(|_| TaskGroupError::Allocation)?;
        self.inner.state.with(|state| {
            if state.phase != GroupPhase::Active {
                return Err(TaskGroupError::Inactive);
            }
            state.pending_members = state
                .pending_members
                .checked_add(1)
                .ok_or(TaskGroupError::CounterOverflow)?;
            Ok(())
        })?;
        Ok(PreparedTaskGroupMembership {
            group: self.clone(),
            record: Some(record),
        })
    }

    /// Publishes one durable stop generation, then visits every member without
    /// retaining the group lock across Process or scheduler operations.
    pub(crate) fn request_stop(&self) -> Result<TaskGroupStopReport, TaskGroupError> {
        let (generation, newly_requested, pending_members) =
            self.inner.state.with(|state| match state.phase {
                GroupPhase::Active => {
                    let generation = state
                        .stop_generation
                        .checked_add(1)
                        .ok_or(TaskGroupError::GenerationExhausted)?;
                    state.stop_generation = generation;
                    state.phase = GroupPhase::Stopping;
                    Ok((generation, true, state.pending_members))
                }
                GroupPhase::Stopping => Ok((state.stop_generation, false, state.pending_members)),
                GroupPhase::Retired => Err(TaskGroupError::Inactive),
            })?;

        let mut current = self.inner.state.with(|state| state.head.clone());
        let mut dispatched_members = 0usize;
        let mut dispatch = StopDispatchProgress::new(pending_members);
        while let Some(record) = current {
            let process = record.process.with(|slot| slot.clone());
            current = record.next.with(|next| next.clone());
            let Some(process) = process else {
                continue;
            };
            dispatched_members = dispatched_members.saturating_add(1);
            let ProcessStopReport {
                dispatch_complete, ..
            } = process.request_stop(TerminalReason::TaskGroupStop { generation });
            dispatch.observe(dispatch_complete);
        }
        Ok(TaskGroupStopReport {
            generation,
            newly_requested,
            dispatched_members,
            incomplete_members: dispatch.incomplete(),
        })
    }

    pub(crate) fn finish_retirement(&self) -> Result<(), TaskGroupError> {
        let mut current = self.inner.state.with(|state| {
            if state.phase == GroupPhase::Retired {
                return Err(TaskGroupError::Inactive);
            }
            if state.phase != GroupPhase::Stopping
                || state.pending_members != 0
                || state.active_members != 0
            {
                return Err(TaskGroupError::MembersRemain);
            }
            state.phase = GroupPhase::Retired;
            Ok(state.head.take())
        })?;
        // Break the retained list iteratively so retirement has bounded stack
        // use even after a process created many short-lived children.
        while let Some(record) = current {
            current = record.next.with(Option::take);
            drop(record);
        }
        Ok(())
    }
}

impl Clone for TaskGroup {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

pub(super) struct PreparedTaskGroupMembership {
    group: TaskGroup,
    record: Option<FallibleArc<MemberRecord>>,
}

impl PreparedTaskGroupMembership {
    pub(super) fn publish(mut self, process: Process) -> (TaskGroupMembership, Option<u64>) {
        let record = match self.record.take() {
            Some(record) => record,
            None => group_invariant_violation(),
        };
        record.process.with(|slot| *slot = Some(process));
        self.group.inner.state.with(|state| {
            if state.phase == GroupPhase::Retired || state.pending_members == 0 {
                group_invariant_violation();
            }
            record.next.with(|next| *next = state.head.clone());
            record.active.store(true, Ordering::Relaxed);
            state.head = Some(record.clone());
            state.pending_members -= 1;
            state.active_members = match state.active_members.checked_add(1) {
                Some(count) => count,
                None => group_invariant_violation(),
            };
        });
        let stop_generation =
            self.group.inner.state.with(|state| {
                (state.phase == GroupPhase::Stopping).then_some(state.stop_generation)
            });
        (
            TaskGroupMembership {
                group: self.group.clone(),
                record: Some(record),
            },
            stop_generation,
        )
    }
}

impl Drop for PreparedTaskGroupMembership {
    fn drop(&mut self) {
        if self.record.is_none() {
            return;
        }
        self.group.inner.state.with(|state| {
            state.pending_members = match state.pending_members.checked_sub(1) {
                Some(count) => count,
                None => group_invariant_violation(),
            };
        });
    }
}

pub(super) struct TaskGroupMembership {
    group: TaskGroup,
    record: Option<FallibleArc<MemberRecord>>,
}

impl TaskGroupMembership {
    pub(super) fn retire(mut self) {
        let record = match self.record.take() {
            Some(record) => record,
            None => group_invariant_violation(),
        };
        if !record.active.swap(false, Ordering::AcqRel) {
            group_invariant_violation();
        }
        let process = record.process.with(Option::take);
        let detached_record = self.group.inner.state.with(|state| {
            state.active_members = match state.active_members.checked_sub(1) {
                Some(count) => count,
                None => group_invariant_violation(),
            };
            unlink_member_record(state, &record)
        });
        drop(process);
        drop(detached_record);
        drop(record);
    }
}

fn unlink_member_record(
    state: &mut GroupState,
    target: &FallibleArc<MemberRecord>,
) -> FallibleArc<MemberRecord> {
    let mut current = state.head.clone();
    let mut previous: Option<FallibleArc<MemberRecord>> = None;
    while let Some(record) = current {
        let next = record.next.with(|next| next.clone());
        if core::ptr::eq::<MemberRecord>(&*record, &**target) {
            if let Some(previous) = previous {
                previous.next.with(|link| *link = next);
            } else {
                state.head = next;
            }
            // A stop traversal may hold this removed record without the group
            // lock. Preserve its published successor until every such reader
            // releases the record, or the traversal could silently truncate.
            return record;
        }
        previous = Some(record);
        current = next;
    }
    group_invariant_violation()
}

impl Drop for TaskGroupMembership {
    fn drop(&mut self) {
        if self.record.is_some() {
            // A live group edge pins the Process. Silent rollback here would
            // free grouped policy without an acknowledged process retirement.
            group_invariant_violation();
        }
    }
}

fn allocate_group_id() -> Result<TaskGroupId, TaskGroupError> {
    let mut current = NEXT_TASK_GROUP_ID.load(Ordering::Relaxed);
    loop {
        if current == 0 {
            return Err(TaskGroupError::CounterOverflow);
        }
        let next = current
            .checked_add(1)
            .ok_or(TaskGroupError::CounterOverflow)?;
        match NEXT_TASK_GROUP_ID.compare_exchange_weak(
            current,
            next,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => return Ok(TaskGroupId(current)),
            Err(observed) => current = observed,
        }
    }
}

#[cold]
fn group_invariant_violation() -> ! {
    crate::hal::cpu::halt()
}
