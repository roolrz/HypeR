// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Guest-physical memory ownership and stage-2 demand paging.

use core::marker::PhantomData;
use core::ptr::{copy_nonoverlapping, write_bytes};

use alloc::vec::Vec;
use hyper::cpu::PerCpu;
use hyper::mm::RetirementCut;
use hyper::mm::allocator::heap::PageOwner;
use hyper::mm::{
    AddressSpaceResidency, BuddyError, ForeignCopyError, ForeignMemory, PAGE_SIZE, PhysicalAddress,
    ResidencyError, copy_from_foreign, copy_to_foreign,
};
use hyper::sync::atomic::{AtomicU64, Ordering};
use hyper::vm::exit::{GuestMemoryFault, MemoryAccess};
use hyper::vm::translation::ActiveMappingError;

use super::registry::VmId;
use super::residency_state::{LocalStage2Observation, Stage2AllocationIdentity, Stage2Incarnation};
use crate::hal::vm::{Stage2AddressSpace, Stage2Error};
use crate::kernel::mm::page_block::PageBlock;

static ACTIVE_STAGE2_ROOT: PerCpu<AtomicU64> =
    PerCpu::new([const { AtomicU64::new(0) }; hyper::cpu::MAX_CPUS]);
static ACTIVE_STAGE2_EPOCH: PerCpu<AtomicU64> =
    PerCpu::new([const { AtomicU64::new(0) }; hyper::cpu::MAX_CPUS]);
static ACTIVE_STAGE2_VMID: PerCpu<AtomicU64> =
    PerCpu::new([const { AtomicU64::new(0) }; hyper::cpu::MAX_CPUS]);
static ACTIVE_STAGE2_GENERATION: PerCpu<AtomicU64> =
    PerCpu::new([const { AtomicU64::new(0) }; hyper::cpu::MAX_CPUS]);
static ACTIVE_INSTRUCTION_ROOT: PerCpu<AtomicU64> =
    PerCpu::new([const { AtomicU64::new(0) }; hyper::cpu::MAX_CPUS]);
static ACTIVE_INSTRUCTION_EPOCH: PerCpu<AtomicU64> =
    PerCpu::new([const { AtomicU64::new(0) }; hyper::cpu::MAX_CPUS]);
static ACTIVE_INSTRUCTION_TRANSLATION_EPOCH: PerCpu<AtomicU64> =
    PerCpu::new([const { AtomicU64::new(0) }; hyper::cpu::MAX_CPUS]);
static ACTIVE_INSTRUCTION_VMID: PerCpu<AtomicU64> =
    PerCpu::new([const { AtomicU64::new(0) }; hyper::cpu::MAX_CPUS]);
static ACTIVE_INSTRUCTION_GENERATION: PerCpu<AtomicU64> =
    PerCpu::new([const { AtomicU64::new(0) }; hyper::cpu::MAX_CPUS]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    AddressOverflow,
    Allocation(BuddyError),
    Cache(hyper::hal::cache::CacheError),
    InvalidRange,
    MetadataAllocation,
    InvalidCpu,
    Poisoned,
    Residency(ResidencyError),
    Registry(super::registry::Error),
    Stage2(Stage2Error),
}

impl From<BuddyError> for Error {
    fn from(error: BuddyError) -> Self {
        Self::Allocation(error)
    }
}

impl From<Stage2Error> for Error {
    fn from(error: Stage2Error) -> Self {
        Self::Stage2(error)
    }
}

impl From<hyper::hal::cache::CacheError> for Error {
    fn from(error: hyper::hal::cache::CacheError) -> Self {
        Self::Cache(error)
    }
}

impl From<super::registry::Error> for Error {
    fn from(error: super::registry::Error) -> Self {
        Self::Registry(error)
    }
}

