// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Fallibly owned kernel objects and active-authority accounting.

use alloc::boxed::Box;
use core::any::{Any, TypeId};
use core::marker::PhantomData;
use core::mem::ManuallyDrop;
#[cfg(test)]
use core::num::NonZeroU32;
use core::num::NonZeroU64;
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering, fence};

use hyper::mm::{AllocationError, DeferredArcDrop, FallibleArc, WeakFallibleArc, try_box};
use hyper::sync::{InterruptSpinLock, SpinLock};

use super::{Rights, signals::SignalSource};

const RETIRED: usize = 1 << (usize::BITS - 1);
const ACTIVE_LIMIT: usize = RETIRED - 1;
const EVENT_OBJECT_KIND: u32 = hyper::abi::native::HYPER_NATIVE_OBJECT_EVENT;
const CHANNEL_OBJECT_KIND: u32 = hyper::abi::native::HYPER_NATIVE_OBJECT_CHANNEL;
const THREAD_OBJECT_KIND: u32 = hyper::abi::native::HYPER_NATIVE_OBJECT_THREAD;

const _: () = assert!(EVENT_OBJECT_KIND != 0);
const _: () = assert!(CHANNEL_OBJECT_KIND != 0);
const _: () = assert!(THREAD_OBJECT_KIND != 0);

static NEXT_KOID: AtomicU64 = AtomicU64::new(1);

#[cfg(not(test))]
type FinalReapQueueLock<T> = InterruptSpinLock<T, crate::hal::irq::LocalMask>;

#[cfg(test)]
struct TestReapQueueMask;

#[cfg(test)]
impl hyper::hal::interrupt::InterruptMask for TestReapQueueMask {
    type State = ();

    fn save_and_disable() -> Self::State {}

    fn restore(_state: Self::State) {}
}

#[cfg(test)]
type FinalReapQueueLock<T> = InterruptSpinLock<T, TestReapQueueMask>;

/// Diagnostic identity which never confers authority.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct Koid(NonZeroU64);

impl Koid {
    fn allocate() -> Result<Self, ObjectCreationError> {
        let mut current = NEXT_KOID.load(Ordering::Relaxed);
        loop {
            let value = NonZeroU64::new(current).ok_or(ObjectCreationError::KoidExhausted)?;
            let next = current
                .checked_add(1)
                .ok_or(ObjectCreationError::KoidExhausted)?;
            match NEXT_KOID.compare_exchange_weak(
                current,
                next,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Ok(Self(value)),
                Err(observed) => current = observed,
            }
        }
    }

    pub(crate) const fn get(self) -> u64 {
        self.0.get()
    }
}

/// Stable object-kind identity used by the handle ABI.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct ObjectKind(u32);

impl ObjectKind {
    /// Native Event object kind declared by the generated ABI schema.
    pub(crate) const EVENT: Self = Self(EVENT_OBJECT_KIND);
    /// Native Channel endpoint kind declared by the generated ABI schema.
    pub(crate) const CHANNEL: Self = Self(CHANNEL_OBJECT_KIND);
    /// Native user-thread kind declared by the generated ABI schema.
    pub(crate) const THREAD: Self = Self(THREAD_OBJECT_KIND);

    /// Constructs a synthetic kind for host-only mechanism tests.
    ///
    /// A production constructor is added here only with a corresponding
    /// generated ABI kind, keeping type/kind coherence in one module.
    #[cfg(test)]
    pub(crate) const fn for_test(value: NonZeroU32) -> Self {
        Self(value.get())
    }

    pub(crate) const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ObjectCreationError {
    Allocation,
    KoidExhausted,
    RegistrationExhausted,
}

/// Immutable userspace-publication policy attached at object construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExportPolicy {
    KernelOnly,
    User,
}

/// Strong-reference classes exposed by authority-free diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ObjectReferenceSnapshot {
    pub(crate) kernel_service: usize,
    pub(crate) scheduler: usize,
    pub(crate) publication: usize,
    pub(crate) user_authority: usize,
    pub(crate) operation_pin: usize,
    pub(crate) diagnostic: usize,
    pub(crate) retirement: usize,
}

#[derive(Clone, Copy)]
pub(crate) enum ReferenceKind {
    KernelService,
    Scheduler,
    Publication,
    UserAuthority,
    OperationPin,
    Diagnostic,
    Retirement,
}

impl ReferenceKind {
    const COUNT: usize = 7;

    const fn index(self) -> usize {
        self as usize
    }
}

mod reference_class_private {
    pub(crate) trait Sealed {}
}

