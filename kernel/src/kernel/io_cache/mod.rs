// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Bounded kernel-owned cache for immutable file data.
//!
//! The cache owns only clean, immutable payloads. A miss reserves one slot
//! under the cache lock, but the caller allocates and fills its payload after
//! that lock has been released. Publication consumes a generation-tagged
//! reservation, so a stale completion cannot populate a recycled slot.
//!
//! The system cache uses a fixed boot-time budget. Cache pages are shared by
//! filesystems and can predate the root resource domain, so charging the first
//! reader would assign arbitrary ownership and create an initialization cycle.
//! Per-mount accounting may be added with writable mounts; it is deliberately
//! absent from this immutable foundation.

#![cfg_attr(test, allow(dead_code))]

use alloc::vec::Vec;
use core::num::NonZeroU64;

use hyper::mm::FallibleArc;
#[cfg(not(test))]
use hyper::sync::InterruptSpinLock;
#[cfg(test)]
use hyper::sync::SpinLock;

mod state;

/// Maximum number of clean file pages retained by the system cache.
///
/// Active readers may briefly retain an evicted shared page after the cache
/// releases it. The bound applies to cache-owned resident entries, while the
/// internal API keeps reader ownership short-lived and non-exportable.
pub(crate) const SYSTEM_PAGE_CAPACITY: usize = 256;

#[cfg(not(test))]
type CacheLock<T> = InterruptSpinLock<T, crate::hal::irq::LocalMask>;
#[cfg(test)]
type CacheLock<T> = SpinLock<T>;

/// One nonzero generation of a mounted filesystem instance.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct FilesystemGeneration(NonZeroU64);

impl FilesystemGeneration {
    pub(crate) const fn new(value: NonZeroU64) -> Self {
        Self(value)
    }

    pub(crate) const fn get(self) -> u64 {
        self.0.get()
    }
}

/// Stable identity of one node within a filesystem generation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct NodeIdentity(NonZeroU64);

impl NodeIdentity {
    pub(crate) const fn new(value: NonZeroU64) -> Self {
        Self(value)
    }

    pub(crate) const fn get(self) -> u64 {
        self.0.get()
    }
}

/// Page offset within one file-data stream.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(transparent)]
pub(crate) struct FilePageIndex(u64);

impl FilePageIndex {
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }

    pub(crate) const fn get(self) -> u64 {
        self.0
    }
}

/// Complete cache identity; paths never identify file contents.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct CacheKey {
    filesystem: FilesystemGeneration,
    node: NodeIdentity,
    page: FilePageIndex,
}

impl CacheKey {
    pub(crate) const fn new(
        filesystem: FilesystemGeneration,
        node: NodeIdentity,
        page: FilePageIndex,
    ) -> Self {
        Self {
            filesystem,
            node,
            page,
        }
    }

    pub(crate) const fn filesystem(self) -> FilesystemGeneration {
        self.filesystem
    }

    pub(crate) const fn node(self) -> NodeIdentity {
        self.node
    }