impl From<ForeignCopyError<Error>> for Error {
    fn from(error: ForeignCopyError<Error>) -> Self {
        match error {
            ForeignCopyError::AddressOverflow => Self::AddressOverflow,
            ForeignCopyError::Backend(error) => error,
            ForeignCopyError::InvalidPageSize | ForeignCopyError::InvalidRange => {
                Self::InvalidRange
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GuestMemoryStats {
    pub addressable_pages: usize,
    pub committed_pages: usize,
    pub boot_committed_pages: usize,
    pub demand_faults: u64,
    pub read_faults: u64,
    pub write_faults: u64,
    pub execute_faults: u64,
    pub page_walk_faults: u64,
    pub repeated_faults: u64,
    pub failed_faults: u64,
}

/// VM-owned sparse guest RAM and its architecture stage-2 hierarchy.
pub(crate) struct GuestAddressSpace {
    ipa_base: u64,
    size: u64,
    pages: Vec<Option<PageBlock>>,
    committed_pages: usize,
    boot_committed_pages: usize,
    demand_faults: u64,
    read_faults: u64,
    write_faults: u64,
    execute_faults: u64,
    page_walk_faults: u64,
    repeated_faults: u64,
    failed_faults: u64,
    poisoned: bool,
    translation_epoch: u64,
    residency: AddressSpaceResidency<{ hyper::cpu::MAX_CPUS }>,
    instruction_epoch: AtomicU64,
    stage2: Stage2AddressSpace,
    table_pages: Stage2PagePool,
    identifier: Stage2Identifier,
}

pub(crate) type Stage2IdentifierReservation =
    crate::kernel::mm::translation_id::IdentifierReservation<
        crate::kernel::mm::translation_id::Stage2Vmid,
    >;
type ActiveStage2Identifier = crate::kernel::mm::translation_id::ActiveIdentifier<
    crate::kernel::mm::translation_id::Stage2Vmid,
>;
type RetiringStage2Identifier = crate::kernel::mm::translation_id::RetiringIdentifier<
    crate::kernel::mm::translation_id::Stage2Vmid,
>;

enum Stage2Identifier {
    Reserved(Option<Stage2IdentifierReservation>),
    Active(ActiveStage2Identifier),
    Retiring(RetiringStage2Identifier),
    Retired,
    Poisoned,
}

impl Drop for GuestAddressSpace {
    fn drop(&mut self) {
        use super::address_space_state::{IdentifierState, destruction_is_safe};

        let state = match &self.identifier {
            Stage2Identifier::Reserved(_) => IdentifierState::Reserved,
            Stage2Identifier::Active(_) => IdentifierState::Active,
            Stage2Identifier::Retiring(_) => IdentifierState::Active,
            Stage2Identifier::Retired => IdentifierState::Retired,
            // Poisoned is installed only while consuming an unpublished VMID
            // reservation. A failed activation never published this address
            // space to hardware, so its pages remain safe to destroy.
            Stage2Identifier::Poisoned => IdentifierState::UnpublishedFailure,
        };
        if !destruction_is_safe(state) {
            // Drop runs before Rust destroys `pages`, `stage2`, and
            // `table_pages`. Fail closed here so active translation storage is
            // never returned while a CPU or stale TLB entry may reference it.
            crate::hal::cpu::halt()
        }
    }
}

impl GuestAddressSpace {
    pub(crate) fn new(
        hardware_vmid: Stage2IdentifierReservation,
        ipa_base: u64,
        size: u64,
    ) -> Result<Self, Error> {
        let page_count = validate_region(ipa_base, size)?;
        let mut pages = Vec::new();
        pages
            .try_reserve_exact(page_count)
            .map_err(|_| Error::MetadataAllocation)?;
        pages.resize_with(page_count, || None);

        let table_capacity = Stage2AddressSpace::required_table_pages(ipa_base, size)?;
        let mut table_pages = Stage2PagePool::with_capacity(table_capacity)?;
        let mut allocate_table = |pages, alignment| table_pages.allocate_zeroed(pages, alignment);
        // SAFETY: Stage2PagePool returns uniquely owned, zeroed, naturally
        // aligned PageBlocks from writable RAM in the permanent linear map.
        // `table_pages` retains every block for at least as long as `stage2`,
        // and this not-yet-published address space has exclusive mutation.
        let identifier = hardware_vmid.value();
        // SAFETY: The consumed reservation is the unique construction
        // authority for this VMID, while `tables` retains every allocated
        // hierarchy page through GuestAddressSpace retirement.
        let stage2 = unsafe { Stage2AddressSpace::new(identifier, &mut allocate_table)? };
        Ok(Self {
            ipa_base,
            size,
            pages,
            committed_pages: 0,
            boot_committed_pages: 0,
            demand_faults: 0,
            read_faults: 0,
            write_faults: 0,
            execute_faults: 0,
            page_walk_faults: 0,
            repeated_faults: 0,
            failed_faults: 0,
            poisoned: false,
            // Epoch zero is reserved for per-CPU residency slots which have
            // never activated or synchronized a guest address space.
            translation_epoch: 1,
            residency: AddressSpaceResidency::try_new(1).map_err(Error::Residency)?,
            instruction_epoch: AtomicU64::new(1),
            stage2,
            table_pages,
            identifier: Stage2Identifier::Reserved(Some(hardware_vmid)),
        })
    }

    pub(super) fn activate_identifier_for_install(&mut self) -> Result<(), super::registry::Error> {
        use super::address_space_state::{IdentifierState, activation_may_begin};

        let state = match &self.identifier {
            Stage2Identifier::Reserved(Some(_)) => IdentifierState::Reserved,
            Stage2Identifier::Active(_) => IdentifierState::Active,
            Stage2Identifier::Retiring(_) => IdentifierState::Active,
            Stage2Identifier::Retired => IdentifierState::Retired,
            Stage2Identifier::Reserved(None) | Stage2Identifier::Poisoned => {
                IdentifierState::UnpublishedFailure
            }
        };
        if !activation_may_begin(state) {
            // Reject without replacing Active: a second safe activation call
            // must never turn live hardware ownership into a drop-safe state.
            return Err(super::registry::Error::InvalidReservation);
        }
        let previous = core::mem::replace(&mut self.identifier, Stage2Identifier::Poisoned);
        let Stage2Identifier::Reserved(Some(reservation)) = previous else {
            // The exclusive preflight above makes this branch impossible.
            crate::hal::cpu::halt()
        };
        let active = reservation
            .activate()
            .map_err(|_| super::registry::Error::IdentityExhausted)?;
        self.identifier = Stage2Identifier::Active(active);
        Ok(())
    }

    fn active_identifier(&self) -> Result<&ActiveStage2Identifier, Error> {
        match &self.identifier {
            Stage2Identifier::Active(identifier) => Ok(identifier),
            Stage2Identifier::Retiring(_) | Stage2Identifier::Retired => Err(Error::Poisoned),
            Stage2Identifier::Reserved(_) | Stage2Identifier::Poisoned => Err(Error::Poisoned),
        }
    }

    fn incarnation(&self) -> Result<Stage2Incarnation, Error> {
        let identifier = self.active_identifier()?;
        Ok(Stage2Incarnation::new(
            self.stage2.root_address(),
            identifier.value(),
            identifier.generation(),
            self.translation_epoch,
        ))
    }

    pub(in crate::kernel) fn begin_retirement(
        &mut self,
        capability: &crate::hal::vm::GuestStage2RetirementCapability,
        topology_count: usize,
    ) -> Result<GuestStage2Retirement, Error> {
        self.ensure_healthy()?;
        if topology_count == 0 || topology_count > hyper::cpu::MAX_CPUS {
            return Err(Error::InvalidCpu);
        }
        let incarnation = self.incarnation()?;
        // The registry acquired this selected-mechanism proof before its own
        // irreversible cut. Request preparation is therefore infallible and
        // remains ahead of residency and identifier retirement.
        let request = crate::hal::vm::prepare_guest_stage2_retirement(capability, &self.stage2);
        let cut = self
            .residency
            .begin_retirement(incarnation.translation_epoch())
            .map_err(Error::Residency)?;
        if cut
            .targets()
            .iter()
            .copied()
            .enumerate()
            .any(|(cpu, targeted)| targeted && cpu >= topology_count)
        {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: guest retirement targets escape the frozen CPU topology"
            ));
        }

        let previous = core::mem::replace(&mut self.identifier, Stage2Identifier::Poisoned);
        let Stage2Identifier::Active(identifier) = previous else {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: guest retirement lost its active VMID"
            ));
        };
        let retiring = match identifier.begin_retirement() {
            Ok(retiring) => retiring,
            Err(error) => crate::kernel::crash::fatal(format_args!(
                "HypeR: guest VMID retirement could not begin after the residency cut: {error:?}"
            )),
        };
        self.identifier = Stage2Identifier::Retiring(retiring);
        Ok(GuestStage2Retirement {
            cut,
            allocation: incarnation.allocation(),
            request,
        })
    }

