// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native machine-root ownership, admission, and acknowledged replacement.

use alloc::vec::Vec;
use core::mem::{ManuallyDrop, size_of};
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use hyper::cpu::{PerCpu, PinnedExecution};
use hyper::mm::allocator::heap::PageOwner;
use hyper::mm::{AddressSpaceResidency, FallibleArc, PAGE_SIZE, ResidencyError, UniqueFallibleArc};
use hyper::sync::InterruptSpinLock;

use super::kernel_adapter::DomainCharge;
use super::{
    Access, AddressError, AddressSpaceError as LogicalError, DomainAccount, KernelPageBackend,
    KernelPageError, MappingChange, MemoryAccount, MemoryCharge, Permissions,
    PreparedMappingChange, PreparedPageSnapshot, PreparedUserWrite, UserAddress, UserAddressSpace,
    UserSlice, VmoError, WritableVmo,
};
use crate::kernel::accounting::{
    CommittedCharge, ResourceAmount, ResourceDomain, ResourceError, ResourceKind,
};
use crate::kernel::mm::page_block::PageBlock;
use crate::kernel::mm::translation_id::{
    ActiveIdentifier, HostAsid, IdentifierReservation, RetiringIdentifier, Stage2Vmid,
};

type LogicalAddressSpace = UserAddressSpace<KernelPageBackend, DomainAccount>;
type LogicalPrepared<'a> = PreparedMappingChange<'a, KernelPageBackend, DomainAccount>;
type LogicalUserWrite = PreparedUserWrite<KernelPageBackend, DomainAccount>;
type StateLock = InterruptSpinLock<MachineState, crate::hal::irq::LocalMask>;
type LogicalAddressSpaceError = LogicalError<KernelPageError, ResourceError>;

static ACTIVE_OWNER: PerCpu<AtomicU64> =
    PerCpu::new([const { AtomicU64::new(0) }; hyper::cpu::MAX_CPUS]);
static ACTIVE_EPOCH: PerCpu<AtomicU64> =
    PerCpu::new([const { AtomicU64::new(0) }; hyper::cpu::MAX_CPUS]);
static ACTIVE: PerCpu<AtomicBool> =
    PerCpu::new([const { AtomicBool::new(false) }; hyper::cpu::MAX_CPUS]);

#[derive(Debug)]
pub(crate) enum Error {
    Allocation,
    Address(AddressError),
    Hal(crate::hal::user::AddressSpaceError),
    Identifier(crate::kernel::mm::translation_id::Error),
    InvalidRange,
    Logical(LogicalAddressSpaceError),
    Page(hyper::mm::BuddyError),
    Residency(ResidencyError),
    Resource(ResourceError),
    SizeOverflow,
    Transport,
    Unsupported,
    Vmo(VmoError<KernelPageError, ResourceError>),
}

#[must_use = "recover and retry the exact native address-space owner"]
pub(crate) struct RetirementFailure {
    error: Error,
    owner: UniqueFallibleArc<NativeAddressSpace>,
}

impl RetirementFailure {
    pub(crate) fn into_parts(self) -> (Error, UniqueFallibleArc<NativeAddressSpace>) {
        (self.error, self.owner)
    }
}

impl From<crate::hal::user::AddressSpaceError> for Error {
    fn from(error: crate::hal::user::AddressSpaceError) -> Self {
        Self::Hal(error)
    }
}

impl From<crate::kernel::mm::translation_id::Error> for Error {
    fn from(error: crate::kernel::mm::translation_id::Error) -> Self {
        Self::Identifier(error)
    }
}

impl From<LogicalAddressSpaceError> for Error {
    fn from(error: LogicalAddressSpaceError) -> Self {
        Self::Logical(error)
    }
}

impl From<AddressError> for Error {
    fn from(error: AddressError) -> Self {
        Self::Address(error)
    }
}

impl From<VmoError<KernelPageError, ResourceError>> for Error {
    fn from(error: VmoError<KernelPageError, ResourceError>) -> Self {
        Self::Vmo(error)
    }
}

impl From<ResidencyError> for Error {
    fn from(error: ResidencyError) -> Self {
        Self::Residency(error)
    }
}

impl From<ResourceError> for Error {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}

type MachineIdentifier = crate::hal::user::AddressSpaceIdentifier<
    ActiveIdentifier<HostAsid>,
    ActiveIdentifier<Stage2Vmid>,
>;
type RetiringMachineIdentifier = crate::hal::user::AddressSpaceIdentifier<
    RetiringIdentifier<HostAsid>,
    RetiringIdentifier<Stage2Vmid>,
>;
type ReservedMachineIdentifier = crate::hal::user::AddressSpaceIdentifier<
    IdentifierReservation<HostAsid>,
    IdentifierReservation<Stage2Vmid>,
