// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Fallibly owned kernel objects and active-authority accounting.

use alloc::boxed::Box;
use core::any::{Any, TypeId};
use core::num::{NonZeroU32, NonZeroU64};
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering, fence};

use hyper::mm::{AllocationError, FallibleArc, try_box};

use super::Rights;

const RETIRED: usize = 1 << (usize::BITS - 1);
const ACTIVE_LIMIT: usize = RETIRED - 1;

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
pub(crate) struct ObjectKind(NonZeroU32);

impl ObjectKind {
    /// Constructs a synthetic kind for host-only mechanism tests.
    ///
    /// A production constructor is added here only with a corresponding
    /// generated ABI kind, keeping type/kind coherence in one module.
    #[cfg(test)]
    pub(crate) const fn for_test(value: NonZeroU32) -> Self {
        Self(value)
    }

    pub(crate) const fn get(self) -> u32 {
        self.0.get()
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
/// callback: it must be infallible, nonblocking, allocation-free, and must not
/// recursively release capabilities. It may only detach state and publish
/// already-reserved teardown work. The transition rejects future handle
/// preparation but is not operation quiescence: resolved internal references
/// may still be executing on other CPUs, and the callback must not invalidate
/// state they can access.
pub(crate) trait KernelObject: private::Sealed + Any + Send + Sync {
    const KIND: ObjectKind;
    const SUPPORTED_RIGHTS: Rights;

    fn on_zero_active_handles(&self) {}
}

trait ErasedKernelObject: Any + Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn on_zero_active_handles(&self);
}

impl<T: KernelObject> ErasedKernelObject for T {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn on_zero_active_handles(&self) {
        KernelObject::on_zero_active_handles(self);
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
}

/// Shared object lifetime without process-local authority.
pub(crate) struct ObjectRef(FallibleArc<SharedObject>);

impl ObjectRef {
    /// Fallibly constructs an unpublished object with no active handles.
    pub(crate) fn try_new<T: KernelObject>(payload: T) -> Result<Self, ObjectCreationError> {
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

    pub(crate) fn active_handle_count(&self) -> usize {
        self.0.header.active_handles.load(Ordering::Relaxed) & ACTIVE_LIMIT
    }

    /// Reports whether the zero-active transition has latched.
    ///
    /// The latch prevents authority resurrection. It is not a teardown-complete
    /// signal: the winning thread invokes the object callback after latching it.
    pub(crate) fn is_retired(&self) -> bool {
        self.0.header.active_handles.load(Ordering::Relaxed) == RETIRED
    }

    pub(crate) fn downcast_ref<T: KernelObject>(&self) -> Option<&T> {
        if self.0.header.payload_type != TypeId::of::<T>() {
            return None;
        }
        self.0.payload.as_any().downcast_ref::<T>()
    }

    pub(super) fn acquire_initial_handle(&self) -> Result<(), ActiveHandleError> {
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

    pub(super) fn acquire_additional_handle(&self) -> Result<(), ActiveHandleError> {
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

    pub(super) fn release_active_handle(&self) {
        let active = &self.0.header.active_handles;
        let mut current = active.load(Ordering::Relaxed);
        loop {
            if current == 0 || current == RETIRED {
                // PreparedHandle is the only constructor for an active owner.
                // Continuing would conceal a double release and make later
                // teardown decisions rely on a false active-handle count.
                super::invariant_violation();
            }
            let next = if current == 1 { RETIRED } else { current - 1 };
            match active.compare_exchange_weak(current, next, Ordering::Release, Ordering::Relaxed)
            {
                Ok(_) => {
                    if next == RETIRED {
                        // Pair with every releasing decrement whose active
                        // owner preceded this final one. The callback may now
                        // detach state after observing all completed owners.
                        fence(Ordering::Acquire);
                        self.0.payload.on_zero_active_handles();
                    }
                    return;
                }
                Err(observed) => current = observed,
            }
        }
    }
}

impl Clone for ObjectRef {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ActiveHandleError {
    Retired,
    AlreadyActive,
    CountExhausted,
}
