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
    PreparedMappingChange, PreparedPageSnapshot, UserAddress, UserAddressSpace, UserSlice,
    VmoError, WritableVmo,
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
    Logical(LogicalAddressSpaceError),
    Page(hyper::mm::BuddyError),
    Residency(ResidencyError),
    Resource(ResourceError),
    SizeOverflow,
    Transport,
    Unsupported,
    Vmo(VmoError<KernelPageError, ResourceError>),
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

enum MachineIdentifier {
    Vhe(ActiveIdentifier<HostAsid>),
    Nvhe(ActiveIdentifier<Stage2Vmid>),
}

enum RetiringMachineIdentifier {
    Vhe(RetiringIdentifier<HostAsid>),
    Nvhe(RetiringIdentifier<Stage2Vmid>),
}

impl MachineIdentifier {
    fn value(&self) -> u16 {
        match self {
            Self::Vhe(identifier) => identifier.value(),
            Self::Nvhe(identifier) => identifier.value(),
        }
    }

    fn generation(&self) -> u64 {
        match self {
            Self::Vhe(identifier) => identifier.generation(),
            Self::Nvhe(identifier) => identifier.generation(),
        }
    }

    fn begin_retirement(self) -> Result<RetiringMachineIdentifier, Error> {
        match self {
            Self::Vhe(identifier) => identifier
                .begin_retirement()
                .map(RetiringMachineIdentifier::Vhe),
            Self::Nvhe(identifier) => identifier
                .begin_retirement()
                .map(RetiringMachineIdentifier::Nvhe),
        }
        .map_err(Into::into)
    }
}

impl RetiringMachineIdentifier {
    unsafe fn complete(self) -> Result<(), Error> {
        match self {
            // SAFETY: The caller supplies the acknowledged invalidation proof
            // for the matching architecture translation namespace.
            Self::Vhe(identifier) => unsafe { identifier.complete() },
            // SAFETY: The same proof covers the shared native/guest VMID.
            Self::Nvhe(identifier) => unsafe { identifier.complete() },
        }
        .map_err(Into::into)
    }
}

enum ReservedMachineIdentifier {
    Vhe(IdentifierReservation<HostAsid>),
    Nvhe(IdentifierReservation<Stage2Vmid>),
}

impl ReservedMachineIdentifier {
    fn value(&self) -> u16 {
        match self {
            Self::Vhe(identifier) => identifier.value(),
            Self::Nvhe(identifier) => identifier.value(),
        }
    }

    fn generation(&self) -> u64 {
        match self {
            Self::Vhe(identifier) => identifier.generation(),
            Self::Nvhe(identifier) => identifier.generation(),
        }
    }

    fn activate(self) -> Result<MachineIdentifier, Error> {
        match self {
            Self::Vhe(identifier) => identifier.activate().map(MachineIdentifier::Vhe),
            Self::Nvhe(identifier) => identifier.activate().map(MachineIdentifier::Nvhe),
        }
        .map_err(Into::into)
    }
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
    _owner_charge: CommittedCharge,
}

impl NativeAddressSpace {
    pub(crate) fn try_new(
        domain: ResourceDomain,
        range: UserSlice,
    ) -> Result<UniqueFallibleArc<Self>, Error> {
        #[cfg(not(CONFIG_ARCH_AARCH64))]
        {
            let _ = (domain, range);
            Err(Error::Unsupported)
        }
        #[cfg(CONFIG_ARCH_AARCH64)]
        {
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
            let kind = crate::hal::user::translation_kind()?;
            let window = super::address_window().map_err(|_| Error::Unsupported)?;
            let account = DomainAccount::new(domain);
            let logical =
                UserAddressSpace::try_new(window, range, KernelPageBackend, account.clone())?;
            let reserved = match kind {
                crate::hal::user::TranslationKind::VheHostStage1 => {
                    ReservedMachineIdentifier::Vhe(crate::kernel::mm::translation_id::reserve(8)?)
                }
                crate::hal::user::TranslationKind::NvheStage2Only => {
                    ReservedMachineIdentifier::Nvhe(crate::kernel::mm::translation_id::reserve(8)?)
                }
            };
            let initial_epoch = logical.mapping_epoch();
            let image = build_image(
                kind,
                reserved.value(),
                reserved.generation(),
                initial_epoch,
                0,
                account.clone(),
                |_| {},
            )?;
            let identifier = reserved.activate()?;
            Ok(owner.write(Self {
                logical: ManuallyDrop::new(logical),
                account,
                identifier: ManuallyDrop::new(identifier),
                state: ManuallyDrop::new(InterruptSpinLock::new(MachineState {
                    current: image,
                    residency: AddressSpaceResidency::new(initial_epoch),
                })),
                _owner_charge: owner_charge,
            }))
        }
    }