>;

fn activate_identifier(identifier: ReservedMachineIdentifier) -> Result<MachineIdentifier, Error> {
    identifier.try_map(
        |identifier| identifier.activate().map_err(Error::Identifier),
        |identifier| identifier.activate().map_err(Error::Identifier),
    )
}

fn begin_identifier_retirement(
    identifier: MachineIdentifier,
) -> Result<RetiringMachineIdentifier, Error> {
    identifier.try_map(
        |identifier| identifier.begin_retirement().map_err(Error::Identifier),
        |identifier| identifier.begin_retirement().map_err(Error::Identifier),
    )
}

unsafe fn complete_identifier_retirement(
    identifier: RetiringMachineIdentifier,
) -> Result<(), Error> {
    identifier
        .try_map(
            // SAFETY: The caller supplies acknowledged invalidation for the
            // matching host-stage identifier namespace.
            |identifier| unsafe { identifier.complete() }.map_err(Error::Identifier),
            // SAFETY: The same proof covers the shared native/guest VMID.
            |identifier| unsafe { identifier.complete() }.map_err(Error::Identifier),
        )
        .map(|_| ())
}

struct TablePage {
    // Drop the physical owner before releasing its resource-domain charge.
    _block: PageBlock,
    _charge: DomainCharge,
}

struct TablePagePool {
    pages: Vec<TablePage>,
    _storage_charge: DomainCharge,
    account: DomainAccount,
    error: Option<Error>,
}

impl TablePagePool {
    fn try_new(capacity: usize, account: DomainAccount) -> Result<Self, Error> {
        let bytes = capacity
            .checked_mul(size_of::<TablePage>())
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(Error::SizeOverflow)?;
        let storage_charge = account
            .try_charge(MemoryCharge {
                kernel_bytes: bytes,
                ..MemoryCharge::default()
            })
            .map_err(Error::Resource)?;
        let mut pages = Vec::new();
        pages
            .try_reserve_exact(capacity)
            .map_err(|_| Error::Allocation)?;
        Ok(Self {
            pages,
            _storage_charge: storage_charge,
            account,
            error: None,
        })
    }

    fn allocate(&mut self, order: usize) -> Option<hyper::mm::PhysicalAddress> {
        if order != 0 || self.pages.len() == self.pages.capacity() {
            self.error = Some(Error::Allocation);
            return None;
        }
        let charge = match self.account.try_charge(MemoryCharge {
            committed_pages: 1,
            pinned_pages: 1,
            ..MemoryCharge::default()
        }) {
            Ok(charge) => charge,
            Err(error) => {
                self.error = Some(Error::Resource(error));
                return None;
            }
        };
        let block = match PageBlock::allocate_for(0, PageOwner::PageTable) {
            Ok(block) => block,
            Err(error) => {
                self.error = Some(Error::Page(error));
                return None;
            }
        };
        let Some(address) = crate::kernel::mm::memory::linear_address(block.physical().get())
        else {
            self.error = Some(Error::Unsupported);
            return None;
        };
        // SAFETY: The new PageBlock is uniquely owned and the permanent linear
        // map covers its complete writable page.
        unsafe {
            core::ptr::with_exposed_provenance_mut::<u8>(address).write_bytes(0, PAGE_SIZE as usize)
        };
        let physical = block.physical();
        self.pages.push(TablePage {
            _block: block,
            _charge: charge,
        });
        Some(physical)
    }
}

struct MachineImage {
    epoch: u64,
    root: crate::hal::user::PreparedAddressSpace,
    _tables: TablePagePool,
    _object_charge: DomainCharge,
}

struct MachineState {
    current: FallibleArc<MachineImage>,
    residency: AddressSpaceResidency<{ hyper::cpu::MAX_CPUS }>,
}

/// Logical and machine state for one native userspace address space.
///
/// Safe Drop deliberately leaks the published root and identifier. A future
/// Process teardown path will consume an acknowledged final-retirement token;
/// freeing either owner without it would permit hardware use-after-free.
pub(crate) struct NativeAddressSpace {
    logical: ManuallyDrop<LogicalAddressSpace>,
    account: DomainAccount,
    identifier: ManuallyDrop<MachineIdentifier>,
    state: ManuallyDrop<StateLock>,
    root_vmar_object_published: AtomicBool,
    _owner_charge: CommittedCharge,
}

/// Unpublished, fully resident storage for one final image mapping.
///
/// Loader writes and relocations operate before this owner is consumed. Once
/// installed, executable data is copied into an immutable instruction-coherent
/// snapshot, preserving W^X even if another staging reference survives.
pub(crate) struct NativeImageSegment {
    range: UserSlice,
    permissions: Permissions,
    storage: WritableVmo<KernelPageBackend, DomainAccount>,
}