    pub(crate) const fn page(self) -> FilePageIndex {
        self.page
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CacheError {
    Allocation,
    InvalidCapacity,
    SequenceExhausted,
    StaleLoad,
    /// A cache consumer detected an impossible published payload.
    Invariant,
}

impl From<state::Error> for CacheError {
    fn from(error: state::Error) -> Self {
        match error {
            state::Error::Allocation => Self::Allocation,
            state::Error::InvalidCapacity => Self::InvalidCapacity,
            state::Error::SequenceExhausted => Self::SequenceExhausted,
            state::Error::StaleLoad => Self::StaleLoad,
        }
    }
}

/// Read-only shared ownership of one published payload.
pub(crate) struct CachedPage<Page> {
    inner: FallibleArc<Page>,
}

impl<Page> CachedPage<Page> {
    pub(crate) fn value(&self) -> &Page {
        &self.inner
    }
}

impl<Page> Clone for CachedPage<Page> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

/// Result of a cache access before backend fill.
pub(crate) enum CacheAccess<'cache, Page> {
    Hit(CachedPage<Page>),
    /// Another caller owns the fill for this exact key.
    LoadInProgress,
    /// This caller owns the only admissible fill for the selected slot.
    Load(LoadReservation<'cache, Page>),
    /// Every bounded slot currently has an unfinished loader.
    CapacityBusy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CacheSnapshot {
    pub(crate) capacity: usize,
    pub(crate) clean: usize,
    pub(crate) loading: usize,
    pub(crate) hits: u64,
    pub(crate) misses: u64,
    pub(crate) evictions: u64,
    pub(crate) in_progress: u64,
    pub(crate) capacity_busy: u64,
}

struct CacheState<Page> {
    model: state::State,
    pages: Vec<Option<FallibleArc<Page>>>,
}

/// Fixed-capacity immutable file-data cache.
pub(crate) struct FileDataCache<Page> {
    state: CacheLock<CacheState<Page>>,
}

impl<Page> FileDataCache<Page> {
    pub(crate) fn try_new_system() -> Result<Self, CacheError> {
        Self::try_new_with_capacity(SYSTEM_PAGE_CAPACITY)
    }

    #[cfg(test)]
    pub(crate) fn try_new(capacity: usize) -> Result<Self, CacheError> {
        Self::try_new_with_capacity(capacity)
    }

    fn try_new_with_capacity(capacity: usize) -> Result<Self, CacheError> {
        let model = state::State::try_new(capacity)?;
        let mut pages = Vec::new();
        pages
            .try_reserve_exact(capacity)
            .map_err(|_| CacheError::Allocation)?;
        pages.resize_with(capacity, || None);
        Ok(Self {
            state: CacheLock::new(CacheState { model, pages }),
        })
    }

    /// Looks up a page or reserves exactly one generation-tagged fill.
    ///
    /// The cache lock covers only the bounded state transition and shared-owner
    /// clone. Backend access, allocation, copying, and destruction occur after
    /// the lock is released.
    pub(crate) fn access(&self, key: CacheKey) -> Result<CacheAccess<'_, Page>, CacheError> {
        let outcome = self
            .state
            .with(|cache| -> Result<LockedAccess<Page>, CacheError> {
                let access = cache.model.access(key)?;
                match access {
                    state::Access::Hit { slot } => {
                        let Some(page) = cache.pages.get(slot).and_then(Option::as_ref) else {
                            cache_invariant_violation();
                        };
                        Ok(LockedAccess::Hit(CachedPage {
                            inner: page.clone(),
                        }))
                    }
                    state::Access::LoadInProgress => Ok(LockedAccess::LoadInProgress),
                    state::Access::Reserved { token, evicted } => {
                        let Some(page) = cache.pages.get_mut(token.slot()) else {
                            cache_invariant_violation();
                        };
                        let old = page.take();
                        if old.is_some() != evicted {
                            cache_invariant_violation();
                        }
                        Ok(LockedAccess::Load { token, old })
                    }
                    state::Access::CapacityBusy => Ok(LockedAccess::CapacityBusy),
                }
            })?;
        Ok(match outcome {
            LockedAccess::Hit(page) => CacheAccess::Hit(page),
            LockedAccess::LoadInProgress => CacheAccess::LoadInProgress,
            LockedAccess::Load { token, old } => {
                // The replaced cache owner is released outside the cache lock.
                drop(old);
                CacheAccess::Load(LoadReservation {
                    cache: self,
                    token,
                    active: true,
                })
            }
            LockedAccess::CapacityBusy => CacheAccess::CapacityBusy,
        })
    }

    pub(crate) fn snapshot(&self) -> CacheSnapshot {
        let snapshot = self.state.with(|cache| cache.model.snapshot());
        CacheSnapshot {
            capacity: snapshot.capacity,
            clean: snapshot.clean,
            loading: snapshot.loading,
            hits: snapshot.hits,
            misses: snapshot.misses,
            evictions: snapshot.evictions,
            in_progress: snapshot.in_progress,
            capacity_busy: snapshot.capacity_busy,
        }
    }
}

enum LockedAccess<Page> {
    Hit(CachedPage<Page>),
    LoadInProgress,
    Load {
        token: state::LoadToken,
        old: Option<FallibleArc<Page>>,
    },
    CapacityBusy,
}

/// Linear ownership of one cache fill performed without the cache lock.
#[must_use = "publish the completed fill or let the reservation roll back"]
pub(crate) struct LoadReservation<'cache, Page> {
    cache: &'cache FileDataCache<Page>,
    token: state::LoadToken,
    active: bool,
}

impl<Page> LoadReservation<'_, Page> {
    /// Publishes a completely initialized immutable payload.
    ///
    /// Shared-owner allocation occurs before taking the cache lock. Failure
    /// returns the original payload and leaves Drop to roll back the loader.
    pub(crate) fn publish(mut self, page: Page) -> Result<CachedPage<Page>, PublishError<Page>> {
        let shared = match FallibleArc::try_new_or_return(page) {
            Ok(shared) => shared,
            Err((_, page)) => {
                return Err(PublishError {
                    cause: CacheError::Allocation,
                    page,
                });
            }
        };
        let result = self.cache.state.with(|cache| {
            cache.model.validate(self.token)?;
            let vacant = cache
                .pages
                .get(self.token.slot())
                .is_some_and(Option::is_none);
            if !vacant {
                cache_invariant_violation();
            }
            if cache.model.publish(self.token).is_err() {
                cache_invariant_violation();
            }
            let Some(slot) = cache.pages.get_mut(self.token.slot()) else {
                cache_invariant_violation();
            };
            *slot = Some(shared.clone());
            Ok::<(), CacheError>(())
        });
        match result {
            Ok(()) => {
                self.active = false;
                Ok(CachedPage { inner: shared })
            }
            Err(cause) => {
                self.active = false;
                let page = match shared.try_unwrap() {
                    Ok(page) => page,
                    Err(_) => cache_invariant_violation(),
                };
                Err(PublishError { cause, page })
            }
        }
    }

    /// Explicitly abandons a failed backend fill.
    pub(crate) fn abort(mut self) -> Result<(), CacheError> {
        let result = self
            .cache
            .state
            .with(|cache| cache.model.abort(self.token).map_err(Into::into));
        // A stale token no longer owns this slot, so it is equally important
        // not to retry its rollback from Drop.
        self.active = false;
        result
    }

    #[cfg(test)]
    pub(crate) fn invalidate_slot_for_test(&self) {
        self.cache
            .state
            .with(|cache| cache.model.invalidate_for_test(self.token.slot()));
    }
}

impl<Page> Drop for LoadReservation<'_, Page> {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        // Staleness means another generation already owns or cleared the
        // slot. The generation check prevents this abandoned fill from
        // affecting that newer owner.
        let _ = self.cache.state.with(|cache| cache.model.abort(self.token));
        self.active = false;
    }
}

#[derive(Debug)]
pub(crate) struct PublishError<Page> {
    cause: CacheError,
    page: Page,
}

impl<Page> PublishError<Page> {
    pub(crate) const fn cause(&self) -> CacheError {
        self.cause
    }

    pub(crate) fn into_page(self) -> Page {
        self.page
    }
}

#[cold]
fn cache_invariant_violation() -> ! {
    loop {
        core::hint::spin_loop();
    }
}
