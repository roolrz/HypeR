// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Hierarchical multi-resource accounting with owned charge transactions.

use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

use hyper::mm::{AllocationError, FallibleArc};
use hyper::sync::InterruptSpinLock;

#[cfg(not(test))]
type DomainLock<T> = InterruptSpinLock<T, crate::hal::irq::LocalMask>;

#[cfg(test)]
struct TestInterruptMask;

#[cfg(test)]
impl hyper::hal::interrupt::InterruptMask for TestInterruptMask {
    type State = ();

    fn save_and_disable() -> Self::State {}

    fn restore(_: Self::State) {}

    fn wait_for_lock_owner() {
        std::thread::yield_now();
    }
}

#[cfg(test)]
type DomainLock<T> = InterruptSpinLock<T, TestInterruptMask>;

/// Maximum root-to-leaf domain count, including both endpoints.
///
/// A finite depth keeps reservation, commit, and Drop rollback on a bounded
/// kernel stack while preserving allocation-free transaction completion.
const MAX_DOMAIN_DEPTH: usize = 32;
const RESOURCE_KIND_COUNT: usize = 19;

static NEXT_DOMAIN_ID: AtomicU64 = AtomicU64::new(1);

/// Stable diagnostic identity which does not grant authority over a domain.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct ResourceDomainId(u64);

impl ResourceDomainId {
    fn allocate() -> Result<Self, ResourceError> {
        let mut current = NEXT_DOMAIN_ID.load(Ordering::Relaxed);
        loop {
            if current == 0 {
                return Err(ResourceError::DomainIdExhausted);
            }
            let Some(next) = current.checked_add(1) else {
                return Err(ResourceError::DomainIdExhausted);
            };
            match NEXT_DOMAIN_ID.compare_exchange_weak(
                current,
                next,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Ok(Self(current)),
                Err(observed) => current = observed,
            }
        }
    }

    pub(crate) const fn get(self) -> u64 {
        self.0
    }
}

/// Independently limited resource dimensions.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub(crate) enum ResourceKind {
    KernelMemoryBytes,
    Processes,
    Threads,
    Handles,
    KernelObjects,
    CommittedPages,
    PinnedPages,
    GuestPages,
    IpcMessages,
    IpcBytes,
    IpcHandles,
    Subscriptions,
    Timers,
    VirtualMachines,
    VirtualCpus,
    DeviceLeases,
    DmaMappings,
    UserAddressSpaces,
    UserMappings,
}

impl ResourceKind {
    const ALL: [Self; RESOURCE_KIND_COUNT] = [
        Self::KernelMemoryBytes,
        Self::Processes,
        Self::Threads,
        Self::Handles,
        Self::KernelObjects,
        Self::CommittedPages,
        Self::PinnedPages,
        Self::GuestPages,
        Self::IpcMessages,
        Self::IpcBytes,
        Self::IpcHandles,
        Self::Subscriptions,
        Self::Timers,
        Self::VirtualMachines,
        Self::VirtualCpus,
        Self::DeviceLeases,
        Self::DmaMappings,
        Self::UserAddressSpaces,
        Self::UserMappings,
    ];

    const fn index(self) -> usize {
        self as usize
    }
}

/// One atomic request spanning every resource dimension.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ResourceAmount([u64; RESOURCE_KIND_COUNT]);

impl ResourceAmount {
    pub(crate) const ZERO: Self = Self([0; RESOURCE_KIND_COUNT]);

    /// Returns a new vector with the selected dimension replaced by `value`.
    pub(crate) const fn with(mut self, kind: ResourceKind, value: u64) -> Self {
        self.0[kind.index()] = value;
        self
    }

    pub(crate) const fn get(self, kind: ResourceKind) -> u64 {
        self.0[kind.index()]
    }

    pub(crate) fn is_empty(self) -> bool {
        self.0.iter().all(|value| *value == 0)
    }
}