/// Compile-time classification for one direct kernel-object owner.
pub(crate) trait ReferenceClass: reference_class_private::Sealed {
    const KIND: ReferenceKind;
}

macro_rules! reference_classes {
    ($($name:ident => $kind:ident),+ $(,)?) => {
        $(
            pub(crate) enum $name {}
            impl reference_class_private::Sealed for $name {}
            impl ReferenceClass for $name {
                const KIND: ReferenceKind = ReferenceKind::$kind;
            }
        )+
    };
}

reference_classes! {
    KernelService => KernelService,
    Scheduler => Scheduler,
    Publication => Publication,
    UserAuthority => UserAuthority,
    OperationPin => OperationPin,
    Diagnostic => Diagnostic,
    Retirement => Retirement,
}

impl From<AllocationError> for ObjectCreationError {
    fn from(_: AllocationError) -> Self {
        Self::Allocation
    }
}

/// Sealing vocabulary shared only by kernel object implementations.
pub(crate) mod private {
    pub(crate) trait Sealed {}
    pub(crate) trait UserExportable: Sealed {}
}

/// Closed kernel-object payload contract.
///
/// Implementations live in kernel subsystems and must also implement the
/// crate-private sealing trait. `on_zero_active_handles` is the sole transition
/// callback: it must be infallible, nonblocking, and allocation-free. It must
/// detach teardown state before releasing payload locks; detached capabilities
/// may then be released because their own zero-active callbacks are appended
/// to the allocation-free retirement queue rather than invoked recursively.
/// The transition rejects future handle preparation but is not operation
/// quiescence: resolved internal references may still be executing on other
/// CPUs, and the callback must not invalidate state they can access.
pub(crate) trait KernelObject: private::Sealed + Any + Send + Sync {
    const KIND: ObjectKind;
    const SUPPORTED_RIGHTS: Rights;

    /// Exposes level-state observation only for objects which support `WAIT`.
    ///
    /// Presence is an immutable property of the concrete object. The returned
    /// source may borrow mutable level state through its own synchronization,
    /// but repeated calls must not add or remove the capability. This accessor
    /// runs under a Process handle-table lock and must remain side-effect-free,
    /// allocation-free, and nonblocking.
    fn signal_source(&self) -> Option<SignalSource<'_>> {
        None
    }

    fn on_zero_active_handles(&self, _retirement: &mut ObjectRetirement) {}
}

/// Closed marker for object payloads which may be published to userspace.
///
/// Kernel-only object types deliberately cannot construct `ObjectPublication`.
/// This makes the publication boundary a type property rather than a rights
/// convention which a future caller could accidentally bypass.
pub(crate) trait UserExportableObject: KernelObject + private::UserExportable {}

impl<T> UserExportableObject for T where T: KernelObject + private::UserExportable {}

trait ErasedKernelObject: Any + Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn signal_source(&self) -> Option<SignalSource<'_>>;
    fn on_zero_active_handles(&self, retirement: &mut ObjectRetirement);
}

impl<T: KernelObject> ErasedKernelObject for T {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn signal_source(&self) -> Option<SignalSource<'_>> {
        KernelObject::signal_source(self)
    }

    fn on_zero_active_handles(&self, retirement: &mut ObjectRetirement) {
        KernelObject::on_zero_active_handles(self, retirement);
    }
}

struct ObjectHeader {
    koid: Koid,
    kind: ObjectKind,
    supported_rights: Rights,
    payload_type: TypeId,
    export_policy: ExportPolicy,
    active_handles: AtomicUsize,
    references: [AtomicUsize; ReferenceKind::COUNT],
}

struct SharedObject {
    // Declared first so directory metadata is detached before payload charges
    // are released by field destruction.
    #[cfg(not(test))]
    directory: super::directory::Membership,
    header: ObjectHeader,
    payload: Box<dyn ErasedKernelObject>,
    // The allocation itself supplies its retirement-queue node. The link is
    // accessed only while the capability retirement queue is locked, so a
    // final-handle transition never needs to allocate work storage.
    retirement_next: SpinLock<Option<ErasedKernelRef<Retirement>>>,
    // The allocation also supplies its final-reap queue node. The queue's
    // deferred owners keep every linked allocation initialized, and only the
    // final-reap queue lock may inspect or replace this link.
    final_reap_next: SpinLock<Option<DeferredArcDrop<SharedObject>>>,
}

/// Private type-erased allocation owner used to implement typed references.
pub(super) struct ObjectRef {
    allocation: ManuallyDrop<FallibleArc<SharedObject>>,
    class: ReferenceKind,
}