impl NativeImageSegment {
    pub(crate) fn try_new(
        address_space: &NativeAddressSpace,
        range: UserSlice,
        permissions: Permissions,
    ) -> Result<Self, Error> {
        if !permissions.is_valid() || permissions == Permissions::NONE {
            return Err(Error::Unsupported);
        }
        let storage = WritableVmo::try_new(
            range.length(),
            KernelPageBackend,
            address_space.account.clone(),
        )?;
        storage
            .populate(0, range.length())
            .map_err(|failure| Error::Vmo(failure.cause))?;
        Ok(Self {
            range,
            permissions,
            storage,
        })
    }

    pub(crate) const fn range(&self) -> UserSlice {
        self.range
    }

    pub(crate) fn write(&self, offset: u64, source: &[u8]) -> Result<(), Error> {
        self.storage.write(offset, source).map_err(Error::Vmo)
    }

    pub(crate) fn read_word(&self, address: UserAddress) -> Result<u64, Error> {
        let offset = self.relative_offset(address, size_of::<u64>())?;
        let mut bytes = [0u8; size_of::<u64>()];
        self.storage.read(offset, &mut bytes).map_err(Error::Vmo)?;
        Ok(u64::from_le_bytes(bytes))
    }

    pub(crate) fn write_word(&self, address: UserAddress, value: u64) -> Result<(), Error> {
        let offset = self.relative_offset(address, size_of::<u64>())?;
        self.storage
            .write(offset, &value.to_le_bytes())
            .map_err(Error::Vmo)
    }

    pub(crate) fn install(
        self,
        address_space: &NativeAddressSpace,
        pin: &(impl PinnedExecution + 'static),
    ) -> Result<(), Error> {
        let logical = address_space.logical();
        let prepared = if self.permissions.contains(Access::Execute) {
            let executable = self.storage.try_executable_snapshot(
                &super::ExecutableProvenance::for_native_image_loader(),
                pin,
            )?;
            logical.prepare_map_executable(
                logical.root_vmar(),
                self.range,
                executable,
                0,
                self.permissions,
                self.permissions,
            )?
        } else {
            logical.prepare_map_writable(
                logical.root_vmar(),
                self.range,
                self.storage,
                0,
                self.permissions,
                self.permissions,
            )?
        };
        address_space.prepare_change(prepared)?.commit()?;
        Ok(())
    }

    fn relative_offset(&self, address: UserAddress, length: usize) -> Result<u64, Error> {
        let length = u64::try_from(length).map_err(|_| Error::SizeOverflow)?;
        let requested = UserSlice::new(address, length)?;
        if !self.range.contains(requested) {
            return Err(Error::InvalidRange);
        }
        Ok(address.get() - self.range.base().get())
    }
}

impl NativeAddressSpace {
    pub(crate) fn try_new(
        domain: ResourceDomain,
        range: UserSlice,
    ) -> Result<UniqueFallibleArc<Self>, Error> {
        let plan = crate::hal::user::address_space_plan()?;
        let owner_bytes = u64::try_from(UniqueFallibleArc::<Self>::allocation_size())
            .map_err(|_| Error::SizeOverflow)?;
        let owner_charge = domain
            .reserve(
                ResourceAmount::ZERO
                    .with(ResourceKind::KernelObjects, 1)
                    .with(ResourceKind::KernelMemoryBytes, owner_bytes),
            )?
            .commit();
        // Reserve the final pinned owner before publishing any root or
        // identifier, so later allocation failure cannot strand hardware.
        let owner: UniqueFallibleArc<core::mem::MaybeUninit<Self>> =
            UniqueFallibleArc::try_new_uninit().map_err(|_| Error::Allocation)?;
        let window = super::address_window(plan.address_limit())?;
        let account = DomainAccount::new(domain);
        let logical = UserAddressSpace::try_new(window, range, KernelPageBackend, account.clone())?;
        let reserved = plan.reserve_identifier(
            crate::kernel::mm::translation_id::reserve,
            crate::kernel::mm::translation_id::reserve,
        )?;
        let initial_epoch = logical.mapping_epoch();
        let image = build_image(
            &reserved,
            |identifier| (identifier.value(), identifier.generation()),
            |identifier| (identifier.value(), identifier.generation()),
            initial_epoch,
            0,
            account.clone(),
            |_| {},
        )?;
        let identifier = activate_identifier(reserved)?;
        Ok(owner.write(Self {
            logical: ManuallyDrop::new(logical),
            account,
            identifier: ManuallyDrop::new(identifier),
            state: ManuallyDrop::new(InterruptSpinLock::new(MachineState {
                current: image,
                residency: AddressSpaceResidency::try_new(initial_epoch)?,
            })),
            root_vmar_object_published: AtomicBool::new(false),
            _owner_charge: owner_charge,
        }))
    }