    pub(super) fn logical(&self) -> &LogicalAddressSpace {
        &self.logical
    }

    /// Copies kernel-owned bytes through the logical user-address contract.
    pub(crate) fn copy_to_user(&self, destination: UserSlice, source: &[u8]) -> Result<(), Error> {
        self.logical.copy_to_user(destination, source)?;
        Ok(())
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
        let kind = match &*self.identifier {
            MachineIdentifier::Vhe(_) => crate::hal::user::TranslationKind::VheHostStage1,
            MachineIdentifier::Nvhe(_) => crate::hal::user::TranslationKind::NvheStage2Only,
        };
        let image = build_image(
            kind,
            self.identifier.value(),
            self.identifier.generation(),
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
    pub(crate) fn retire(&mut self) -> Result<(), Error> {
        let mut transport =
            match crate::kernel::irq::cross_call::UserAddressSpaceTransaction::try_acquire() {
                Ok(transport) => transport,
                Err(()) => return Err(Error::Transport),
            };
        let cut_and_image = self.state.with(|state| {
            let cut = state.residency.begin_retirement(state.current.epoch)?;
            Ok::<_, Error>((cut, state.current.clone()))
        });
        let (cut, image) = cut_and_image?;

        // SAFETY: `self` is consumed, admission is permanently closed, and no
        // ActiveNativeAddressSpace borrow can coexist with this move.
        let identifier = unsafe { ManuallyDrop::take(&mut self.identifier) };
        let retiring = match identifier.begin_retirement() {
            Ok(retiring) => retiring,
            Err(_) => crate::kernel::crash::fatal(format_args!(
                "HypeR: native translation identifier retirement is inconsistent"
            )),
        };
        let outcome = transport.execute(
            crate::kernel::irq::cross_call::UserAddressSpaceExecution {
                owner: self.logical.id().get(),
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
        // SAFETY: Admission remains permanently closed and every resident CPU
        // acknowledged invalidating this exact identifier before reuse.
        if unsafe { retiring.complete() }.is_err() {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: native translation identifier completion is inconsistent"
            ));
        }

        // SAFETY: The final invalidation acknowledged before these published
        // owners are extracted and destroyed. ManuallyDrop prevents `Drop`
        // from observing or releasing the moved fields a second time.
        let state = unsafe { ManuallyDrop::take(&mut self.state) };
        // SAFETY: The logical owner is protected by the same final quiescence.
        let logical = unsafe { ManuallyDrop::take(&mut self.logical) };
        drop(image);
        drop(state);
        drop(logical);
        Ok(())
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
    ) -> StoppedNativeUser<'context, 'owner> {
        let Some(backend) = self.backend.take() else {
            crate::hal::cpu::halt();
        };
        match crate::hal::user::run_user(context, backend, binding, kernel_access) {
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

fn build_image(
    kind: crate::hal::user::TranslationKind,
    identifier: u16,
    generation: u64,
    epoch: u64,
    leaf_count: usize,
    account: DomainAccount,
    enumerate: impl FnMut(&mut dyn FnMut(crate::hal::user::MappingPage)),
) -> Result<FallibleArc<MachineImage>, Error> {
    let levels_per_leaf = match kind {
        crate::hal::user::TranslationKind::VheHostStage1 => 3,
        crate::hal::user::TranslationKind::NvheStage2Only => 2,
    };
    let capacity = leaf_count
        .checked_mul(levels_per_leaf)
        .and_then(|pages| pages.checked_add(1))
        .ok_or(Error::SizeOverflow)?;
    let mut tables = TablePagePool::try_new(capacity, account.clone())?;
    let root = {
        let mut allocator = |order| tables.allocate(order);
        // SAFETY: TablePagePool returns uniquely owned zeroed PageTable blocks
        // and is moved intact into the resulting image through acknowledged
        // retirement.
        unsafe {
            match kind {
                crate::hal::user::TranslationKind::VheHostStage1 => {
                    crate::hal::user::prepare_vhe_address_space(
                        identifier,
                        generation,
                        enumerate,
                        &mut allocator,
                    )
                }
                crate::hal::user::TranslationKind::NvheStage2Only => {
                    crate::hal::user::prepare_nvhe_address_space(
                        identifier,
                        generation,
                        enumerate,
                        &mut allocator,
                    )
                }
            }
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
    let mut native = NativeAddressSpace::try_new(domain, range)?;
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
    native.retire()
}