    pub(in crate::kernel) fn finish_retirement(&mut self, retirement: GuestStage2Retirement) {
        let GuestStage2Retirement {
            cut,
            allocation,
            request: _,
        } = retirement;
        let identity_matches = match &self.identifier {
            Stage2Identifier::Retiring(identifier) => {
                self.stage2.root_address() == allocation.root()
                    && u64::from(identifier.value()) == allocation.vmid()
                    && identifier.generation() == allocation.generation()
            }
            _ => false,
        };
        if !identity_matches {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: guest retirement completion changed translation identity"
            ));
        }
        if self.residency.finish_retirement(cut).is_err() {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: guest residency retirement completion is inconsistent"
            ));
        }
        let previous = core::mem::replace(&mut self.identifier, Stage2Identifier::Poisoned);
        let Stage2Identifier::Retiring(identifier) = previous else {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: guest retirement lost its retiring VMID"
            ));
        };
        // SAFETY: The residency state is irreversibly retired and the caller
        // obtained exact-generation acknowledgement from every sticky target.
        if unsafe { identifier.complete() }.is_err() {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: guest VMID completion is inconsistent"
            ));
        }
        self.identifier = Stage2Identifier::Retired;
    }

    pub fn copy_from(&mut self, ipa: u64, destination: &mut [u8]) -> Result<(), Error> {
        self.ensure_healthy()?;
        copy_from_foreign(self, ipa, destination).map_err(Error::from)
    }

    pub fn copy_to(&mut self, ipa: u64, source: &[u8]) -> Result<(), Error> {
        self.ensure_healthy()?;
        copy_to_foreign(self, ipa, source).map_err(Error::from)
    }

    pub fn finish_boot_loading(&mut self) {
        self.boot_committed_pages = self.committed_pages;
    }

    pub fn publish_instruction(&self, ipa: u64, length: usize) -> Result<(), Error> {
        self.publish_range(ipa, length, true)?;
        if length == 0 {
            return Ok(());
        }
        // Cache maintenance and instruction-byte stores happen-before a CPU
        // which observes this epoch during a later activation.
        if self
            .instruction_epoch
            .fetch_update(Ordering::Release, Ordering::Relaxed, |epoch| {
                epoch.checked_add(1)
            })
            .is_err()
        {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: guest instruction publication epoch exhausted"
            ));
        }
        Ok(())
    }

    pub fn publish_data(&self, ipa: u64, length: usize) -> Result<(), Error> {
        self.publish_range(ipa, length, false)
    }

    pub const fn root_address(&self) -> u64 {
        self.stage2.root_address()
    }

    pub fn statistics(&self) -> GuestMemoryStats {
        GuestMemoryStats {
            addressable_pages: self.pages.len(),
            committed_pages: self.committed_pages,
            boot_committed_pages: self.boot_committed_pages,
            demand_faults: self.demand_faults,
            read_faults: self.read_faults,
            write_faults: self.write_faults,
            execute_faults: self.execute_faults,
            page_walk_faults: self.page_walk_faults,
            repeated_faults: self.repeated_faults,
            failed_faults: self.failed_faults,
        }
    }

    fn resolve_guest_memory_fault(&mut self, fault: GuestMemoryFault) -> Result<bool, Error> {
        self.ensure_healthy()?;
        let Some(page_index) = self.page_index(fault.address().get()) else {
            return Ok(false);
        };
        self.demand_faults = self.demand_faults.saturating_add(1);
        match fault.access() {
            MemoryAccess::Read => self.read_faults = self.read_faults.saturating_add(1),
            MemoryAccess::Write => self.write_faults = self.write_faults.saturating_add(1),
            MemoryAccess::Execute => self.execute_faults = self.execute_faults.saturating_add(1),
        }
        if fault.during_guest_page_walk() {
            self.page_walk_faults = self.page_walk_faults.saturating_add(1);
        }
        if self.pages[page_index].is_some() {
            self.repeated_faults = self.repeated_faults.saturating_add(1);
            let ipa = self.page_ipa(page_index)?;
            // SAFETY: Fault dispatch proves this VM's stage-2 is active on the
            // current CPU, and the address-space lock serializes invalidation.
            unsafe { self.stage2.invalidate_page_active(ipa)? };
            return Ok(true);
        }
        self.commit_page(page_index, true).inspect_err(|_| {
            self.failed_faults = self.failed_faults.saturating_add(1);
        })?;
        Ok(true)
    }

    fn commit_page(&mut self, page_index: usize, active: bool) -> Result<(), Error> {
        self.ensure_healthy()?;
        if self.pages.get(page_index).is_none() {
            return Err(Error::InvalidRange);
        }
        if self.pages[page_index].is_some() {
            return Ok(());
        }
        let active_cpu = if active {
            let cpu = crate::kernel::cpu::current_index().ok_or(Error::InvalidCpu)?;
            self.residency
                .check_single_active(cpu.get(), self.translation_epoch)
                .map_err(Error::Residency)?;
            Some(cpu)
        } else {
            self.residency
                .check_inactive(self.translation_epoch)
                .map_err(Error::Residency)?;
            None
        };
        let page = PageBlock::allocate_for(0, PageOwner::Guest)?;
        let physical = page.physical();
        let virtual_address = linear_address(physical)?;
        // SAFETY: The new page is exclusively VM-owned and fully covered by
        // the permanent writable linear map.
        unsafe { write_bytes(virtual_address as *mut u8, 0, PAGE_SIZE as usize) };
        let ipa = self.page_ipa(page_index)?;
        let mut allocate_table =
            |pages, alignment| self.table_pages.allocate_zeroed(pages, alignment);
        let committed_error = if active {
            // SAFETY: Called only from a lower-EL translation fault while this
            // VM is active, under the address-space lock.
            match unsafe {
                self.stage2
                    .map_normal_page_active(ipa, physical.get(), &mut allocate_table)
            } {
                Ok(()) => None,
                Err(ActiveMappingError::BeforeInstall(error)) => return Err(error.into()),
                Err(ActiveMappingError::InstalledButInvalidationFailed(error)) => Some(error),
            }
        } else {
            // SAFETY: Stage2PagePool preserves the allocation contract stated
            // at construction, and &mut self plus the owning VM's
            // address-space lock serializes all hierarchy mutation.
            unsafe {
                self.stage2
                    .map_normal_page(ipa, physical.get(), &mut allocate_table)?
            };
            None
        };
        // Publication commits physical ownership even when the subsequent
        // invalidation fails. Store the owner before reporting that failure so
        // the live descriptor can never point at a page returned to the buddy.
        self.pages[page_index] = Some(page);
        self.committed_pages += 1;
        let previous_epoch = self.translation_epoch;
        self.translation_epoch = match previous_epoch.checked_add(1) {
            Some(epoch) => epoch,
            None => {
                self.poisoned = true;
                crate::kernel::crash::fatal(format_args!("HypeR: stage-2 mapping epoch exhausted"));
            }
        };
        if let Some(error) = committed_error {
            self.poisoned = true;
            // The single-active execution lease excludes a concurrent vCPU,
            // but a failed local invalidation still leaves architectural state
            // ambiguous. Ownership is retained above before global fail-stop.
            crate::kernel::crash::fatal(format_args!(
                "HypeR: committed stage-2 mapping invalidation failed: {error:?}"
            ));
        }
        if active {
            let Some(cpu) = active_cpu else {
                crate::hal::cpu::halt()
            };
            if self
                .residency
                .advance_single_active(cpu.get(), previous_epoch, self.translation_epoch)
                .is_err()
            {
                crate::kernel::crash::fatal(format_args!(
                    "HypeR: active guest residency epoch publication is inconsistent"
                ));
            }
            let incarnation = match self.incarnation() {
                Ok(incarnation) => incarnation,
                Err(error) => crate::kernel::crash::fatal(format_args!(
                    "HypeR: active guest mapping lost its VMID incarnation: {error:?}"
                )),
            };
            publish_current_residency(incarnation);
        } else if self
            .residency
            .advance_inactive(previous_epoch, self.translation_epoch)
            .is_err()
        {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: inactive guest residency epoch publication is inconsistent"
            ));
        }
        Ok(())
    }

    fn validate_access(&self, ipa: u64, length: usize) -> Result<usize, Error> {
        let offset = ipa.checked_sub(self.ipa_base).ok_or(Error::InvalidRange)?;
        let length = u64::try_from(length).map_err(|_| Error::AddressOverflow)?;
        let end = offset.checked_add(length).ok_or(Error::AddressOverflow)?;
        if end > self.size {
            return Err(Error::InvalidRange);
        }
        usize::try_from(offset).map_err(|_| Error::AddressOverflow)
    }

    fn publish_range(&self, ipa: u64, length: usize, instruction: bool) -> Result<(), Error> {
        self.ensure_healthy()?;
        self.validate_access(ipa, length)?;
        if length == 0 {
            return Ok(());
        }
        if !instruction {
            return self.visit_range_chunks(ipa, length, |address, chunk| {
                // SAFETY: Loading owns the VM and no vCPU can observe these
                // pages until the stage-2 hierarchy is installed and active.
                unsafe { crate::hal::cache::publish_data_range(address, chunk) }
                    .map_err(Error::from)
            });
        }

        let mut walk_error = None;
        // SAFETY: Loading owns every committed page and excludes guest
        // execution and modification for the complete, potentially two-pass
        // transaction. The immutable page set yields identical ranges on each
        // architecture-requested enumeration.
        let cache_result = unsafe {
            crate::hal::cache::publish_instruction_ranges(|visit| {
                if walk_error.is_some() {
                    return;
                }
                if let Err(error) = self.visit_range_chunks(ipa, length, |address, chunk| {
                    visit(address, chunk);
                    Ok(())
                }) {
                    walk_error = Some(error);
                }
            })
        };
        if let Some(error) = walk_error {
            return Err(error);
        }
        cache_result.map_err(Error::from)
    }

    fn visit_range_chunks(
        &self,
        ipa: u64,
        length: usize,
        mut visit: impl FnMut(usize, usize) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let mut offset = self.validate_access(ipa, length)?;
        let mut published = 0;
        while published < length {
            let page_index = offset / PAGE_SIZE as usize;
            let page_offset = offset % PAGE_SIZE as usize;
            let page = self.pages[page_index].as_ref().ok_or(Error::InvalidRange)?;
            let address = linear_address(page.physical())?
                .checked_add(page_offset)
                .ok_or(Error::AddressOverflow)?;
            let chunk = (PAGE_SIZE as usize - page_offset).min(length - published);
            visit(address, chunk)?;
            published += chunk;
            offset += chunk;
        }
        Ok(())
    }

    fn page_index(&self, address: u64) -> Option<usize> {
        let offset = address.checked_sub(self.ipa_base)?;
        if offset >= self.size {
            return None;
        }
        usize::try_from(offset / PAGE_SIZE).ok()
    }

    fn page_ipa(&self, page_index: usize) -> Result<u64, Error> {
        let offset = (page_index as u64)
            .checked_mul(PAGE_SIZE)
            .ok_or(Error::AddressOverflow)?;
        self.ipa_base
            .checked_add(offset)
            .ok_or(Error::AddressOverflow)
    }

    fn ensure_healthy(&self) -> Result<(), Error> {
        if self.poisoned {
            Err(Error::Poisoned)
        } else {
            Ok(())
        }
    }
}