    pub(super) fn logical(&self) -> &LogicalAddressSpace {
        &self.logical
    }

    pub(super) fn claim_root_vmar_object_publication(&self) -> bool {
        self.root_vmar_object_published
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    pub(super) fn abort_root_vmar_object_publication(&self) {
        if self
            .root_vmar_object_published
            .compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            crate::hal::cpu::halt();
        }
    }

    /// Copies kernel-owned bytes through the logical user-address contract.
    pub(crate) fn copy_to_user(&self, destination: UserSlice, source: &[u8]) -> Result<(), Error> {
        self.logical.copy_to_user(destination, source)?;
        Ok(())
    }

    /// Copies bytes from the current logical user-address mappings.
    pub(crate) fn copy_from_user(
        &self,
        source: UserSlice,
        destination: &mut [u8],
    ) -> Result<(), Error> {
        self.logical.copy_from_user(source, destination)?;
        Ok(())
    }

    /// Reserves a stable writable mapping for capability-returning syscalls.
    pub(crate) fn reserve_user_write(
        owner: FallibleArc<Self>,
        destination: UserSlice,
    ) -> Result<UserWriteReservation, Error> {
        let plan = owner.logical.prepare_user_write(destination)?;
        Ok(UserWriteReservation {
            owner,
            plan: Some(plan),
        })
    }