/// Direct, compiler-typed kernel reference with no handle-table lookup.
pub(crate) struct KernelRef<T: KernelObject, C: ReferenceClass> {
    owner: ObjectRef,
    marker: PhantomData<fn() -> (T, C)>,
}

/// Type-erased direct owner used only by heterogeneous capability storage.
pub(crate) struct ErasedKernelRef<C: ReferenceClass> {
    owner: ObjectRef,
    marker: PhantomData<fn() -> C>,
}

/// Exportable service owner. Only this type can derive a publication token.
pub(crate) struct PublishableRef<T: UserExportableObject, C: ReferenceClass> {
    reference: KernelRef<T, C>,
}

/// One candidate for first userspace publication of an exportable object.
///
/// Multiple candidates may be prepared from the same `PublishableRef`, but the
/// object's irreversible unpublished-to-active transition admits exactly one.
pub(crate) struct ObjectPublication<T: UserExportableObject> {
    reference: KernelRef<T, Publication>,
}

/// Linear ownership of exactly one contribution to the active-handle count.
///
/// Construction and duplication update the count before this value exists.
/// Consuming release updates it exactly once before relinquishing the matching
/// `UserAuthority` reference, so safe callers cannot separate lifetime
/// ownership from active authority accounting.
pub(crate) struct ActiveHandleOwner {
    object: Option<ErasedKernelRef<UserAuthority>>,
}

/// Non-owning directory reference to one object allocation.
///
/// This type deliberately exposes only upgrade and liveness observation. A
/// diagnostic registry must never become an authority or extend payload life.
pub(crate) struct WeakObjectRef(WeakFallibleArc<SharedObject>);

/// Active-handle state captured at one diagnostic observation point.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ObjectHandleState {
    Unpublished,
    Active(usize),
    Retired,
}

/// Authority-free object metadata suitable for debug and future ABI encoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ObjectSnapshot {
    pub(crate) koid: Koid,
    pub(crate) kind: ObjectKind,
    pub(crate) supported_rights: Rights,
    pub(crate) handles: ObjectHandleState,
    pub(crate) export_policy: ExportPolicy,
    pub(crate) references: ObjectReferenceSnapshot,
    /// Strong internal owners sampled independently from the class counters.
    ///
    /// Concurrent reference traffic makes this diagnostic snapshot weakly
    /// consistent: consumers must not require the class sum to equal this
    /// value. The temporary diagnostic owner is included for directory scans.
    pub(crate) strong_references: usize,
}

struct FinalReapQueue {
    head: Option<DeferredArcDrop<SharedObject>>,
    // Address of the final node. The head and embedded owner chain keep this
    // allocation live; an address avoids adding another owning reference.
    tail_address: Option<usize>,
}

impl FinalReapQueue {
    const fn new() -> Self {
        Self {
            head: None,
            tail_address: None,
        }
    }

    fn push(&mut self, object: DeferredArcDrop<SharedObject>) {
        let address = core::ptr::from_ref::<SharedObject>(&object).expose_provenance();
        if let Some(tail_address) = self.tail_address {
            // SAFETY: `tail_address` was exposed from the prior final node.
            // That node remains initialized in the owner chain rooted at
            // `head`, and the queue lock excludes both pop and another push.
            let tail =
                unsafe { &*core::ptr::with_exposed_provenance::<SharedObject>(tail_address) };
            tail.final_reap_next.with(|next| {
                if next.is_some() {
                    object_invariant_violation();
                }
                *next = Some(object);
            });
        } else {
            if self.head.is_some() {
                object_invariant_violation();
            }
            self.head = Some(object);
        }
        self.tail_address = Some(address);
    }

    fn pop(&mut self) -> Option<DeferredArcDrop<SharedObject>> {
        let head = self.head.take()?;
        self.head = head.final_reap_next.with(Option::take);
        if self.head.is_none() {
            self.tail_address = None;
        }
        Some(head)
    }

    const fn has_pending(&self) -> bool {
        self.head.is_some()
    }
}

static FINAL_REAP_QUEUE: FinalReapQueueLock<FinalReapQueue> =
    FinalReapQueueLock::new(FinalReapQueue::new());

impl ObjectRef {
    /// Heap bytes required by the erased owner and one concrete payload.
    pub(crate) const fn allocation_size<T: KernelObject>() -> Option<usize> {
        let object =
            FallibleArc::<SharedObject>::allocation_size().checked_add(core::mem::size_of::<T>());
        #[cfg(not(test))]
        {
            match object {
                Some(bytes) => bytes.checked_add(super::directory::registration_size()),
                None => None,
            }
        }
        #[cfg(test)]
        {
            object
        }
    }