#[must_use = "guest retirement must obtain every target acknowledgement"]
pub(in crate::kernel) struct GuestStage2Retirement {
    cut: RetirementCut<{ hyper::cpu::MAX_CPUS }>,
    allocation: Stage2AllocationIdentity,
    request: crate::hal::vm::GuestStage2RetirementRequest,
}

impl GuestStage2Retirement {
    pub(in crate::kernel) fn targets(&self) -> &[bool; hyper::cpu::MAX_CPUS] {
        self.cut.targets()
    }

    pub(in crate::kernel) const fn local_request(&self) -> GuestStage2LocalRequest {
        GuestStage2LocalRequest {
            allocation: self.allocation,
            hardware: self.request,
        }
    }
}

#[derive(Clone, Copy)]
pub(in crate::kernel) struct GuestStage2LocalRequest {
    allocation: Stage2AllocationIdentity,
    hardware: crate::hal::vm::GuestStage2RetirementRequest,
}

pub(in crate::kernel) fn service_local_retirement(request: GuestStage2LocalRequest) {
    crate::hal::vm::service_guest_stage2_retirement(request.hardware);
    if clear_local_observations(request.allocation).is_err() {
        crate::kernel::crash::fatal(format_args!(
            "HypeR: guest retirement could not clear local observations"
        ));
    }
}