    pub(crate) fn prepare_change<'a>(
        &'a self,
        logical: LogicalPrepared<'a>,
    ) -> Result<PreparedNativeChange<'a>, Error> {
        let count = logical.snapshots().count();
        let bytes = count
            .checked_mul(size_of::<(
                super::MappingSnapshot,
                PreparedPageSnapshot<KernelPageBackend, DomainAccount>,
            )>())
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(Error::SizeOverflow)?;
        let _snapshot_charge = self
            .account
            .try_charge(MemoryCharge {
                kernel_bytes: bytes,
                ..MemoryCharge::default()
            })
            .map_err(Error::Resource)?;
        let mut mappings = Vec::new();
        mappings
            .try_reserve_exact(count)
            .map_err(|_| Error::Allocation)?;
        let mut leaf_count = 0usize;
        for snapshot in logical.snapshots() {
            let pages = logical.resident_pages(snapshot.token)?;
            leaf_count = leaf_count
                .checked_add(pages.pages().len())
                .ok_or(Error::SizeOverflow)?;
            mappings.push((snapshot, pages));
        }
        let image = build_image(
            &self.identifier,
            |identifier| (identifier.value(), identifier.generation()),
            |identifier| (identifier.value(), identifier.generation()),
            logical.next_epoch(),
            leaf_count,
            self.account.clone(),
            |visit| {
                for (snapshot, pages) in &mappings {
                    for (index, physical) in pages.pages().iter().copied().enumerate() {
                        let offset = (index as u64) * PAGE_SIZE;
                        visit(crate::hal::user::MappingPage {
                            address: snapshot.range.base().get() + offset,
                            physical,
                            readable: snapshot.permissions.contains(Access::Read),
                            writable: snapshot.permissions.contains(Access::Write),
                            executable: snapshot.permissions.contains(Access::Execute),
                        });
                    }
                }
            },
        )?;
        Ok(PreparedNativeChange {
            owner: self,
            logical,
            image,
        })
    }

    pub(crate) fn activate<'a>(
        &'a self,
        pin: &'a dyn PinnedExecution,
        kernel_access: &crate::hal::user::PreparedKernelAccess<'_>,
    ) -> Result<ActiveNativeAddressSpace<'a>, Error> {
        let cpu = crate::kernel::cpu::current_index().ok_or(Error::Unsupported)?;
        let owner = self.logical.id().get();
        if ACTIVE_OWNER[cpu]
            .compare_exchange(0, owner, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(Error::Residency(ResidencyError::AlreadyActive));
        }
        let backend = self.state.with(|state| {
            state
                .residency
                .check_admission(cpu.get(), state.current.epoch)?;
            // SAFETY: The state lock closes the update cut through the local
            // install and active publication; `pin` prevents CPU migration.
            let backend = unsafe {
                crate::hal::user::activate_local(
                    &state.current.root,
                    cpu,
                    pin,
                    self,
                    kernel_access,
                )?
            };
            ACTIVE_EPOCH[cpu].store(state.current.epoch, Ordering::Relaxed);
            ACTIVE[cpu].store(true, Ordering::Release);
            if let Err(error) = state.residency.publish_admission(cpu.get()) {
                ACTIVE[cpu].store(false, Ordering::Release);
                // SAFETY: Activation and rollback remain in one pinned,
                // IRQ-masked interval on `cpu`.
                let _ = unsafe { crate::hal::user::deactivate_local(backend) };
                return Err(Error::Residency(error));
            }
            Ok(backend)
        });
        let backend = match backend {
            Ok(backend) => backend,
            Err(error) => {
                ACTIVE_OWNER[cpu].store(0, Ordering::Release);
                return Err(error);
            }
        };
        Ok(ActiveNativeAddressSpace {
            owner: self,
            cpu,
            backend: Some(backend),
        })
    }

    /// Retires the last machine root and returns its hardware identifier to
    /// the allocator only after every previously resident CPU acknowledged a
    /// tagged invalidation.
    // Returning the owner is intentional: a recoverable pre-publication Busy
    // result must preserve the address space without allocation or leakage.
    pub(crate) fn retire(mut owner: UniqueFallibleArc<Self>) -> Result<(), RetirementFailure> {
        let mut transport =
            match crate::kernel::irq::cross_call::UserAddressSpaceTransaction::try_acquire() {
                Ok(transport) => transport,
                Err(()) => {
                    return Err(RetirementFailure {
                        error: Error::Transport,
                        owner,
                    });
                }
            };
        let cut_and_image = owner.state.with(|state| {
            let cut = state.residency.begin_retirement(state.current.epoch)?;
            Ok::<_, Error>((cut, state.current.clone()))
        });
        let (cut, image) = match cut_and_image {
            Ok(value) => value,
            Err(error) => return Err(RetirementFailure { error, owner }),
        };

        // SAFETY: `owner` is consumed, admission is permanently closed, and no
        // ActiveNativeAddressSpace borrow can coexist with this move.
        let identifier = unsafe { ManuallyDrop::take(&mut owner.identifier) };
        let retiring = match begin_identifier_retirement(identifier) {
            Ok(retiring) => retiring,
            Err(_) => crate::kernel::crash::fatal(format_args!(
                "HypeR: native translation identifier retirement is inconsistent"
            )),
        };
        let outcome = transport.execute(
            crate::kernel::irq::cross_call::UserAddressSpaceExecution {
                owner: owner.logical.id().get(),
                epoch: image.epoch,
                new_epoch: image.epoch,
                active_target: false,
                expected: &image.root,
                operation: crate::kernel::irq::cross_call::UserAddressSpaceOperation::Invalidate(
                    &image.root,
                ),
            },
            crate::kernel::cpu::online_cpu_count(),
            cut.targets(),
        );
        if outcome.rejected_cpu.is_some() || outcome.ambiguous_cpu.is_some() {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: native address-space final invalidation was not acknowledged"
            ));
        }
        if owner
            .state
            .with(|state| state.residency.finish_retirement(cut))
            .is_err()
        {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: native address-space residency retirement is inconsistent"
            ));
        }
        // SAFETY: Residency is now irreversibly retired and every target
        // acknowledged invalidating this exact identifier before reuse.
        if unsafe { complete_identifier_retirement(retiring) }.is_err() {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: native translation identifier completion is inconsistent"
            ));
        }

        // SAFETY: The final invalidation acknowledged before these published
        // owners are extracted and destroyed. ManuallyDrop prevents `Drop`
        // from observing or releasing the moved fields a second time.
        let state = unsafe { ManuallyDrop::take(&mut owner.state) };
        // SAFETY: The logical owner is protected by the same final quiescence.
        let logical = unsafe { ManuallyDrop::take(&mut owner.logical) };
        drop(image);
        drop(state);
        drop(logical);
        drop(owner);
        Ok(())
    }
}

/// Owned guard which prevents output mappings from changing before commit.
pub(crate) struct UserWriteReservation {
    owner: FallibleArc<NativeAddressSpace>,
    plan: Option<LogicalUserWrite>,
}

impl UserWriteReservation {
    pub(crate) fn copy_from(&self, source: &[u8]) -> Result<(), Error> {
        let Some(plan) = self.plan.as_ref() else {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: completed user-write reservation reused"
            ));
        };
        self.owner.logical.write_user_reservation(plan, source)?;
        Ok(())
    }

    pub(crate) fn complete(mut self) {
        self.release();
    }

    fn release(&mut self) {
        let plan = match self.plan.take() {
            Some(plan) => plan,
            None => return,
        };
        self.owner.logical.release_user_write(plan);
    }
}

impl Drop for UserWriteReservation {
    fn drop(&mut self) {
        self.release();
    }
}

impl Drop for NativeAddressSpace {
    fn drop(&mut self) {
        // ManuallyDrop fields intentionally retain published owners. See the
        // type-level teardown contract above.
    }
}

pub(crate) struct ActiveNativeAddressSpace<'owner> {
    owner: &'owner NativeAddressSpace,
    cpu: hyper::cpu::CpuIndex,
    backend: Option<crate::hal::user::ActiveAddressSpace<'owner>>,
}