    /// Fallibly constructs an unpublished object with no active handles.
    fn try_new<T: KernelObject>(
        payload: T,
        export_policy: ExportPolicy,
        class: ReferenceKind,
    ) -> Result<Self, ObjectCreationError> {
        let signal_source = payload.signal_source();
        if T::SUPPORTED_RIGHTS.contains(Rights::WAIT) != signal_source.is_some()
            || signal_source.is_some_and(SignalSource::has_empty_mask)
        {
            // Object definitions are closed kernel code. Publishing a WAIT
            // right without its mechanism, or an unreachable mechanism
            // without WAIT, would make authority depend on a hidden type test.
            object_invariant_violation();
        }
        let payload: Box<dyn ErasedKernelObject> = try_box(payload)?;
        let koid = Koid::allocate()?;
        let object = SharedObject {
            #[cfg(not(test))]
            directory: super::directory::Membership::new(koid),
            header: ObjectHeader {
                koid,
                kind: T::KIND,
                supported_rights: T::SUPPORTED_RIGHTS,
                payload_type: TypeId::of::<T>(),
                export_policy,
                active_handles: AtomicUsize::new(0),
                references: core::array::from_fn(|index| {
                    AtomicUsize::new(usize::from(index == class.index()))
                }),
            },
            payload,
            retirement_next: SpinLock::new(None),
            final_reap_next: SpinLock::new(None),
        };
        let object = Self {
            allocation: ManuallyDrop::new(FallibleArc::try_new(object)?),
            class,
        };
        #[cfg(not(test))]
        super::directory::register(&object)?;
        Ok(object)
    }

    pub(crate) fn koid(&self) -> Koid {
        self.allocation.header.koid
    }

    pub(crate) fn kind(&self) -> ObjectKind {
        self.allocation.header.kind
    }

    pub(crate) fn supported_rights(&self) -> Rights {
        self.allocation.header.supported_rights
    }

    pub(super) fn downgrade(&self) -> WeakObjectRef {
        WeakObjectRef(self.allocation.downgrade())
    }

    #[cfg(not(test))]
    pub(super) fn publish_directory_membership(&self) {
        self.allocation.directory.publish();
    }

    pub(crate) fn snapshot(&self) -> ObjectSnapshot {
        let active = self
            .allocation
            .header
            .active_handles
            .load(Ordering::Acquire);
        let handles = match active {
            0 => ObjectHandleState::Unpublished,
            RETIRED => ObjectHandleState::Retired,
            count => ObjectHandleState::Active(count),
        };
        ObjectSnapshot {
            koid: self.koid(),
            kind: self.kind(),
            supported_rights: self.supported_rights(),
            handles,
            export_policy: self.allocation.header.export_policy,
            references: reference_snapshot(&self.allocation.header.references),
            strong_references: self.allocation.strong_count(),
        }
    }

    #[cfg(test)]
    pub(crate) fn active_handle_count(&self) -> usize {
        self.allocation
            .header
            .active_handles
            .load(Ordering::Relaxed)
            & ACTIVE_LIMIT
    }

    /// Reports whether the zero-active transition has latched.
    ///
    /// The latch prevents authority resurrection. It is not a teardown-complete
    /// signal: the winning thread invokes the object callback after latching it.
    #[cfg(test)]
    pub(crate) fn is_retired(&self) -> bool {
        self.allocation
            .header
            .active_handles
            .load(Ordering::Relaxed)
            == RETIRED
    }

    pub(crate) fn downcast_ref<T: KernelObject>(&self) -> Option<&T> {
        if self.allocation.header.payload_type != TypeId::of::<T>() {
            return None;
        }
        self.allocation.payload.as_any().downcast_ref::<T>()
    }

