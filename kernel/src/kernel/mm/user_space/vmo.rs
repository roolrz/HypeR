// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Sparse physically owned VM objects and immutable executable snapshots.

use alloc::vec::Vec;
use core::mem::size_of;
use core::sync::atomic::{AtomicUsize, Ordering};

use hyper::mm::{AllocationError, FallibleArc, PAGE_SIZE, PhysicalAddress};
use hyper::sync::InterruptSpinLock;

use super::contract::{MemoryAccount, MemoryCharge, PageBackend};

#[cfg(not(test))]
type UserMemoryLock<T> = InterruptSpinLock<T, crate::hal::irq::LocalMask>;

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
type UserMemoryLock<T> = InterruptSpinLock<T, TestInterruptMask>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum VmoError<BackendError, AccountError> {
    Account(AccountError),
    Allocation,
    Backend(BackendError),
    Busy,
    InvalidRange,
    SizeOverflow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct VmoPopulateError<BackendError, AccountError> {
    pub(crate) cause: VmoError<BackendError, AccountError>,
    pub(crate) committed_pages: usize,
}

struct PagePreparationError<BackendError, AccountError> {
    cause: VmoError<BackendError, AccountError>,
    committed_pages: usize,
}

struct OwnedPage<Page, Charge> {
    page: Page,
    _charge: Charge,
}

type PageRef<Backend, Account> = FallibleArc<
    UserMemoryLock<OwnedPage<<Backend as PageBackend>::Page, <Account as MemoryAccount>::Charge>>,
>;

struct VmoState<Backend: PageBackend, Account: MemoryAccount> {
    pages: Vec<Option<PageRef<Backend, Account>>>,
}

type VmoLock<Backend, Account> = UserMemoryLock<VmoState<Backend, Account>>;
type VmoResult<Backend, Account, Value> =
    Result<Value, VmoError<<Backend as PageBackend>::Error, <Account as MemoryAccount>::Error>>;

struct VmoInner<Backend: PageBackend, Account: MemoryAccount> {
    size: u64,
    backend: Backend,
    account: Account,
    state: VmoLock<Backend, Account>,
    access_state: AtomicUsize,
    _metadata_charge: Account::Charge,
}

pub(crate) struct WritableVmo<Backend: PageBackend, Account: MemoryAccount> {
    inner: FallibleArc<VmoInner<Backend, Account>>,
}

pub(crate) struct ExecutableVmo<Backend: PageBackend, Account: MemoryAccount> {
    inner: FallibleArc<VmoInner<Backend, Account>>,
}

struct WritableMappingLeaseInner<Backend: PageBackend, Account: MemoryAccount> {
    vmo: FallibleArc<VmoInner<Backend, Account>>,
    _charge: Account::Charge,
}

pub(super) struct WritableMappingLease<Backend: PageBackend, Account: MemoryAccount> {
    inner: FallibleArc<WritableMappingLeaseInner<Backend, Account>>,
}

/// Opaque authority to publish bytes as native executable provenance.
pub(crate) struct ExecutableProvenance {
    _private: (),
}

#[cfg(test)]
impl ExecutableProvenance {
    pub(crate) const fn for_test() -> Self {
        Self { _private: () }
    }
}

#[cfg(feature = "kernel-self-test")]
impl ExecutableProvenance {
    pub(crate) const fn for_kernel_self_test() -> Self {
        Self { _private: () }
    }
}

impl ExecutableProvenance {
    pub(super) const fn for_native_image_loader() -> Self {
        Self { _private: () }
    }

    pub(super) const fn for_capability() -> Self {
        Self { _private: () }
    }
}

impl<Backend: PageBackend, Account: MemoryAccount> Clone for WritableVmo<Backend, Account> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<Backend: PageBackend, Account: MemoryAccount> Clone for ExecutableVmo<Backend, Account> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<Backend: PageBackend, Account: MemoryAccount> Clone
    for WritableMappingLease<Backend, Account>
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<Backend: PageBackend, Account: MemoryAccount> WritableVmo<Backend, Account> {
    pub(crate) fn try_new(
        size: u64,
        backend: Backend,
        account: Account,
    ) -> Result<Self, VmoError<Backend::Error, Account::Error>> {
        let page_count = page_count(size)?;
        let metadata = metadata_charge::<Backend, Account>(page_count)?;
        let metadata_charge = account.try_charge(metadata).map_err(VmoError::Account)?;
        let mut pages = Vec::new();
        pages
            .try_reserve_exact(page_count)
            .map_err(|_| VmoError::Allocation)?;
        pages.resize_with(page_count, || None);
        let inner = VmoInner {
            size,
            backend,
            account,
            state: UserMemoryLock::new(VmoState { pages }),
            access_state: AtomicUsize::new(0),
            _metadata_charge: metadata_charge,
        };
        Ok(Self {
            inner: FallibleArc::try_new(inner).map_err(map_allocation)?,
        })
    }

    pub(crate) fn size(&self) -> u64 {
        self.inner.size
    }

    /// Acquires persistent authority for a hardware-writable mapping.
    ///
    /// The lease remains live through address-space retirement, so executable
    /// snapshot admission cannot race a stale writable translation.
    pub(super) fn try_mapping_write_lease(
        &self,
    ) -> VmoResult<Backend, Account, WritableMappingLease<Backend, Account>> {
        let admission = MappingAdmission::acquire(&self.inner)?;
        let charge = self
            .inner
            .account
            .try_charge(MemoryCharge {
                kernel_bytes:
                    FallibleArc::<WritableMappingLeaseInner<Backend, Account>>::allocation_size()
                        as u64,
                kernel_objects: 1,
                ..MemoryCharge::default()
            })
            .map_err(VmoError::Account)?;
        let lease_inner = WritableMappingLeaseInner {
            vmo: self.inner.clone(),
            _charge: charge,
        };
        // Ownership of the admission moves to `lease_inner` before the
        // fallible allocation. FallibleArc drops its input on allocation
        // failure, so exactly one owner releases the admission on every path.
        admission.commit();
        let inner = FallibleArc::try_new(lease_inner).map_err(map_allocation)?;
        Ok(WritableMappingLease { inner })
    }

    pub(crate) fn read(
        &self,
        offset: u64,
        destination: &mut [u8],
    ) -> Result<(), VmoError<Backend::Error, Account::Error>> {
        let access = KernelAccessGuard::acquire(&self.inner)?;
        let result = read_owned_inner(&self.inner, offset, destination);
        drop(access);
        result
    }

    /// Explicitly commits stable physical backing for a page-aligned range.
    ///
    /// Failure reports how many pages this call committed before the failing
    /// allocation. Those pages remain valid VMO ownership; callers must not
    /// mistake this operation for a rollbackable mapping preparation.
    pub(crate) fn populate(
        &self,
        offset: u64,
        length: u64,
    ) -> Result<(), VmoPopulateError<Backend::Error, Account::Error>> {
        if length == 0 || !offset.is_multiple_of(PAGE_SIZE) || !length.is_multiple_of(PAGE_SIZE) {
            return Err(VmoPopulateError {
                cause: VmoError::InvalidRange,
                committed_pages: 0,
            });
        }
        let length_usize = usize::try_from(length).map_err(|_| VmoPopulateError {
            cause: VmoError::SizeOverflow,
            committed_pages: 0,
        })?;
        validate_range(self.inner.size, offset, length_usize).map_err(|cause| {
            VmoPopulateError {
                cause,
                committed_pages: 0,
            }
        })?;
        let (first, count) =
            covered_pages(offset, length_usize).map_err(|cause| VmoPopulateError {
                cause,
                committed_pages: 0,
            })?;
        self.prepare_pages(first, count)
            .map(|_| ())
            .map_err(|failure| VmoPopulateError {
                cause: failure.cause,
                committed_pages: failure.committed_pages,
            })
    }

    /// Ensures every page in the range has stable physical backing.
    ///
    /// Each page is admitted and published independently, keeping IRQ-masked
    /// metadata sections constant-time. A later allocation failure may leave
    /// an earlier page validly committed to this VMO, but never publishes a
    /// partial address-space mapping.
    fn prepare_pages(
        &self,
        first: usize,
        count: usize,
    ) -> Result<usize, PagePreparationError<Backend::Error, Account::Error>> {
        let end = first.checked_add(count).ok_or(PagePreparationError {
            cause: VmoError::SizeOverflow,
            committed_pages: 0,
        })?;
        let mut committed_pages = 0;
        for index in first..end {
            let present = self
                .inner
                .state
                .with(|state| {
                    state
                        .pages
                        .get(index)
                        .map(Option::is_some)
                        .ok_or(VmoError::InvalidRange)
                })
                .map_err(|cause| PagePreparationError {
                    cause,
                    committed_pages,
                })?;
            if present {
                continue;
            }
            let charge = self
                .inner
                .account
                .try_charge(MemoryCharge {
                    kernel_bytes: FallibleArc::<
                        UserMemoryLock<OwnedPage<Backend::Page, Account::Charge>>,
                    >::allocation_size() as u64,
                    committed_pages: 1,
                    ..MemoryCharge::default()
                })
                .map_err(|error| PagePreparationError {
                    cause: VmoError::Account(error),
                    committed_pages,
                })?;
            let page =
                self.inner
                    .backend
                    .allocate_zeroed()
                    .map_err(|error| PagePreparationError {
                        cause: VmoError::Backend(error),
                        committed_pages,
                    })?;
            let page = FallibleArc::try_new(UserMemoryLock::new(OwnedPage {
                page,
                _charge: charge,
            }))
            .map_err(|error| PagePreparationError {
                cause: map_allocation(error),
                committed_pages,
            })?;
            let loser = self.inner.state.with(|state| {
                let slot = match state.pages.get_mut(index) {
                    Some(slot) => slot,
                    None => return Some(page),
                };
                if slot.is_none() {
                    *slot = Some(page);
                    None
                } else {
                    Some(page)
                }
            });
            // Page/accounting destruction is deliberately outside the VMO
            // lock when another first-touch won publication.
            let installed = loser.is_none();
            drop(loser);
            if installed {
                committed_pages += 1;
            }
        }
        Ok(committed_pages)
    }

    /// Writes object bytes while excluding snapshots and writable mappings.
    ///
    /// A read-only machine mapping may appear or remain active concurrently,
    /// so this path deliberately uses the backend's exposed-memory primitive.
    /// The operation may leave an earlier prefix modified on backend failure.
    pub(crate) fn write(
        &self,
        offset: u64,
        source: &[u8],
    ) -> Result<(), VmoError<Backend::Error, Account::Error>> {
        validate_range(self.inner.size, offset, source.len())?;
        let access = KernelAccessGuard::acquire(&self.inner)?;
        let (first, count) = covered_pages(offset, source.len())?;
        self.prepare_pages(first, count)
            .map_err(|failure| failure.cause)?;
        visit_chunks(
            offset,
            source.len(),
            |index, page_offset, source_offset, length| {
                let page = page_ref(&self.inner, index)?;
                page.with(|owned| {
                    self.inner
                        .backend
                        .write_exposed(
                            &mut owned.page,
                            page_offset,
                            &source[source_offset..source_offset + length],
                        )
                        .map_err(VmoError::Backend)
                })
            },
        )?;
        drop(access);
        Ok(())
    }

    /// Produces a coherent, fully backed, physically distinct image.
    ///
    /// Snapshot admission excludes new writers and waits for no context: it
    /// reports `Busy` while a writer is active. The returned type exposes no
    /// write operation, and no source page is shared with executable backing.
    pub(crate) fn try_executable_snapshot(
        &self,
        _provenance: &ExecutableProvenance,
        publication_context: &Backend::InstructionPublicationContext,
    ) -> Result<ExecutableVmo<Backend, Account>, VmoError<Backend::Error, Account::Error>> {
        let freeze = SnapshotGuard::acquire(&self.inner)?;
        let snapshot = WritableVmo::try_new(
            self.inner.size,
            self.inner.backend.clone(),
            self.inner.account.clone(),
        )?;
        let page_count = page_count(self.inner.size)?;
        snapshot
            .prepare_pages(0, page_count)
            .map_err(|failure| failure.cause)?;
        let mut buffer = [0u8; PAGE_SIZE as usize];
        for index in 0..page_count {
            let offset = (index as u64)
                .checked_mul(PAGE_SIZE)
                .ok_or(VmoError::SizeOverflow)?;
            let length = usize::try_from((self.inner.size - offset).min(PAGE_SIZE))
                .map_err(|_| VmoError::SizeOverflow)?;
            read_owned_inner(&self.inner, offset, &mut buffer[..length])?;
            snapshot.write_without_writer_guard(offset, &buffer[..length])?;
            buffer.fill(0);
        }
        drop(freeze);
        snapshot.publish_instruction_pages(publication_context)?;
        Ok(ExecutableVmo {
            inner: snapshot.inner,
        })
    }

    fn write_without_writer_guard(
        &self,
        offset: u64,
        source: &[u8],
    ) -> Result<(), VmoError<Backend::Error, Account::Error>> {
        visit_chunks(
            offset,
            source.len(),
            |index, page_offset, source_offset, length| {
                let page = page_ref(&self.inner, index)?;
                page.with(|owned| {
                    self.inner
                        .backend
                        .write_owned(
                            &mut owned.page,
                            page_offset,
                            &source[source_offset..source_offset + length],
                        )
                        .map_err(VmoError::Backend)
                })
            },
        )
    }

    fn publish_instruction_pages(
        &self,
        publication_context: &Backend::InstructionPublicationContext,
    ) -> Result<(), VmoError<Backend::Error, Account::Error>> {
        let count = page_count(self.inner.size)?;
        let mut missing = false;
        self.inner
            .backend
            .publish_instruction_pages(publication_context, |visit| {
                for index in 0..count {
                    let Some(page) = optional_page_ref(&self.inner, index).ok().flatten() else {
                        missing = true;
                        continue;
                    };
                    page.with(|owned| visit(&mut owned.page));
                }
            })
            .map_err(VmoError::Backend)?;
        if missing {
            return Err(VmoError::InvalidRange);
        }
        Ok(())
    }
}

impl<Backend: PageBackend, Account: MemoryAccount> ExecutableVmo<Backend, Account> {
    pub(crate) fn size(&self) -> u64 {
        self.inner.size
    }

    pub(crate) fn read(
        &self,
        offset: u64,
        destination: &mut [u8],
    ) -> Result<(), VmoError<Backend::Error, Account::Error>> {
        read_owned_inner(&self.inner, offset, destination)
    }
}

impl<Backend: PageBackend, Account: MemoryAccount> Drop
    for WritableMappingLeaseInner<Backend, Account>
{
    fn drop(&mut self) {
        release_mapping_admission(&self.vmo);
    }
}

pub(super) enum MappingObject<Backend: PageBackend, Account: MemoryAccount> {
    Writable(WritableVmo<Backend, Account>),
    Executable(ExecutableVmo<Backend, Account>),
}

impl<Backend: PageBackend, Account: MemoryAccount> Clone for MappingObject<Backend, Account> {
    fn clone(&self) -> Self {
        match self {
            Self::Writable(vmo) => Self::Writable(vmo.clone()),
            Self::Executable(vmo) => Self::Executable(vmo.clone()),
        }
    }
}

impl<Backend: PageBackend, Account: MemoryAccount> MappingObject<Backend, Account> {
    pub(super) fn size(&self) -> u64 {
        match self {
            Self::Writable(vmo) => vmo.size(),
            Self::Executable(vmo) => vmo.size(),
        }
    }

    pub(super) fn executable(&self) -> bool {
        matches!(self, Self::Executable(_))
    }

    pub(super) fn try_write_lease(
        &self,
    ) -> VmoResult<Backend, Account, WritableMappingLease<Backend, Account>> {
        match self {
            Self::Writable(vmo) => vmo.try_mapping_write_lease(),
            Self::Executable(_) => Err(VmoError::InvalidRange),
        }
    }

    pub(super) fn fully_resident(
        &self,
        offset: u64,
        length: u64,
    ) -> Result<bool, VmoError<Backend::Error, Account::Error>> {
        let length = usize::try_from(length).map_err(|_| VmoError::SizeOverflow)?;
        let (first, count) = covered_pages(offset, length)?;
        let inner = match self {
            Self::Writable(vmo) => &vmo.inner,
            Self::Executable(vmo) => &vmo.inner,
        };
        Ok(resident_count(inner, first, count)? == count)
    }

    pub(super) fn read_exposed(
        &self,
        offset: u64,
        destination: &mut [u8],
    ) -> Result<(), VmoError<Backend::Error, Account::Error>> {
        match self {
            Self::Writable(vmo) => read_exposed_inner(&vmo.inner, offset, destination),
            Self::Executable(vmo) => read_exposed_inner(&vmo.inner, offset, destination),
        }
    }

    pub(super) fn write_exposed(
        &self,
        offset: u64,
        source: &[u8],
    ) -> Result<(), VmoError<Backend::Error, Account::Error>> {
        match self {
            Self::Writable(vmo) => write_exposed_inner(&vmo.inner, offset, source),
            Self::Executable(_) => Err(VmoError::InvalidRange),
        }
    }
}

struct KernelAccessGuard<'a, Backend: PageBackend, Account: MemoryAccount> {
    inner: &'a VmoInner<Backend, Account>,
}