impl ForeignMemory for GuestAddressSpace {
    type Error = Error;

    fn address_base(&self) -> u64 {
        self.ipa_base
    }

    fn address_size(&self) -> u64 {
        self.size
    }

    fn page_size(&self) -> usize {
        PAGE_SIZE as usize
    }

    fn read_page(
        &mut self,
        page_index: usize,
        page_offset: usize,
        destination: &mut [u8],
    ) -> Result<(), Self::Error> {
        let Some(page) = self.pages.get(page_index).ok_or(Error::InvalidRange)? else {
            destination.fill(0);
            return Ok(());
        };
        let source = linear_address(page.physical())?
            .checked_add(page_offset)
            .ok_or(Error::AddressOverflow)?;
        // SAFETY: The generic copy layer bounds this chunk to one VM-owned
        // page, whose mapping remains stable under the address-space lock.
        unsafe {
            copy_nonoverlapping(
                source as *const u8,
                destination.as_mut_ptr(),
                destination.len(),
            )
        };
        Ok(())
    }

    fn write_page(
        &mut self,
        page_index: usize,
        page_offset: usize,
        source: &[u8],
    ) -> Result<(), Self::Error> {
        self.commit_page(page_index, false)?;
        let page = self.pages[page_index].as_ref().ok_or(Error::InvalidRange)?;
        let destination = linear_address(page.physical())?
            .checked_add(page_offset)
            .ok_or(Error::AddressOverflow)?;
        // SAFETY: The generic copy layer bounds this chunk to one VM-owned
        // page, whose mapping remains stable under the address-space lock.
        unsafe { copy_nonoverlapping(source.as_ptr(), destination as *mut u8, source.len()) };
        Ok(())
    }
}