    /// Returns this object's type-erased signal observation capability.
    pub(crate) fn signal_source(&self) -> Option<SignalSource<'_>> {
        self.allocation.payload.signal_source()
    }

    pub(crate) fn acquire_initial_handle(&self) -> Result<(), ActiveHandleError> {
        self.allocation
            .header
            .active_handles
            .compare_exchange(0, 1, Ordering::Relaxed, Ordering::Relaxed)
            .map(|_| ())
            .map_err(|current| {
                if current == RETIRED {
                    ActiveHandleError::Retired
                } else {
                    ActiveHandleError::AlreadyActive
                }
            })
    }

    pub(crate) fn acquire_additional_handle(&self) -> Result<(), ActiveHandleError> {
        let active = &self.allocation.header.active_handles;
        let mut current = active.load(Ordering::Relaxed);
        loop {
            if current == RETIRED {
                return Err(ActiveHandleError::Retired);
            }
            if current == 0 {
                return Err(ActiveHandleError::AlreadyActive);
            }
            if current == ACTIVE_LIMIT {
                return Err(ActiveHandleError::CountExhausted);
            }
            match active.compare_exchange_weak(
                current,
                current + 1,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Ok(()),
                Err(observed) => current = observed,
            }
        }
    }

    /// Releases one owner and reports whether it won the zero-active cut.
    ///
    /// The winner must enqueue this object for callback completion. Returning
    /// the decision, instead of invoking object policy here, keeps capability
    /// destruction iterative even when a callback releases more handles.
    pub(crate) fn release_active_handle(&self) -> bool {
        let active = &self.allocation.header.active_handles;
        let mut current = active.load(Ordering::Relaxed);
        loop {
            if current == 0 || current == RETIRED {
                // PreparedHandle is the only constructor for an active owner.
                // Continuing would conceal a double release and make later
                // teardown decisions rely on a false active-handle count.
                object_invariant_violation();
            }
            let next = if current == 1 { RETIRED } else { current - 1 };
            match active.compare_exchange_weak(current, next, Ordering::Release, Ordering::Relaxed)
            {
                Ok(_) => {
                    if next == RETIRED {
                        // Pair with every releasing decrement whose active
                        // owner preceded this final one. Retirement queue
                        // publication follows this acquire before object
                        // policy can observe the completed owners.
                        fence(Ordering::Acquire);
                        return true;
                    }
                    return false;
                }
                Err(observed) => current = observed,
            }
        }
    }

    /// Appends one already-retired object behind this queue node.
    fn link_retirement_successor(&self, successor: ErasedKernelRef<Retirement>) {
        self.allocation.retirement_next.with(|next| {
            if next.is_some() {
                object_invariant_violation();
            }
            *next = Some(successor);
        });
    }

    /// Detaches the successor while the retirement queue is exclusively held.
    fn take_retirement_successor(&self) -> Option<ErasedKernelRef<Retirement>> {
        self.allocation.retirement_next.with(Option::take)
    }

    /// Completes object-specific policy after the zero-active cut.
    fn complete_zero_active_transition(&self, retirement: &mut ObjectRetirement) {
        if self
            .allocation
            .header
            .active_handles
            .load(Ordering::Acquire)
            != RETIRED
        {
            object_invariant_violation();
        }
        self.allocation.payload.on_zero_active_handles(retirement);
    }

    fn clone_as(&self, class: ReferenceKind) -> Self {
        let allocation = FallibleArc::clone(&self.allocation);
        retain_reference_class(&allocation.header.references[class.index()]);
        Self {
            allocation: ManuallyDrop::new(allocation),
            class,
        }
    }

    fn into_class(mut self, class: ReferenceKind) -> Self {
        if self.class.index() != class.index() {
            release_reference_class(&self.allocation.header.references[self.class.index()]);
            retain_reference_class(&self.allocation.header.references[class.index()]);
            self.class = class;
        }
        self
    }
}

/// Heap bytes required by one erased owner and concrete object payload.
pub(crate) const fn object_allocation_size<T: KernelObject>() -> Option<usize> {
    ObjectRef::allocation_size::<T>()
}

impl<T: KernelObject> KernelRef<T, Scheduler> {
    /// Constructs a kernel-only object whose initial owner is the scheduler.
    pub(crate) fn try_new_scheduler(payload: T) -> Result<Self, ObjectCreationError> {
        Ok(Self::from_owner(ObjectRef::try_new(
            payload,
            ExportPolicy::KernelOnly,
            ReferenceKind::Scheduler,
        )?))
    }
}

impl<T: KernelObject, C: ReferenceClass> KernelRef<T, C> {
    fn from_owner(owner: ObjectRef) -> Self {
        if owner.downcast_ref::<T>().is_none() || owner.class.index() != C::KIND.index() {
            object_invariant_violation();
        }
        Self {
            owner,
            marker: PhantomData,
        }
    }

    fn from_validated_owner(owner: ObjectRef) -> Self {
        if owner.class.index() != C::KIND.index() {
            object_invariant_violation();
        }
        Self {
            owner,
            marker: PhantomData,
        }
    }