impl<'owner> ActiveNativeAddressSpace<'owner> {
    pub(crate) fn run_user<'context>(
        mut self,
        context: &'context mut crate::hal::user::UserContext,
        binding: hyper::hal::user::UserRunBinding,
        kernel_access: crate::hal::user::PreparedKernelAccess<'_>,
        service: &hyper::hal::user::NativeCallService<'_>,
    ) -> StoppedNativeUser<'context, 'owner> {
        let Some(backend) = self.backend.take() else {
            crate::hal::cpu::halt();
        };
        match crate::hal::user::run_user(context, backend, binding, kernel_access, service) {
            Ok(stopped) => StoppedNativeUser {
                active: Some(self),
                stopped: Some(stopped),
            },
            Err(error) => {
                // HAL has already restored the preceding translation. This is
                // an invariant failure in a prepared context. Retain logical
                // residency and fail-stop rather than return a recoverable
                // error after losing its backend token.
                core::mem::forget(self);
                crate::kernel::crash::fatal(format_args!(
                    "HypeR: native-user architecture entry failed: {error:?}"
                ))
            }
        }
    }

    pub(crate) fn leave(mut self) -> Result<(), Error> {
        if crate::kernel::cpu::current_index() != Some(self.cpu) {
            return Err(Error::Hal(crate::hal::user::AddressSpaceError::InvalidCpu));
        }
        loop {
            let result = self.owner.state.with(|state| {
                let active_epoch = ACTIVE_EPOCH[self.cpu].load(Ordering::Acquire);
                match state.residency.leave(self.cpu.get(), active_epoch) {
                    Err(ResidencyError::Busy) => {
                        return Err(Error::Residency(ResidencyError::Busy));
                    }
                    Err(error) => return Err(Error::Residency(error)),
                    Ok(()) => {}
                }
                let Some(backend) = self.backend.take() else {
                    crate::hal::cpu::halt();
                };
                if backend.cpu() != self.cpu {
                    crate::hal::cpu::halt();
                }
                // SAFETY: The current-CPU check above, PinnedExecution borrow,
                // and non-Send token prove same-PE teardown.
                unsafe { crate::hal::user::deactivate_local(backend)? };
                ACTIVE[self.cpu].store(false, Ordering::Release);
                ACTIVE_OWNER[self.cpu].store(0, Ordering::Release);
                Ok(())
            });
            match result {
                Err(Error::Residency(ResidencyError::Busy)) => core::hint::spin_loop(),
                other => return other,
            }
        }
    }
}

/// Native-user exception captured while the process root remains installed.
#[must_use = "native user stop must restore the kernel translation before dispatch"]
pub(crate) struct StoppedNativeUser<'context, 'owner> {
    active: Option<ActiveNativeAddressSpace<'owner>>,
    stopped: Option<crate::hal::user::StoppedUser<'context, 'owner>>,
}

impl<'context> StoppedNativeUser<'context, '_> {
    pub(crate) fn leave(mut self) -> (crate::hal::user::UserExit<'context>, StoppedNativeRun) {
        let Some(stopped) = self.stopped.take() else {
            crate::hal::cpu::halt();
        };
        let (exit, backend, architecture) = stopped.release();
        let Some(mut active) = self.active.take() else {
            crate::hal::cpu::halt();
        };
        if active.backend.is_some() || backend.cpu() != active.cpu {
            crate::hal::cpu::halt();
        }
        active.backend = Some(backend);
        if active.leave().is_err() {
            // ActiveNativeAddressSpace::Drop already fail-stops if hardware
            // ownership could not be closed. Keep this branch explicit.
            crate::hal::cpu::halt();
        }
        (
            exit,
            StoppedNativeRun {
                binding: architecture.binding(),
            },
        )
    }
}

impl Drop for StoppedNativeUser<'_, '_> {
    fn drop(&mut self) {
        if self.stopped.is_some() || self.active.is_some() {
            crate::hal::cpu::halt();
        }
    }
}

/// Proof that both architecture publication and process translation are idle.
pub(crate) struct StoppedNativeRun {
    binding: hyper::hal::user::UserRunBinding,
}

impl StoppedNativeRun {
    pub(crate) const fn binding(&self) -> hyper::hal::user::UserRunBinding {
        self.binding
    }
}

impl Drop for ActiveNativeAddressSpace<'_> {
    fn drop(&mut self) {
        if self.backend.is_some() {
            crate::hal::cpu::halt();
        }
    }
}

#[must_use = "prepared logical and machine roots must commit together or roll back together"]
pub(crate) struct PreparedNativeChange<'owner> {
    owner: &'owner NativeAddressSpace,
    logical: LogicalPrepared<'owner>,
    image: FallibleArc<MachineImage>,
}