struct MappingAdmission<'a, Backend: PageBackend, Account: MemoryAccount> {
    inner: &'a VmoInner<Backend, Account>,
    armed: bool,
}

impl<'a, Backend: PageBackend, Account: MemoryAccount> MappingAdmission<'a, Backend, Account> {
    fn acquire(inner: &'a VmoInner<Backend, Account>) -> VmoResult<Backend, Account, Self> {
        acquire_mapping_admission(inner)?;
        Ok(Self { inner, armed: true })
    }

    fn commit(mut self) {
        self.armed = false;
    }
}

impl<Backend: PageBackend, Account: MemoryAccount> Drop for MappingAdmission<'_, Backend, Account> {
    fn drop(&mut self) {
        if self.armed {
            release_mapping_admission(self.inner);
        }
    }
}

impl<'a, Backend: PageBackend, Account: MemoryAccount> KernelAccessGuard<'a, Backend, Account> {
    fn acquire(
        inner: &'a VmoInner<Backend, Account>,
    ) -> Result<Self, VmoError<Backend::Error, Account::Error>> {
        acquire_kernel_access(inner)?;
        Ok(Self { inner })
    }
}

impl<Backend: PageBackend, Account: MemoryAccount> Drop
    for KernelAccessGuard<'_, Backend, Account>
{
    fn drop(&mut self) {
        release_kernel_access(self.inner);
    }
}