    pub(crate) fn object(&self) -> &T {
        match self.owner.downcast_ref::<T>() {
            Some(object) => object,
            None => object_invariant_violation(),
        }
    }

    pub(crate) fn koid(&self) -> Koid {
        self.owner.koid()
    }

    pub(crate) fn snapshot(&self) -> ObjectSnapshot {
        self.owner.snapshot()
    }
}

impl<T: KernelObject, C: ReferenceClass> Clone for KernelRef<T, C> {
    fn clone(&self) -> Self {
        Self::from_validated_owner(self.owner.clone())
    }
}

impl<T: UserExportableObject, C: ReferenceClass> PublishableRef<T, C> {
    pub(crate) fn object(&self) -> &T {
        self.reference.object()
    }

    pub(crate) fn snapshot(&self) -> ObjectSnapshot {
        self.reference.snapshot()
    }

    #[cfg(test)]
    pub(crate) fn koid(&self) -> Koid {
        self.reference.koid()
    }

    #[cfg(test)]
    pub(crate) fn active_handle_count(&self) -> usize {
        self.reference.owner.active_handle_count()
    }

    #[cfg(test)]
    pub(crate) fn is_retired(&self) -> bool {
        self.reference.owner.is_retired()
    }

    pub(crate) fn publication(&self) -> ObjectPublication<T> {
        ObjectPublication {
            reference: KernelRef::from_validated_owner(
                self.reference.owner.clone_as(ReferenceKind::Publication),
            ),
        }
    }
}

impl<T: UserExportableObject> PublishableRef<T, KernelService> {
    /// Transfers one service owner into the scheduler ownership class.
    pub(crate) fn into_scheduler(self) -> KernelRef<T, Scheduler> {
        KernelRef::from_owner(self.reference.owner.into_class(ReferenceKind::Scheduler))
    }
}

impl<T: UserExportableObject> PublishableRef<T, KernelService> {
    pub(crate) fn try_new(payload: T) -> Result<Self, ObjectCreationError> {
        Ok(Self {
            reference: KernelRef::from_owner(ObjectRef::try_new(
                payload,
                ExportPolicy::User,
                ReferenceKind::KernelService,
            )?),
        })
    }
}

impl<T: UserExportableObject, C: ReferenceClass> Clone for PublishableRef<T, C> {
    fn clone(&self) -> Self {
        Self {
            reference: self.reference.clone(),
        }
    }
}

impl<T: UserExportableObject> ObjectPublication<T> {
    pub(crate) fn try_new(payload: T) -> Result<Self, ObjectCreationError> {
        Ok(Self {
            reference: KernelRef::from_owner(ObjectRef::try_new(
                payload,
                ExportPolicy::User,
                ReferenceKind::Publication,
            )?),
        })
    }

    pub(crate) fn supported_rights(&self) -> Rights {
        self.reference.owner.supported_rights()
    }

    /// Irreversibly activates the first userspace handle owner.
    pub(crate) fn activate(self) -> Result<ActiveHandleOwner, ActiveHandleError> {
        let owner = self.reference.owner;
        if owner.allocation.header.export_policy != ExportPolicy::User {
            return Err(ActiveHandleError::NotExportable);
        }
        owner.acquire_initial_handle()?;
        Ok(ActiveHandleOwner {
            object: Some(ErasedKernelRef {
                owner: owner.into_class(ReferenceKind::UserAuthority),
                marker: PhantomData,
            }),
        })
    }
}

impl<C: ReferenceClass> ErasedKernelRef<C> {
    pub(crate) fn koid(&self) -> Koid {
        self.owner.koid()
    }

    pub(crate) fn kind(&self) -> ObjectKind {
        self.owner.kind()
    }

    pub(crate) fn snapshot(&self) -> ObjectSnapshot {
        self.owner.snapshot()
    }
}

impl ActiveHandleOwner {
    fn object(&self) -> &ErasedKernelRef<UserAuthority> {
        match self.object.as_ref() {
            Some(object) => object,
            None => object_invariant_violation(),
        }
    }

    pub(crate) fn koid(&self) -> Koid {
        self.object().koid()
    }

    pub(crate) fn kind(&self) -> ObjectKind {
        self.object().kind()
    }

    pub(crate) fn pin<T: KernelObject>(&self) -> Option<KernelRef<T, OperationPin>> {
        self.object().owner.downcast_ref::<T>()?;
        Some(KernelRef::from_validated_owner(
            self.object().owner.clone_as(ReferenceKind::OperationPin),
        ))
    }