impl PreparedNativeChange<'_> {
    pub(crate) fn commit(self) -> Result<MappingChange, Error> {
        let Self {
            owner,
            logical,
            image,
        } = self;
        let mut transport =
            crate::kernel::irq::cross_call::UserAddressSpaceTransaction::try_acquire()
                .map_err(|()| Error::Transport)?;
        let cut = owner
            .state
            .with(|state| state.residency.begin_update(logical.base_epoch()))?;
        let committed = match logical.commit_machine() {
            Ok(committed) => committed,
            Err(error) => {
                if owner
                    .state
                    .with(|state| state.residency.abort_update(cut))
                    .is_err()
                {
                    crate::hal::cpu::halt();
                }
                return Err(Error::Logical(error));
            }
        };
        let change = committed.change();
        let old = owner
            .state
            .with(|state| core::mem::replace(&mut state.current, image));

        let current = owner.state.with(|state| state.current.clone());
        execute_cut(
            &mut transport,
            owner.logical.id().get(),
            change,
            true,
            &old.root,
            crate::kernel::irq::cross_call::UserAddressSpaceOperation::Replace(&current.root),
            cut.active(),
        );

        let mut inactive = *cut.targets();
        for (index, active) in cut.active().iter().copied().enumerate() {
            if active {
                inactive[index] = false;
            }
        }
        execute_cut(
            &mut transport,
            owner.logical.id().get(),
            change,
            false,
            &old.root,
            crate::kernel::irq::cross_call::UserAddressSpaceOperation::Invalidate(&old.root),
            &inactive,
        );

        if owner
            .state
            .with(|state| state.residency.finish_update(cut, change.epoch))
            .is_err()
        {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: native address-space residency completion is inconsistent"
            ));
        }
        // SAFETY: Both active root switches and every inactive-resident tagged
        // invalidation acknowledged before either old owner is released.
        unsafe { committed.complete_machine_retirement() };
        drop(old);
        Ok(change)
    }
}

fn execute_cut(
    transport: &mut crate::kernel::irq::cross_call::UserAddressSpaceTransaction,
    owner: u64,
    change: MappingChange,
    active_target: bool,
    expected: &crate::hal::user::PreparedAddressSpace,
    operation: crate::kernel::irq::cross_call::UserAddressSpaceOperation<'_>,
    targets: &[bool; hyper::cpu::MAX_CPUS],
) {
    if !targets.iter().copied().any(|target| target) {
        return;
    }
    let outcome = transport.execute(
        crate::kernel::irq::cross_call::UserAddressSpaceExecution {
            owner,
            epoch: change.previous_epoch,
            new_epoch: change.epoch,
            active_target,
            expected,
            operation,
        },
        crate::kernel::cpu::online_cpu_count(),
        targets,
    );
    if outcome.rejected_cpu.is_some() || outcome.ambiguous_cpu.is_some() {
        crate::kernel::crash::fatal(format_args!(
            "HypeR: committed native address-space shootdown was not acknowledged"
        ));
    }
}

fn build_image<HostStage, SecondStage>(
    identifier: &crate::hal::user::AddressSpaceIdentifier<HostStage, SecondStage>,
    host_identity: impl FnOnce(&HostStage) -> (u16, u64),
    second_identity: impl FnOnce(&SecondStage) -> (u16, u64),
    epoch: u64,
    leaf_count: usize,
    account: DomainAccount,
    enumerate: impl FnMut(&mut dyn FnMut(crate::hal::user::MappingPage)),
) -> Result<FallibleArc<MachineImage>, Error> {
    let capacity = identifier
        .table_page_capacity(leaf_count)
        .ok_or(Error::SizeOverflow)?;
    let mut tables = TablePagePool::try_new(capacity, account.clone())?;
    let root = {
        let mut allocator = |order| tables.allocate(order);
        // SAFETY: TablePagePool returns uniquely owned zeroed PageTable blocks
        // and is moved intact into the resulting image through acknowledged
        // retirement.
        unsafe {
            identifier.prepare_address_space(
                host_identity,
                second_identity,
                enumerate,
                &mut allocator,
            )
        }
    };
    let root = match root {
        Ok(root) => root,
        Err(error) => {
            return Err(match tables.error.take() {
                Some(allocation_error) => allocation_error,
                None => Error::Hal(error),
            });
        }
    };
    if let Some(error) = tables.error.take() {
        return Err(error);
    }
    let object_charge = account
        .try_charge(MemoryCharge {
            kernel_bytes: FallibleArc::<MachineImage>::allocation_size() as u64,
            kernel_objects: 1,
            ..MemoryCharge::default()
        })
        .map_err(Error::Resource)?;
    FallibleArc::try_new(MachineImage {
        epoch,
        root,
        _tables: tables,
        _object_charge: object_charge,
    })
    .map_err(|_| Error::Allocation)
}

