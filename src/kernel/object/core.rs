// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Fallibly owned kernel objects and active-authority accounting.

use alloc::boxed::Box;
use core::any::{Any, TypeId};
#[cfg(test)]
use core::num::NonZeroU32;
use core::num::NonZeroU64;
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering, fence};

use hyper::mm::{AllocationError, FallibleArc, try_box};
use hyper::sync::SpinLock;

use super::{Rights, signals::SignalSource};

const RETIRED: usize = 1 << (usize::BITS - 1);
const ACTIVE_LIMIT: usize = RETIRED - 1;
const EVENT_OBJECT_KIND: u32 = hyper::abi::native::HYPER_NATIVE_OBJECT_EVENT;
const CHANNEL_OBJECT_KIND: u32 = hyper::abi::native::HYPER_NATIVE_OBJECT_CHANNEL;

const _: () = assert!(EVENT_OBJECT_KIND != 0);
const _: () = assert!(CHANNEL_OBJECT_KIND != 0);

static NEXT_KOID: AtomicU64 = AtomicU64::new(1);

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
}

impl From<AllocationError> for ObjectCreationError {
    fn from(_: AllocationError) -> Self {
        Self::Allocation
    }
}

/// Sealing vocabulary shared only by kernel object implementations.
pub(crate) mod private {
    pub(crate) trait Sealed {}
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
    active_handles: AtomicUsize,
}

struct SharedObject {
    header: ObjectHeader,
    payload: Box<dyn ErasedKernelObject>,
    // The allocation itself supplies its retirement-queue node. The link is
    // accessed only while the capability retirement queue is locked, so a
    // final-handle transition never needs to allocate work storage.
    retirement_next: SpinLock<Option<ObjectRef>>,
}

/// Shared object lifetime without process-local authority.
pub(crate) struct ObjectRef(FallibleArc<SharedObject>);

impl ObjectRef {
    /// Heap bytes required by the erased owner and one concrete payload.
    pub(crate) const fn allocation_size<T: KernelObject>() -> Option<usize> {
        FallibleArc::<SharedObject>::allocation_size().checked_add(core::mem::size_of::<T>())
    }

    /// Fallibly constructs an unpublished object with no active handles.
    pub(crate) fn try_new<T: KernelObject>(payload: T) -> Result<Self, ObjectCreationError> {
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
            header: ObjectHeader {
                koid,
                kind: T::KIND,
                supported_rights: T::SUPPORTED_RIGHTS,
                payload_type: TypeId::of::<T>(),
                active_handles: AtomicUsize::new(0),
            },
            payload,
            retirement_next: SpinLock::new(None),
        };
        Ok(Self(FallibleArc::try_new(object)?))
    }

    pub(crate) fn koid(&self) -> Koid {
        self.0.header.koid
    }

    pub(crate) fn kind(&self) -> ObjectKind {
        self.0.header.kind
    }

    pub(crate) fn supported_rights(&self) -> Rights {
        self.0.header.supported_rights
    }

    #[cfg(test)]
    pub(crate) fn active_handle_count(&self) -> usize {
        self.0.header.active_handles.load(Ordering::Relaxed) & ACTIVE_LIMIT
    }

    /// Reports whether the zero-active transition has latched.
    ///
    /// The latch prevents authority resurrection. It is not a teardown-complete
    /// signal: the winning thread invokes the object callback after latching it.
    #[cfg(test)]
    pub(crate) fn is_retired(&self) -> bool {
        self.0.header.active_handles.load(Ordering::Relaxed) == RETIRED
    }

    pub(crate) fn downcast_ref<T: KernelObject>(&self) -> Option<&T> {
        if self.0.header.payload_type != TypeId::of::<T>() {
            return None;
        }
        self.0.payload.as_any().downcast_ref::<T>()
    }

    /// Returns this object's type-erased signal observation capability.
    pub(crate) fn signal_source(&self) -> Option<SignalSource<'_>> {
        self.0.payload.signal_source()
    }

    pub(crate) fn acquire_initial_handle(&self) -> Result<(), ActiveHandleError> {
        self.0
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
        let active = &self.0.header.active_handles;
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
        let active = &self.0.header.active_handles;
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
    pub(crate) fn link_retirement_successor(&self, successor: ObjectRef) {
        self.0.retirement_next.with(|next| {
            if next.is_some() {
                object_invariant_violation();
            }
            *next = Some(successor);
        });
    }

    /// Detaches the successor while the retirement queue is exclusively held.
    pub(crate) fn take_retirement_successor(&self) -> Option<ObjectRef> {
        self.0.retirement_next.with(Option::take)
    }

    /// Completes object-specific policy after the zero-active cut.
    fn complete_zero_active_transition(&self, retirement: &mut ObjectRetirement) {
        if self.0.header.active_handles.load(Ordering::Acquire) != RETIRED {
            object_invariant_violation();
        }
        self.0.payload.on_zero_active_handles(retirement);
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
    head: Option<ObjectRef>,
    tail: Option<ObjectRef>,
}

impl ObjectRetirement {
    pub(crate) const fn new() -> Self {
        Self {
            head: None,
            tail: None,
        }
    }

    pub(crate) fn enqueue(&mut self, object: ObjectRef) {
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

    fn pop(&mut self) -> Option<ObjectRef> {
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
        Self(self.0.clone())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ActiveHandleError {
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