fn acquire_kernel_access<Backend: PageBackend, Account: MemoryAccount>(
    inner: &VmoInner<Backend, Account>,
) -> VmoResult<Backend, Account, ()> {
    let mut current = inner.access_state.load(Ordering::Relaxed);
    loop {
        if current & (SNAPSHOT_BIT | MAPPING_ACCESS_MASK) != 0 {
            return Err(VmoError::Busy);
        }
        let next = current.checked_add(1).ok_or(VmoError::SizeOverflow)?;
        if next & !KERNEL_ACCESS_MASK != 0 {
            return Err(VmoError::SizeOverflow);
        }
        match inner.access_state.compare_exchange_weak(
            current,
            next,
            Ordering::Acquire,
            Ordering::Relaxed,
        ) {
            Ok(_) => return Ok(()),
            Err(observed) => current = observed,
        }
    }
}

fn release_kernel_access<Backend: PageBackend, Account: MemoryAccount>(
    inner: &VmoInner<Backend, Account>,
) {
    let previous = inner.access_state.fetch_sub(1, Ordering::Release);
    if previous & KERNEL_ACCESS_MASK == 0 || previous & !KERNEL_ACCESS_MASK != 0 {
        vmo_invariant_violation();
    }
}

fn acquire_mapping_admission<Backend: PageBackend, Account: MemoryAccount>(
    inner: &VmoInner<Backend, Account>,
) -> VmoResult<Backend, Account, ()> {
    let mut current = inner.access_state.load(Ordering::Relaxed);
    loop {
        if current & (SNAPSHOT_BIT | KERNEL_ACCESS_MASK) != 0 {
            return Err(VmoError::Busy);
        }
        let next = current
            .checked_add(MAPPING_UNIT)
            .ok_or(VmoError::SizeOverflow)?;
        if next & SNAPSHOT_BIT != 0 {
            return Err(VmoError::SizeOverflow);
        }
        match inner.access_state.compare_exchange_weak(
            current,
            next,
            Ordering::Acquire,
            Ordering::Relaxed,
        ) {
            Ok(_) => return Ok(()),
            Err(observed) => current = observed,
        }
    }
}