    pub(crate) fn pin_waitable(&self) -> Option<ErasedKernelRef<OperationPin>> {
        self.object().owner.signal_source()?;
        Some(ErasedKernelRef {
            owner: self.object().owner.clone_as(ReferenceKind::OperationPin),
            marker: PhantomData,
        })
    }

    /// Creates one additional active owner or leaves the count unchanged.
    pub(crate) fn try_duplicate(&self) -> Result<Self, ActiveHandleError> {
        self.object().owner.acquire_additional_handle()?;
        Ok(Self {
            object: Some(self.object().clone()),
        })
    }

    /// Releases exactly one active owner into the caller's iterative worklist.
    pub(crate) fn release_into(mut self, retirement: &mut ObjectRetirement) {
        let object = match self.object.take() {
            Some(object) => object,
            None => object_invariant_violation(),
        };
        if object.owner.release_active_handle() {
            retirement.enqueue(ErasedKernelRef {
                owner: object.owner.into_class(ReferenceKind::Retirement),
                marker: PhantomData,
            });
        }
    }
}

impl Drop for ActiveHandleOwner {
    fn drop(&mut self) {
        let Some(object) = self.object.take() else {
            return;
        };
        let mut retirement = ObjectRetirement::new();
        if object.owner.release_active_handle() {
            retirement.enqueue(ErasedKernelRef {
                owner: object.owner.into_class(ReferenceKind::Retirement),
                marker: PhantomData,
            });
        }
        retirement.drain();
    }
}

impl ErasedKernelRef<OperationPin> {
    pub(crate) fn signal_source(&self) -> Option<SignalSource<'_>> {
        self.owner.signal_source()
    }
}

impl ErasedKernelRef<Retirement> {
    fn link_retirement_successor(&self, successor: Self) {
        self.owner.link_retirement_successor(successor);
    }

    fn take_retirement_successor(&self) -> Option<Self> {
        self.owner.take_retirement_successor()
    }

    fn complete_zero_active_transition(&self, retirement: &mut ObjectRetirement) {
        self.owner.complete_zero_active_transition(retirement);
    }
}

impl<C: ReferenceClass> Clone for ErasedKernelRef<C> {
    fn clone(&self) -> Self {
        Self {
            owner: self.owner.clone(),
            marker: PhantomData,
        }
    }
}

impl WeakObjectRef {
    pub(super) fn upgrade(&self) -> Option<ErasedKernelRef<Diagnostic>> {
        let allocation = self.0.upgrade()?;
        retain_reference_class(&allocation.header.references[ReferenceKind::Diagnostic.index()]);
        Some(ErasedKernelRef {
            owner: ObjectRef {
                allocation: ManuallyDrop::new(allocation),
                class: ReferenceKind::Diagnostic,
            },
            marker: PhantomData,
        })
    }

    pub(super) fn is_alive(&self) -> bool {
        self.0.is_alive()
    }
}

/// Allocation-free worklist for zero-active object callbacks.
///
/// A final handle contributes the already-allocated object as its own queue
/// node. Callbacks receive this same worklist, so releasing capability owners
/// can append successors without nesting another object callback on the Rust
/// stack. Each top-level handle release drains its complete callback closure
/// synchronously before returning.
pub(crate) struct ObjectRetirement {
    head: Option<ErasedKernelRef<Retirement>>,
    tail: Option<ErasedKernelRef<Retirement>>,
}

impl ObjectRetirement {
    pub(crate) const fn new() -> Self {
        Self {
            head: None,
            tail: None,
        }
    }

    pub(crate) fn enqueue(&mut self, object: ErasedKernelRef<Retirement>) {
        if let Some(tail) = self.tail.as_ref() {
            tail.link_retirement_successor(object.clone());
            self.tail = Some(object);
        } else {
            if self.head.is_some() {
                object_invariant_violation();
            }
            self.head = Some(object.clone());
            self.tail = Some(object);
        }
    }

    pub(crate) fn drain(&mut self) {
        while let Some(object) = self.pop() {
            object.complete_zero_active_transition(self);
        }
    }

    fn pop(&mut self) -> Option<ErasedKernelRef<Retirement>> {
        let head = self.head.take()?;
        self.head = head.take_retirement_successor();
        if self.head.is_none() {
            self.tail = None;
        }
        Some(head)
    }
}

impl Drop for ObjectRetirement {
    fn drop(&mut self) {
        if self.head.is_some() || self.tail.is_some() {
            object_invariant_violation();
        }
    }
}

impl Clone for ObjectRef {
    fn clone(&self) -> Self {
        self.clone_as(self.class)
    }
}