impl super::linux::abi::PayloadMemory for GuestAddressSpace {
    type Error = Error;

    fn copy_to(
        &mut self,
        address: hyper::vm::exit::GuestPhysicalAddress,
        bytes: &[u8],
    ) -> Result<(), Self::Error> {
        GuestAddressSpace::copy_to(self, address.get(), bytes)
    }

    fn publish_instruction(
        &self,
        address: hyper::vm::exit::GuestPhysicalAddress,
        length: usize,
    ) -> Result<(), Self::Error> {
        GuestAddressSpace::publish_instruction(self, address.get(), length)
    }

    fn publish_data(
        &self,
        address: hyper::vm::exit::GuestPhysicalAddress,
        length: usize,
    ) -> Result<(), Self::Error> {
        GuestAddressSpace::publish_data(self, address.get(), length)
    }
}

/// Linear residency retained from guest stage-2 activation until hardware
/// detach completes on the same CPU.
#[must_use = "an active guest residency must leave before VM execution release"]
pub(in crate::kernel) struct GuestResidencyClaim {
    cpu: hyper::cpu::CpuIndex,
    admitted: Stage2Incarnation,
    armed: bool,
    cpu_affine: PhantomData<*mut ()>,
}

impl GuestResidencyClaim {
    fn new(cpu: hyper::cpu::CpuIndex, admitted: Stage2Incarnation) -> Self {
        Self {
            cpu,
            admitted,
            armed: true,
            cpu_affine: PhantomData,
        }
    }
}

impl Drop for GuestResidencyClaim {
    fn drop(&mut self) {
        if self.armed {
            // No safe destructor can prove architecture hardware is detached
            // or repair residency history after abandoning this capability.
            crate::hal::cpu::halt()
        }
    }
}

#[must_use = "failed leave retains the exact armed residency claim"]
pub(in crate::kernel) struct GuestResidencyLeaveFailure {
    error: Error,
    claim: GuestResidencyClaim,
}

impl GuestResidencyLeaveFailure {
    pub(in crate::kernel) const fn error(&self) -> Error {
        self.error
    }

    pub(in crate::kernel) fn into_claim(self) -> GuestResidencyClaim {
        self.claim
    }
}