fn release_mapping_admission<Backend: PageBackend, Account: MemoryAccount>(
    inner: &VmoInner<Backend, Account>,
) {
    let previous = inner
        .access_state
        .fetch_sub(MAPPING_UNIT, Ordering::Release);
    if previous & MAPPING_ACCESS_MASK == 0 || previous & (SNAPSHOT_BIT | KERNEL_ACCESS_MASK) != 0 {
        vmo_invariant_violation();
    }
}

struct SnapshotGuard<'a, Backend: PageBackend, Account: MemoryAccount> {
    inner: &'a VmoInner<Backend, Account>,
}

impl<'a, Backend: PageBackend, Account: MemoryAccount> SnapshotGuard<'a, Backend, Account> {
    fn acquire(
        inner: &'a VmoInner<Backend, Account>,
    ) -> Result<Self, VmoError<Backend::Error, Account::Error>> {
        inner
            .access_state
            .compare_exchange(0, SNAPSHOT_BIT, Ordering::Acquire, Ordering::Relaxed)
            .map(|_| Self { inner })
            .map_err(|_| VmoError::Busy)
    }
}

impl<Backend: PageBackend, Account: MemoryAccount> Drop for SnapshotGuard<'_, Backend, Account> {
    fn drop(&mut self) {
        if self
            .inner
            .access_state
            .compare_exchange(SNAPSHOT_BIT, 0, Ordering::Release, Ordering::Relaxed)
            .is_err()
        {
            vmo_invariant_violation();
        }
    }
}