impl Drop for ObjectRef {
    fn drop(&mut self) {
        release_reference_class(&self.allocation.header.references[self.class.index()]);
        // SAFETY: `ObjectRef` stores its owner in `ManuallyDrop` solely so this
        // destructor can consume it. This method runs exactly once and never
        // accesses the field after taking it.
        let object = unsafe { ManuallyDrop::take(&mut self.allocation) };
        if let Some(object) = object.release_deferred() {
            validate_final_release(&object);
            FINAL_REAP_QUEUE.with(|queue| queue.push(object));
            #[cfg(not(test))]
            crate::kernel::reaper::request();
        }
    }
}

fn validate_final_release(object: &DeferredArcDrop<SharedObject>) {
    if object
        .header
        .references
        .iter()
        .any(|counter| counter.load(Ordering::Relaxed) != 0)
    {
        object_invariant_violation();
    }
    let active = object.header.active_handles.load(Ordering::Relaxed);
    if active != 0 && active != RETIRED {
        object_invariant_violation();
    }
}

fn reference_snapshot(references: &[AtomicUsize; ReferenceKind::COUNT]) -> ObjectReferenceSnapshot {
    ObjectReferenceSnapshot {
        kernel_service: references[ReferenceKind::KernelService.index()].load(Ordering::Relaxed),
        scheduler: references[ReferenceKind::Scheduler.index()].load(Ordering::Relaxed),
        publication: references[ReferenceKind::Publication.index()].load(Ordering::Relaxed),
        user_authority: references[ReferenceKind::UserAuthority.index()].load(Ordering::Relaxed),
        operation_pin: references[ReferenceKind::OperationPin.index()].load(Ordering::Relaxed),
        diagnostic: references[ReferenceKind::Diagnostic.index()].load(Ordering::Relaxed),
        retirement: references[ReferenceKind::Retirement.index()].load(Ordering::Relaxed),
    }
}

fn retain_reference_class(counter: &AtomicUsize) {
    let mut current = counter.load(Ordering::Relaxed);
    loop {
        if current == usize::MAX {
            return;
        }
        match counter.compare_exchange_weak(
            current,
            current + 1,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => return,
            Err(observed) => current = observed,
        }
    }
}

fn release_reference_class(counter: &AtomicUsize) {
    let mut current = counter.load(Ordering::Relaxed);
    loop {
        if current == usize::MAX {
            return;
        }
        if current == 0 {
            object_invariant_violation();
        }
        match counter.compare_exchange_weak(
            current,
            current - 1,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => return,
            Err(observed) => current = observed,
        }
    }
}

/// Reports whether at least one zero-reference object awaits final drop.
///
/// This is a scheduling hint only: another reaper may consume the work before
/// the caller acts, and a producer may enqueue immediately after `false`.
#[cfg(not(test))]
pub(crate) fn final_reap_pending(_access: &crate::kernel::reaper::ReaperAccess) -> bool {
    FINAL_REAP_QUEUE.with(|queue| queue.has_pending())
}

#[cfg(test)]
pub(crate) fn final_reap_pending() -> bool {
    FINAL_REAP_QUEUE.with(|queue| queue.has_pending())
}

/// Performs one final object drop outside the reap-queue lock.
///
/// Object-specific destructors may acquire directory or subsystem locks and
/// may release further object references. Detaching the owner first therefore keeps
/// queue lock ordering acyclic and turns cascading final drops into new queue
/// entries rather than recursive destruction.
#[cfg(not(test))]
pub(crate) fn reap_one_final_object(_access: &mut crate::kernel::reaper::ReaperAccess) -> bool {
    let object = FINAL_REAP_QUEUE.with(FinalReapQueue::pop);
    let Some(object) = object else {
        return false;
    };
    drop(object);
    true
}

#[cfg(test)]
pub(crate) fn reap_one_final_object() -> bool {
    let object = FINAL_REAP_QUEUE.with(FinalReapQueue::pop);
    let Some(object) = object else {
        return false;
    };
    drop(object);
    true
}

/// Reaps at most `limit` objects without allocating.
#[cfg(test)]
pub(crate) fn reap_final_objects(limit: usize) -> usize {
    let mut reaped = 0usize;
    while reaped < limit && reap_one_final_object() {
        reaped += 1;
    }
    reaped
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ActiveHandleError {
    NotExportable,
    Retired,
    AlreadyActive,
    CountExhausted,
}

#[cold]
fn object_invariant_violation() -> ! {
    loop {
        core::hint::spin_loop();
    }
}