/// Local ceilings. Ancestors remain independently authoritative ceilings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ResourceLimits(ResourceAmount);

impl ResourceLimits {
    pub(crate) const UNLIMITED: Self = Self(ResourceAmount([u64::MAX; RESOURCE_KIND_COUNT]));

    pub(crate) const fn with(mut self, kind: ResourceKind, limit: u64) -> Self {
        self.0 = self.0.with(kind, limit);
        self
    }

    pub(crate) const fn get(self, kind: ResourceKind) -> u64 {
        self.0.get(kind)
    }
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self::UNLIMITED
    }
}

/// One race-safe domain-local accounting snapshot.
///
/// `total` is authoritative. Concurrent commit or release may make the
/// pending/committed split conservatively stale, so that split is diagnostic;
/// admission and local-limit changes use `total` under the control lock.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ResourceUsage {
    total: ResourceAmount,
    pending: ResourceAmount,
}

impl ResourceUsage {
    pub(crate) fn committed(self, kind: ResourceKind) -> u64 {
        match self.total.get(kind).checked_sub(self.pending.get(kind)) {
            Some(value) => value,
            None => accounting_invariant_violation(),
        }
    }

    pub(crate) const fn pending(self, kind: ResourceKind) -> u64 {
        self.pending.get(kind)
    }