fn read_owned_inner<Backend: PageBackend, Account: MemoryAccount>(
    inner: &VmoInner<Backend, Account>,
    offset: u64,
    destination: &mut [u8],
) -> Result<(), VmoError<Backend::Error, Account::Error>> {
    validate_range(inner.size, offset, destination.len())?;
    visit_chunks(
        offset,
        destination.len(),
        |index, page_offset, target_offset, length| {
            let Some(page) = optional_page_ref(inner, index)? else {
                destination[target_offset..target_offset + length].fill(0);
                return Ok(());
            };
            page.with(|owned| {
                inner
                    .backend
                    .read_owned(
                        &owned.page,
                        page_offset,
                        &mut destination[target_offset..target_offset + length],
                    )
                    .map_err(VmoError::Backend)
            })
        },
    )
}

fn read_exposed_inner<Backend: PageBackend, Account: MemoryAccount>(
    inner: &VmoInner<Backend, Account>,
    offset: u64,
    destination: &mut [u8],
) -> VmoResult<Backend, Account, ()> {
    validate_range(inner.size, offset, destination.len())?;
    visit_chunks(
        offset,
        destination.len(),
        |index, page_offset, target_offset, length| {
            let page = page_ref(inner, index)?;
            page.with(|owned| {
                inner
                    .backend
                    .read_exposed(
                        &owned.page,
                        page_offset,
                        &mut destination[target_offset..target_offset + length],
                    )
                    .map_err(VmoError::Backend)
            })
        },
    )
}