/// Activates the installed VM's stage-2 hierarchy on the current CPU.
///
/// # Safety
///
/// The caller must own the stopped vCPU carrying `vm`, retain this VM's
/// exclusive execution claim, and keep local interrupts masked.
pub(in crate::kernel) unsafe fn activate(
    vm: &super::registry::VmBinding,
) -> Result<GuestResidencyClaim, Error> {
    vm.with_address_space(|address_space| {
        address_space.ensure_healthy()?;
        let incarnation = address_space.incarnation()?;
        if incarnation.allocation().vmid() == 0 {
            return Err(Error::Poisoned);
        }
        let cpu = crate::kernel::cpu::current_index().ok_or(Error::InvalidCpu)?;
        address_space
            .residency
            .check_admission(cpu.get(), incarnation.translation_epoch())
            .map_err(Error::Residency)?;
        if !load_stage2_observation(cpu).matches(incarnation, incarnation.translation_epoch()) {
            // SAFETY: The caller owns the stopped vCPU, and the installed
            // address space is pinned in the VM registry for the active guest
            // lifetime. Architecture activation includes any local
            // invalidation required before this CPU may consume the current
            // mapping epoch.
            unsafe { address_space.stage2.activate() };
            store_stage2_observation(
                cpu,
                LocalStage2Observation::new(incarnation, incarnation.translation_epoch()),
            );
        }

        let instruction_epoch = address_space.instruction_epoch.load(Ordering::Acquire);
        if !load_instruction_observation(cpu).matches(incarnation, instruction_epoch) {
            // FENCE.I is hart-local on RISC-V; AArch64 and x86 likewise require
            // a local instruction synchronization event before entering a
            // newly published instruction stream on this CPU.
            crate::hal::cache::synchronize_instruction_execution();
            store_instruction_observation(
                cpu,
                LocalStage2Observation::new(incarnation, instruction_epoch),
            );
        }
        if address_space
            .residency
            .publish_admission(cpu.get())
            .is_err()
        {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: guest residency publication failed after stage-2 activation"
            ));
        }
        Ok(GuestResidencyClaim::new(cpu, incarnation))
    })
}

/// Leaves the exact admitted CPU after architecture hardware and host timer
/// ownership have been restored but before VM execution admission is released.
pub(in crate::kernel) fn leave(
    vm: &super::registry::VmBinding,
    mut claim: GuestResidencyClaim,
) -> Result<(), GuestResidencyLeaveFailure> {
    let current = match crate::kernel::cpu::current_index() {
        Some(cpu) if cpu == claim.cpu => cpu,
        _ => {
            return Err(GuestResidencyLeaveFailure {
                error: Error::InvalidCpu,
                claim,
            });
        }
    };
    let result = vm.with_address_space(|address_space| {
        let incarnation = address_space.incarnation()?;
        if !claim.admitted.same_allocation(incarnation) {
            return Err(Error::Poisoned);
        }
        address_space
            .residency
            .leave(current.get(), incarnation.translation_epoch())
            .map_err(Error::Residency)
    });
    match result {
        Ok(()) => {
            claim.armed = false;
            Ok(())
        }
        Err(error) => Err(GuestResidencyLeaveFailure { error, claim }),
    }
}

fn publish_current_residency(incarnation: Stage2Incarnation) {
    let Some(cpu) = crate::kernel::cpu::current_index() else {
        crate::kernel::crash::fatal(format_args!(
            "HypeR: active stage-2 mapping has no registered CPU owner"
        ));
    };
    // Active mapping publication already completed the architecture-local
    // invalidation. Updating this CPU-private observation avoids repeating a
    // whole-context activation at the following IRQ-tail resume.
    store_stage2_observation(
        cpu,
        LocalStage2Observation::new(incarnation, incarnation.translation_epoch()),
    );
}

fn load_stage2_observation(cpu: hyper::cpu::CpuIndex) -> LocalStage2Observation {
    observation_from_atomics(
        ACTIVE_STAGE2_ROOT[cpu].load(Ordering::Relaxed),
        ACTIVE_STAGE2_VMID[cpu].load(Ordering::Relaxed),
        ACTIVE_STAGE2_GENERATION[cpu].load(Ordering::Relaxed),
        ACTIVE_STAGE2_EPOCH[cpu].load(Ordering::Relaxed),
        ACTIVE_STAGE2_EPOCH[cpu].load(Ordering::Relaxed),
    )
}

fn store_stage2_observation(cpu: hyper::cpu::CpuIndex, observation: LocalStage2Observation) {
    let allocation = observation.allocation();
    ACTIVE_STAGE2_ROOT[cpu].store(allocation.root(), Ordering::Relaxed);
    ACTIVE_STAGE2_VMID[cpu].store(allocation.vmid(), Ordering::Relaxed);
    ACTIVE_STAGE2_GENERATION[cpu].store(allocation.generation(), Ordering::Relaxed);
    ACTIVE_STAGE2_EPOCH[cpu].store(observation.translation_epoch(), Ordering::Relaxed);
}

fn load_instruction_observation(cpu: hyper::cpu::CpuIndex) -> LocalStage2Observation {
    observation_from_atomics(
        ACTIVE_INSTRUCTION_ROOT[cpu].load(Ordering::Relaxed),
        ACTIVE_INSTRUCTION_VMID[cpu].load(Ordering::Relaxed),
        ACTIVE_INSTRUCTION_GENERATION[cpu].load(Ordering::Relaxed),
        ACTIVE_INSTRUCTION_TRANSLATION_EPOCH[cpu].load(Ordering::Relaxed),
        ACTIVE_INSTRUCTION_EPOCH[cpu].load(Ordering::Relaxed),
    )
}