    pub(crate) fn total(self, kind: ResourceKind) -> u64 {
        self.total.get(kind)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResourceError {
    Allocation,
    DomainIdExhausted,
    HierarchyTooDeep,
    EmptyCharge,
    DomainInactive(ResourceDomainId),
    LimitExceeded {
        domain: ResourceDomainId,
        resource: ResourceKind,
        limit: u64,
        used: u64,
        requested: u64,
    },
    UsageOverflow {
        domain: ResourceDomainId,
        resource: ResourceKind,
    },
    LimitBelowUsage {
        domain: ResourceDomainId,
        resource: ResourceKind,
        limit: u64,
        used: u64,
    },
    OutstandingUsage,
    ActiveChildren,
    ChildCountExhausted,
    RetirementNotStarted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RetirementSnapshot {
    pub(crate) usage: ResourceUsage,
    pub(crate) active_children: usize,
}

impl RetirementSnapshot {
    pub(crate) fn is_quiescent(self) -> bool {
        self.active_children == 0
            && ResourceKind::ALL
                .iter()
                .all(|kind| self.usage.total(*kind) == 0)
    }
}

impl From<AllocationError> for ResourceError {
    fn from(_: AllocationError) -> Self {
        Self::Allocation
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum DomainLifecycle {
    Active,
    Retiring,
    Retired,
}

struct DomainControl {
    limits: ResourceLimits,
    lifecycle: DomainLifecycle,
}

struct DomainInner {
    id: ResourceDomainId,
    parent: Option<ResourceDomain>,
    metadata_charge: Option<CommittedCharge>,
    depth: usize,
    control: DomainLock<DomainControl>,
    active_children: AtomicUsize,
    object_published: AtomicBool,
    total: [AtomicU64; RESOURCE_KIND_COUNT],
    pending: [AtomicU64; RESOURCE_KIND_COUNT],
    #[cfg(test)]
    fail_child_allocation: AtomicBool,
}

impl Drop for DomainInner {
    fn drop(&mut self) {
        if self.active_children.load(Ordering::Relaxed) != 0 {
            accounting_invariant_violation();
        }
        for kind in ResourceKind::ALL {
            if self.total[kind.index()].load(Ordering::Relaxed) != 0
                || self.pending[kind.index()].load(Ordering::Relaxed) != 0
            {
                accounting_invariant_violation();
            }
        }

        let Some(parent) = self.parent.as_ref() else {
            return;
        };
        let previous = parent.inner.active_children.fetch_sub(1, Ordering::Release);
        if previous == 0 {
            accounting_invariant_violation();
        }
    }
}

const DOMAIN_METADATA_BYTES: u64 = FallibleArc::<DomainInner>::allocation_size() as u64;

fn domain_metadata_charge() -> ResourceAmount {
    ResourceAmount::ZERO
        .with(ResourceKind::KernelObjects, 1)
        .with(ResourceKind::KernelMemoryBytes, DOMAIN_METADATA_BYTES)
}

/// Shared authority over one node in a hierarchical accounting tree.
///
/// Children and charge owners retain their complete ancestor lifetime through
/// strong parent links. Admission visits root-to-leaf using one short lock at
/// a time; conservative atomic usage prevents siblings from overbooking their
/// shared ancestors. Failure and Drop release leaf-to-root. No public API
/// exposes a lock or invokes caller code while locked.
pub(crate) struct ResourceDomain {
    inner: FallibleArc<DomainInner>,
}

impl ResourceDomain {
    pub(crate) fn try_new_root(limits: ResourceLimits) -> Result<Self, ResourceError> {
        let inner = DomainInner {
            id: ResourceDomainId::allocate()?,
            parent: None,
            metadata_charge: None,
            depth: 0,
            control: DomainLock::new(DomainControl {
                limits,
                lifecycle: DomainLifecycle::Active,
            }),
            active_children: AtomicUsize::new(0),
            object_published: AtomicBool::new(false),
            total: [const { AtomicU64::new(0) }; RESOURCE_KIND_COUNT],
            pending: [const { AtomicU64::new(0) }; RESOURCE_KIND_COUNT],
            #[cfg(test)]
            fail_child_allocation: AtomicBool::new(false),
        };
        Ok(Self {
            inner: FallibleArc::try_new(inner)?,
        })
    }

    /// Constructs and registers one child beneath this domain.
    ///
    /// Parent-sponsored metadata quota is committed before child publication.
    /// If registration or allocation fails, linear owners restore both the
    /// child count and the metadata charge without a special cleanup branch.
    pub(crate) fn try_new_child(&self, limits: ResourceLimits) -> Result<Self, ResourceError> {
        let Some(depth) = self.inner.depth.checked_add(1) else {
            return Err(ResourceError::HierarchyTooDeep);
        };
        if depth >= MAX_DOMAIN_DEPTH {
            return Err(ResourceError::HierarchyTooDeep);
        }
        let id = ResourceDomainId::allocate()?;
        // This reservation is the child-creation admission point. Retirement
        // may close the domain immediately afterward, but the pending/committed
        // metadata total keeps the domain non-quiescent until this pre-cutoff
        // creation either publishes its child or rolls back.
        let metadata_charge = self.reserve(domain_metadata_charge())?.commit();
        self.inner.control.with(|_| {
            self.inner
                .active_children
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |children| {
                    children.checked_add(1)
                })
                .map(|_| ())
                .map_err(|_| ResourceError::ChildCountExhausted)
        })?;

        let inner = DomainInner {
            id,
            parent: Some(self.clone()),
            metadata_charge: Some(metadata_charge),
            depth,
            control: DomainLock::new(DomainControl {
                limits,
                lifecycle: DomainLifecycle::Active,
            }),
            active_children: AtomicUsize::new(0),
            object_published: AtomicBool::new(false),
            total: [const { AtomicU64::new(0) }; RESOURCE_KIND_COUNT],
            pending: [const { AtomicU64::new(0) }; RESOURCE_KIND_COUNT],
            #[cfg(test)]
            fail_child_allocation: AtomicBool::new(false),
        };
        #[cfg(test)]
        if self
            .inner
            .fail_child_allocation
            .swap(false, Ordering::Relaxed)
        {
            drop(inner);
            return Err(ResourceError::Allocation);
        }
        Ok(Self {
            inner: FallibleArc::try_new(inner)?,
        })
    }

    pub(crate) fn id(&self) -> ResourceDomainId {
        self.inner.id
    }

    pub(crate) fn parent_id(&self) -> Option<ResourceDomainId> {
        self.inner.parent.as_ref().map(Self::id)
    }

    /// Reserves quota at this domain and every ancestor in one transaction.
    ///
    /// Pending usage counts against every limit immediately. The returned
    /// linear token must be committed or aborted; Drop performs exact local
    /// rollback without allocation or callbacks.
    pub(crate) fn reserve(
        &self,
        amount: ResourceAmount,
    ) -> Result<ChargeReservation, ResourceError> {
        if amount.is_empty() {
            return Err(ResourceError::EmptyCharge);
        }
        let path = DomainPath::new(self);
        path.reserve(amount)?;
        Ok(ChargeReservation {
            domain: Some(self.clone()),
            amount,
        })
    }

    pub(crate) fn usage(&self) -> ResourceUsage {
        self.inner.control.with(|_| ResourceUsage {
            total: load_total_counters(&self.inner.total),
            pending: load_counters(&self.inner.pending),
        })
    }

    pub(crate) fn local_limits(&self) -> ResourceLimits {
        self.inner.control.with(|control| control.limits)
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
            accounting_invariant_violation();
        }
    }

    /// Replaces local ceilings without changing usage or ancestor policy.
    ///
    /// Child ceilings may exceed an ancestor ceiling: the effective limit is
    /// the minimum remaining capacity along the path. This permits a parent
    /// policy to tighten or relax without rewriting every descendant.
    pub(crate) fn set_local_limits(&self, limits: ResourceLimits) -> Result<(), ResourceError> {
        self.inner.control.with(|control| {
            if control.lifecycle != DomainLifecycle::Active {
                return Err(ResourceError::DomainInactive(self.id()));
            }
            for kind in ResourceKind::ALL {
                let used = self.inner.total[kind.index()].load(Ordering::Acquire);
                let limit = limits.get(kind);
                if limit < used {
                    return Err(ResourceError::LimitBelowUsage {
                        domain: self.id(),
                        resource: kind,
                        limit,
                        used,
                    });
                }
            }
            control.limits = limits;
            Ok(())
        })
    }

    /// Permanently closes admission at this node before existing work drains.
    ///
    /// Descendant reservations visit this node and therefore observe the same
    /// cutoff. Reservations admitted before the cutoff remain committable.
    pub(crate) fn begin_retirement(&self) -> Result<(), ResourceError> {
        self.inner.control.with(|control| match control.lifecycle {
            DomainLifecycle::Active => {
                control.lifecycle = DomainLifecycle::Retiring;
                Ok(())
            }
            DomainLifecycle::Retiring => Ok(()),
            DomainLifecycle::Retired => Err(ResourceError::DomainInactive(self.id())),
        })
    }

    pub(crate) fn retirement_snapshot(&self) -> Result<RetirementSnapshot, ResourceError> {
        self.inner.control.with(|control| {
            if control.lifecycle == DomainLifecycle::Active {
                return Err(ResourceError::RetirementNotStarted);
            }
            Ok(RetirementSnapshot {
                usage: ResourceUsage {
                    total: load_total_counters(&self.inner.total),
                    pending: load_counters(&self.inner.pending),
                },
                active_children: self.inner.active_children.load(Ordering::Acquire),
            })
        })
    }

    /// Publishes terminal retirement only after sponsored ownership drains.
    pub(crate) fn finish_retirement(&self) -> Result<(), ResourceError> {
        self.inner.control.with(|control| {
            match control.lifecycle {
                DomainLifecycle::Active => return Err(ResourceError::RetirementNotStarted),
                DomainLifecycle::Retired => {
                    return Err(ResourceError::DomainInactive(self.id()));
                }
                DomainLifecycle::Retiring => {}
            }
            if self.inner.active_children.load(Ordering::Acquire) != 0 {
                return Err(ResourceError::ActiveChildren);
            }
            if ResourceKind::ALL
                .iter()
                .any(|kind| self.inner.total[kind.index()].load(Ordering::Acquire) != 0)
            {
                return Err(ResourceError::OutstandingUsage);
            }
            control.lifecycle = DomainLifecycle::Retired;
            Ok(())
        })
    }

    #[cfg(test)]
    pub(crate) fn fail_next_child_allocation_for_test(&self) {
        self.inner
            .fail_child_allocation
            .store(true, Ordering::Relaxed);
    }
}

impl Clone for ResourceDomain {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

struct DomainPath<'domain> {
    nodes: [Option<&'domain DomainInner>; MAX_DOMAIN_DEPTH],
    len: usize,
}

impl<'domain> DomainPath<'domain> {
    fn new(target: &'domain ResourceDomain) -> Self {
        let len = target.inner.depth + 1;
        let mut nodes = [None; MAX_DOMAIN_DEPTH];
        let mut cursor = Some(target);
        let mut index = len;
        while let Some(domain) = cursor {
            if index == 0 {
                accounting_invariant_violation();
            }
            index -= 1;
            nodes[index] = Some(&*domain.inner);
            cursor = domain.inner.parent.as_ref();
        }
        if index != 0 {
            accounting_invariant_violation();
        }
        Self { nodes, len }
    }

    fn node(&self, index: usize) -> &DomainInner {
        match self.nodes.get(index).copied().flatten() {
            Some(node) => node,
            None => accounting_invariant_violation(),
        }
    }

    /// Admits one node at a time. Prefix usage is conservative and grants no
    /// authority; a later failure removes that prefix leaf-to-root.
    fn reserve(&self, amount: ResourceAmount) -> Result<(), ResourceError> {
        let mut admitted = 0;
        while admitted < self.len {
            let domain = self.node(admitted);
            let result = domain.control.with(|control| {
                if control.lifecycle != DomainLifecycle::Active {
                    return Err(ResourceError::DomainInactive(domain.id));
                }
                for kind in ResourceKind::ALL {
                    let used = domain.total[kind.index()].load(Ordering::Relaxed);
                    let requested = amount.get(kind);
                    let Some(projected) = used.checked_add(requested) else {
                        return Err(ResourceError::UsageOverflow {
                            domain: domain.id,
                            resource: kind,
                        });
                    };
                    let limit = control.limits.get(kind);
                    if projected > limit {
                        return Err(ResourceError::LimitExceeded {
                            domain: domain.id,
                            resource: kind,
                            limit,
                            used,
                            requested,
                        });
                    }
                }
                add_counters(&domain.total, amount);
                add_counters(&domain.pending, amount);
                Ok(())
            });
            if let Err(error) = result {
                self.release_pending_prefix(admitted, amount);
                return Err(error);
            }
            admitted += 1;
        }
        Ok(())
    }

    fn commit(&self, amount: ResourceAmount) {
        for index in (0..self.len).rev() {
            subtract_pending_counters(&self.node(index).pending, amount);
        }
    }

    fn release_pending(&self, amount: ResourceAmount) {
        self.release_pending_prefix(self.len, amount);
    }

    fn release_pending_prefix(&self, admitted: usize, amount: ResourceAmount) {
        for index in (0..admitted).rev() {
            let domain = self.node(index);
            subtract_pending_counters(&domain.pending, amount);
            subtract_total_counters(&domain.total, amount);
        }
    }

    fn release_committed(&self, amount: ResourceAmount) {
        for index in (0..self.len).rev() {
            subtract_total_counters(&self.node(index).total, amount);
        }
    }
}

fn load_counters(counters: &[AtomicU64; RESOURCE_KIND_COUNT]) -> ResourceAmount {
    let mut amount = ResourceAmount::ZERO;
    for kind in ResourceKind::ALL {
        amount.0[kind.index()] = counters[kind.index()].load(Ordering::Relaxed);
    }
    amount
}

fn load_total_counters(counters: &[AtomicU64; RESOURCE_KIND_COUNT]) -> ResourceAmount {
    let mut amount = ResourceAmount::ZERO;
    for kind in ResourceKind::ALL {
        // Acquire pairs with the releasing total decrement performed after a
        // pending decrement. Observing the new total therefore also observes
        // a pending value no greater than that total.
        amount.0[kind.index()] = counters[kind.index()].load(Ordering::Acquire);
    }
    amount
}

fn add_counters(counters: &[AtomicU64; RESOURCE_KIND_COUNT], amount: ResourceAmount) {
    for kind in ResourceKind::ALL {
        counters[kind.index()].fetch_add(amount.get(kind), Ordering::Relaxed);
    }
}

fn subtract_pending_counters(counters: &[AtomicU64; RESOURCE_KIND_COUNT], amount: ResourceAmount) {
    for kind in ResourceKind::ALL {
        let previous = counters[kind.index()].fetch_sub(amount.get(kind), Ordering::Relaxed);
        if previous < amount.get(kind) {
            accounting_invariant_violation();
        }
    }
}

fn subtract_total_counters(counters: &[AtomicU64; RESOURCE_KIND_COUNT], amount: ResourceAmount) {
    for kind in ResourceKind::ALL {
        // Every pending decrement is sequenced before this publication. AcqRel
        // also carries pending publications from an earlier concurrent total
        // RMW, so a reader of the newest total cannot miss either decrement on
        // a weakly ordered machine.
        let previous = counters[kind.index()].fetch_sub(amount.get(kind), Ordering::AcqRel);
        if previous < amount.get(kind) {
            accounting_invariant_violation();
        }
    }
}

/// Unpublished quota ownership counted as pending usage.
#[must_use = "dropping a reservation rolls its pending charge back"]
pub(crate) struct ChargeReservation {
    domain: Option<ResourceDomain>,
    amount: ResourceAmount,
}

impl ChargeReservation {
    pub(crate) fn domain_id(&self) -> ResourceDomainId {
        match self.domain.as_ref() {
            Some(domain) => domain.id(),
            None => accounting_invariant_violation(),
        }
    }

    pub(crate) const fn amount(&self) -> ResourceAmount {
        self.amount
    }

    /// Atomically changes pending ownership into committed ownership.
    pub(crate) fn commit(mut self) -> CommittedCharge {
        let domain = match self.domain.as_ref() {
            Some(domain) => domain,
            None => accounting_invariant_violation(),
        };
        DomainPath::new(domain).commit(self.amount);
        let domain = match self.domain.take() {
            Some(domain) => domain,
            None => accounting_invariant_violation(),
        };
        CommittedCharge {
            domain,
            amount: self.amount,
        }
    }

    /// Explicitly restores pending quota before returning.
    pub(crate) fn abort(mut self) {
        let domain = match self.domain.as_ref() {
            Some(domain) => domain,
            None => accounting_invariant_violation(),
        };
        DomainPath::new(domain).release_pending(self.amount);
        self.domain = None;
    }
}

impl Drop for ChargeReservation {
    fn drop(&mut self) {
        let Some(domain) = self.domain.as_ref() else {
            return;
        };
        DomainPath::new(domain).release_pending(self.amount);
    }
}

/// Published resource ownership counted as committed usage.
///
/// The charge is intentionally linear. Moving it transfers accounting
/// ownership; Drop releases the charge from the leaf and every ancestor.
#[must_use = "the committed charge owns quota until it is dropped"]
pub(crate) struct CommittedCharge {
    domain: ResourceDomain,
    amount: ResourceAmount,
}

impl CommittedCharge {
    pub(crate) fn domain_id(&self) -> ResourceDomainId {
        self.domain.id()
    }

    pub(crate) const fn amount(&self) -> ResourceAmount {
        self.amount
    }
}

impl Drop for CommittedCharge {
    fn drop(&mut self) {
        DomainPath::new(&self.domain).release_committed(self.amount);
    }
}

#[cold]
fn accounting_invariant_violation() -> ! {
    loop {
        core::hint::spin_loop();
    }
}