fn write_exposed_inner<Backend: PageBackend, Account: MemoryAccount>(
    inner: &VmoInner<Backend, Account>,
    offset: u64,
    source: &[u8],
) -> VmoResult<Backend, Account, ()> {
    validate_range(inner.size, offset, source.len())?;
    visit_chunks(
        offset,
        source.len(),
        |index, page_offset, source_offset, length| {
            let page = page_ref(inner, index)?;
            page.with(|owned| {
                inner
                    .backend
                    .write_exposed(
                        &mut owned.page,
                        page_offset,
                        &source[source_offset..source_offset + length],
                    )
                    .map_err(VmoError::Backend)
            })
        },
    )
}

fn optional_page_ref<Backend: PageBackend, Account: MemoryAccount>(
    inner: &VmoInner<Backend, Account>,
    index: usize,
) -> VmoResult<Backend, Account, Option<PageRef<Backend, Account>>> {
    inner.state.with(|state| {
        state
            .pages
            .get(index)
            .ok_or(VmoError::InvalidRange)
            .cloned()
    })
}

fn resident_count<Backend: PageBackend, Account: MemoryAccount>(
    inner: &VmoInner<Backend, Account>,
    first: usize,
    count: usize,
) -> VmoResult<Backend, Account, usize> {
    let end = first.checked_add(count).ok_or(VmoError::SizeOverflow)?;
    let mut resident = 0;
    for index in first..end {
        if optional_page_ref(inner, index)?.is_some() {
            resident += 1;
        }
    }
    Ok(resident)
}