fn store_instruction_observation(cpu: hyper::cpu::CpuIndex, observation: LocalStage2Observation) {
    let allocation = observation.allocation();
    ACTIVE_INSTRUCTION_ROOT[cpu].store(allocation.root(), Ordering::Relaxed);
    ACTIVE_INSTRUCTION_VMID[cpu].store(allocation.vmid(), Ordering::Relaxed);
    ACTIVE_INSTRUCTION_GENERATION[cpu].store(allocation.generation(), Ordering::Relaxed);
    ACTIVE_INSTRUCTION_TRANSLATION_EPOCH[cpu]
        .store(observation.translation_epoch(), Ordering::Relaxed);
    ACTIVE_INSTRUCTION_EPOCH[cpu].store(observation.synchronization_epoch(), Ordering::Relaxed);
}

fn observation_from_atomics(
    root: u64,
    vmid: u64,
    generation: u64,
    translation_epoch: u64,
    synchronization_epoch: u64,
) -> LocalStage2Observation {
    LocalStage2Observation::new(
        Stage2Incarnation::new(root, vmid as u16, generation, translation_epoch),
        synchronization_epoch,
    )
}

/// Clears only per-CPU observations for the exact retained VMID allocation.
///
/// Stage-C retirement will invoke this locally after its tagged invalidation.
#[allow(dead_code)]
pub(super) fn clear_local_observations(allocation: Stage2AllocationIdentity) -> Result<(), Error> {
    let cpu = crate::kernel::cpu::current_index().ok_or(Error::InvalidCpu)?;
    let mut stage2 = load_stage2_observation(cpu);
    if stage2.clear_allocation(allocation) {
        store_stage2_observation(cpu, stage2);
    }
    let mut instruction = load_instruction_observation(cpu);
    if instruction.clear_allocation(allocation) {
        store_instruction_observation(cpu, instruction);
    }
    Ok(())
}

pub(in crate::kernel) fn resolve_guest_memory_fault(
    vm: &super::registry::VmBinding,
    fault: GuestMemoryFault,
) -> Result<bool, Error> {
    vm.with_address_space(|address_space| address_space.resolve_guest_memory_fault(fault))
}

/// Copies guest bytes through VM ownership metadata, never by treating an IPA
/// as a host pointer. Uncommitted demand-zero pages read as zero without being
/// allocated.
pub fn copy_from_guest(
    virtual_machine: VmId,
    source_ipa: u64,
    destination: &mut [u8],
) -> Result<(), Error> {
    super::registry::with_address_space(virtual_machine, |address_space| {
        address_space.copy_from(source_ipa, destination)
    })?
}

/// Copies kernel bytes into guest RAM, committing demand-zero pages as needed.
/// Callers needing a coherent snapshot must stop or otherwise serialize the
/// target vCPU while the copy is in progress.
pub fn copy_to_guest(
    virtual_machine: VmId,
    destination_ipa: u64,
    source: &[u8],
) -> Result<(), Error> {
    super::registry::with_address_space(virtual_machine, |address_space| {
        address_space.copy_to(destination_ipa, source)
    })?
}

pub fn statistics(virtual_machine: VmId) -> Option<GuestMemoryStats> {
    super::registry::with_address_space(virtual_machine, |address_space| address_space.statistics())
        .ok()
}

struct Stage2PagePool {
    pages: Vec<PageBlock>,
}

impl Stage2PagePool {
    fn with_capacity(capacity: usize) -> Result<Self, Error> {
        let mut pages = Vec::new();
        pages
            .try_reserve_exact(capacity)
            .map_err(|_| Error::MetadataAllocation)?;
        Ok(Self { pages })
    }

    fn allocate_zeroed(&mut self, pages: usize, alignment_pages: usize) -> Option<PhysicalAddress> {
        if pages == 0
            || !pages.is_power_of_two()
            || alignment_pages != pages
            || self.pages.len() == self.pages.capacity()
        {
            return None;
        }
        let byte_count = pages.checked_mul(PAGE_SIZE as usize)?;
        let order = pages.trailing_zeros() as usize;
        let page = PageBlock::allocate_for(order, PageOwner::PageTable).ok()?;
        let physical = page.physical();
        let virtual_address = linear_address(physical).ok()?;
        // SAFETY: This new table page is exclusive and permanently mapped.
        unsafe { write_bytes(virtual_address as *mut u8, 0, byte_count) };
        self.pages.push(page);
        Some(physical)
    }
}

fn validate_region(ipa_base: u64, size: u64) -> Result<usize, Error> {
    if size == 0 || ipa_base & (PAGE_SIZE - 1) != 0 || size & (PAGE_SIZE - 1) != 0 {
        return Err(Error::InvalidRange);
    }
    ipa_base.checked_add(size).ok_or(Error::AddressOverflow)?;
    usize::try_from(size / PAGE_SIZE).map_err(|_| Error::AddressOverflow)
}

fn linear_address(physical: PhysicalAddress) -> Result<usize, Error> {
    crate::kernel::mm::memory::linear_address(physical.get()).ok_or(Error::InvalidRange)
}