pub(crate) fn service_local_rpc(
    owner: u64,
    epoch: u64,
    new_epoch: u64,
    active_target: bool,
    expected_active: crate::hal::user::LocalIdentity,
    request: crate::hal::user::LocalRequest,
) -> crate::kernel::irq::cross_call::LocalApply {
    let Some(cpu) = crate::kernel::cpu::current_index() else {
        return crate::kernel::irq::cross_call::LocalApply::Rejected;
    };
    let active = ACTIVE[cpu].load(Ordering::Acquire);
    if active_target {
        if !active
            || ACTIVE_OWNER[cpu].load(Ordering::Relaxed) != owner
            || ACTIVE_EPOCH[cpu].load(Ordering::Relaxed) != epoch
        {
            return crate::kernel::irq::cross_call::LocalApply::Rejected;
        }
        if !crate::hal::user::local_identity_is_active(expected_active) {
            return crate::kernel::irq::cross_call::LocalApply::Rejected;
        }
    } else if active && ACTIVE_OWNER[cpu].load(Ordering::Relaxed) == owner {
        return crate::kernel::irq::cross_call::LocalApply::Rejected;
    }
    // SAFETY: The closed synchronous transport retains the corresponding
    // MachineImage until this CPU acknowledges the request.
    if !unsafe { crate::hal::user::service_local_request(request) } {
        return crate::kernel::irq::cross_call::LocalApply::AppliedOrUnknown;
    }
    if active_target {
        ACTIVE_EPOCH[cpu].store(new_epoch, Ordering::Release);
    }
    crate::kernel::irq::cross_call::LocalApply::Applied
}

// SAFETY: NativeAddressSpace leaks on safe Drop and releases an old image only
// after every active/resident CPU acknowledged replacement. Its identifier is
// retained until a future acknowledged final-retirement path.
unsafe impl hyper::hal::user::UserTranslationOwner for NativeAddressSpace {}

#[cfg(feature = "kernel-self-test")]
pub(crate) fn prepare_native_entry_self_test(
    domain: ResourceDomain,
    range: UserSlice,
    code_range: UserSlice,
    stack_range: UserSlice,
    code: &[u8],
    pin: &(impl PinnedExecution + 'static),
) -> Result<UniqueFallibleArc<NativeAddressSpace>, Error> {
    if code.is_empty() || code.len() > PAGE_SIZE as usize {
        return Err(Error::SizeOverflow);
    }
    let native = NativeAddressSpace::try_new(domain, range)?;
    let source = WritableVmo::try_new(PAGE_SIZE, KernelPageBackend, native.account.clone())?;
    source.write(0, code)?;
    let executable = source
        .try_executable_snapshot(&super::ExecutableProvenance::for_kernel_self_test(), pin)?;
    let logical = native.logical();
    let code_change = logical.prepare_map_executable(
        logical.root_vmar(),
        code_range,
        executable,
        0,
        Permissions::read_execute(),
        Permissions::read_execute(),
    )?;
    native.prepare_change(code_change)?.commit()?;

    let stack = WritableVmo::try_new(PAGE_SIZE, KernelPageBackend, native.account.clone())?;
    stack
        .populate(0, PAGE_SIZE)
        .map_err(|failure| Error::Vmo(failure.cause))?;
    let stack_change = logical.prepare_map_writable(
        logical.root_vmar(),
        stack_range,
        stack,
        0,
        Permissions::read_write(),
        Permissions::read_write(),
    )?;
    native.prepare_change(stack_change)?.commit()?;
    Ok(native)
}

#[cfg(feature = "kernel-self-test")]
pub(crate) fn run_dormant_self_test() -> Result<(), Error> {
    let domain =
        ResourceDomain::try_new_root(crate::kernel::accounting::ResourceLimits::UNLIMITED)?;
    let range = UserSlice::new(UserAddress::new(0x30_0000), PAGE_SIZE)?;
    let native = NativeAddressSpace::try_new(domain, range)?;
    let vmo = WritableVmo::try_new(PAGE_SIZE, KernelPageBackend, native.account.clone())?;
    vmo.populate(0, PAGE_SIZE)
        .map_err(|error| Error::Vmo(error.cause))?;
    let logical = native.logical();
    let prepared = logical.prepare_map_writable(
        logical.root_vmar(),
        range,
        vmo,
        0,
        Permissions::read_write(),
        Permissions::read_write(),
    )?;
    native.prepare_change(prepared)?.commit()?;
    match NativeAddressSpace::retire(native) {
        Ok(()) => Ok(()),
        Err(failure) => {
            let (error, _owner) = failure.into_parts();
            Err(error)
        }
    }
}