fn page_ref<Backend: PageBackend, Account: MemoryAccount>(
    inner: &VmoInner<Backend, Account>,
    index: usize,
) -> Result<PageRef<Backend, Account>, VmoError<Backend::Error, Account::Error>> {
    optional_page_ref(inner, index)?.ok_or(VmoError::InvalidRange)
}

pub(super) fn resident_pages<Backend: PageBackend, Account: MemoryAccount>(
    object: &MappingObject<Backend, Account>,
    offset: u64,
    length: u64,
) -> VmoResult<Backend, Account, Vec<PhysicalAddress>> {
    let inner = match object {
        MappingObject::Writable(vmo) => &vmo.inner,
        MappingObject::Executable(vmo) => &vmo.inner,
    };
    let length_usize = usize::try_from(length).map_err(|_| VmoError::SizeOverflow)?;
    validate_range(inner.size, offset, length_usize)?;
    let (first, count) = covered_pages(offset, length_usize)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| VmoError::Allocation)?;
    let end = first.checked_add(count).ok_or(VmoError::SizeOverflow)?;
    for index in first..end {
        let page = optional_page_ref(inner, index)?;
        let page = page.ok_or(VmoError::InvalidRange)?;
        let physical = page.with(|owned| inner.backend.physical_address(&owned.page));
        result.push(physical);
    }
    Ok(result)
}

fn page_count<BackendError, AccountError>(
    size: u64,
) -> Result<usize, VmoError<BackendError, AccountError>> {
    if size == 0 {
        return Err(VmoError::InvalidRange);
    }
    if !size.is_multiple_of(PAGE_SIZE) {
        return Err(VmoError::InvalidRange);
    }
    let rounded = size
        .checked_add(PAGE_SIZE - 1)
        .ok_or(VmoError::SizeOverflow)?;
    usize::try_from(rounded / PAGE_SIZE).map_err(|_| VmoError::SizeOverflow)
}

fn covered_pages<BackendError, AccountError>(
    offset: u64,
    length: usize,
) -> Result<(usize, usize), VmoError<BackendError, AccountError>> {
    if length == 0 {
        return Ok((0, 0));
    }
    let first = usize::try_from(offset / PAGE_SIZE).map_err(|_| VmoError::SizeOverflow)?;
    let length = u64::try_from(length).map_err(|_| VmoError::SizeOverflow)?;
    let end = offset.checked_add(length).ok_or(VmoError::SizeOverflow)?;
    let last = usize::try_from((end - 1) / PAGE_SIZE).map_err(|_| VmoError::SizeOverflow)?;
    let count = last
        .checked_sub(first)
        .and_then(|distance| distance.checked_add(1))
        .ok_or(VmoError::SizeOverflow)?;
    Ok((first, count))
}

fn metadata_charge<Backend: PageBackend, Account: MemoryAccount>(
    pages: usize,
) -> Result<MemoryCharge, VmoError<Backend::Error, Account::Error>> {
    let slots = pages
        .checked_mul(size_of::<Option<PageRef<Backend, Account>>>())
        .ok_or(VmoError::SizeOverflow)?;
    let bytes = FallibleArc::<VmoInner<Backend, Account>>::allocation_size()
        .checked_add(slots)
        .ok_or(VmoError::SizeOverflow)?;
    Ok(MemoryCharge {
        kernel_bytes: u64::try_from(bytes).map_err(|_| VmoError::SizeOverflow)?,
        kernel_objects: 1,
        ..MemoryCharge::default()
    })
}

fn validate_range<BackendError, AccountError>(
    size: u64,
    offset: u64,
    length: usize,
) -> Result<(), VmoError<BackendError, AccountError>> {
    let length = u64::try_from(length).map_err(|_| VmoError::SizeOverflow)?;
    let end = offset.checked_add(length).ok_or(VmoError::SizeOverflow)?;
    if end > size {
        return Err(VmoError::InvalidRange);
    }
    Ok(())
}

fn visit_chunks<BackendError, AccountError>(
    mut offset: u64,
    length: usize,
    mut operation: impl FnMut(
        usize,
        usize,
        usize,
        usize,
    ) -> Result<(), VmoError<BackendError, AccountError>>,
) -> Result<(), VmoError<BackendError, AccountError>> {
    let mut visited = 0;
    while visited < length {
        let index = usize::try_from(offset / PAGE_SIZE).map_err(|_| VmoError::SizeOverflow)?;
        let page_offset =
            usize::try_from(offset % PAGE_SIZE).map_err(|_| VmoError::SizeOverflow)?;
        let chunk = (PAGE_SIZE as usize - page_offset).min(length - visited);
        operation(index, page_offset, visited, chunk)?;
        visited = visited.checked_add(chunk).ok_or(VmoError::SizeOverflow)?;
        offset = offset
            .checked_add(chunk as u64)
            .ok_or(VmoError::SizeOverflow)?;
    }
    Ok(())
}

fn map_allocation<BackendError, AccountError>(
    _: AllocationError,
) -> VmoError<BackendError, AccountError> {
    VmoError::Allocation
}

const SNAPSHOT_BIT: usize = 1usize << (usize::BITS - 1);
const MAPPING_UNIT: usize = 1usize << (usize::BITS / 2);
const KERNEL_ACCESS_MASK: usize = MAPPING_UNIT - 1;
const MAPPING_ACCESS_MASK: usize = SNAPSHOT_BIT - MAPPING_UNIT;

#[cold]
fn vmo_invariant_violation() -> ! {
    loop {
        core::hint::spin_loop();
    }
}
