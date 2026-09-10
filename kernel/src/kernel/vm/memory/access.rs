// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Guest page commitment, copying, fault resolution, and publication.

#[cfg(feature = "kernel-self-test")]
use core::ptr::copy_nonoverlapping;
#[cfg(feature = "kernel-self-test")]
use core::ptr::write_bytes;

#[cfg(feature = "kernel-self-test")]
use hyper::mm::allocator::heap::PageOwner;
use hyper::mm::{ForeignMemory, PAGE_SIZE, PhysicalAddress};
#[cfg(feature = "kernel-self-test")]
use hyper::mm::{copy_from_foreign, copy_to_foreign};
use hyper::sync::atomic::Ordering;
use hyper::vm::exit::{GuestMemoryFault, MemoryAccess};
use hyper::vm::translation::{
    ActiveMappingError, Stage2FaultResolution, Stage2PagePermissions, resolve_stage2_fault,
};

#[cfg(feature = "kernel-self-test")]
use super::GuestMemoryStats;
use super::residency::publish_current_residency;
use super::storage::{
    FixedBitmap, GuestMemoryBacking, ResidentInstructionSnapshot, accumulate_charge,
    bitmap_storage_bytes, linear_address,
};
use super::{Error, GuestAddressSpace};
use crate::kernel::accounting::{ResourceAmount, ResourceKind};
#[cfg(feature = "kernel-self-test")]
use crate::kernel::mm::page_block::PageBlock;

#[derive(Clone, Copy)]
enum LeafPublication<'pin> {
    Inactive,
    Active(&'pin dyn hyper::cpu::PinnedExecution),
}

impl LeafPublication<'_> {
    const fn is_active(self) -> bool {
        matches!(self, Self::Active(_))
    }
}

impl GuestAddressSpace {
    #[cfg(feature = "kernel-self-test")]
    pub fn copy_from(&mut self, ipa: u64, destination: &mut [u8]) -> Result<(), Error> {
        self.ensure_healthy()?;
        copy_from_foreign(self, ipa, destination).map_err(Error::from)
    }

    #[cfg(feature = "kernel-self-test")]
    pub fn copy_to(&mut self, ipa: u64, source: &[u8]) -> Result<(), Error> {
        self.ensure_healthy()?;
        copy_to_foreign(self, ipa, source).map_err(Error::from)
    }

    pub fn finish_boot_loading(&mut self) {
        self.boot_committed_pages = self.committed_pages;
    }

    /// Publishes every resident shared-backing page as potentially executable.
    ///
    /// The userspace VMM defines executable guest ranges. Until range metadata
    /// becomes part of the VM construction ABI, sealing conservatively
    /// publishes all pages populated by the loader. Sparse holes remain
    /// untouched and are still committed on first guest access.
    pub(crate) fn publish_resident_instructions(&mut self) -> Result<(), Error> {
        self.ensure_healthy()?;
        // Snapshot before disabling preemption: allocation and address
        // resolution are fallible and may be proportional to guest RAM, while
        // the snapshot freezes the exact ranges required by a possibly
        // multi-pass HAL operation.
        // Pages may become resident afterwards, but cannot disappear or change
        // physical identity while this address space retains its backing.
        let snapshot = self.snapshot_resident_instruction_pages()?;
        let pin =
            crate::kernel::task::scheduler::preempt_disable().map_err(|_| Error::InvalidCpu)?;
        let result = self.publish_instruction_snapshot(&pin, &snapshot);
        drop(pin);
        result
    }

    fn snapshot_resident_instruction_pages(&self) -> Result<ResidentInstructionSnapshot, Error> {
        let metadata_bytes = bitmap_storage_bytes(self.mapped_pages.len())
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(Error::MetadataAllocation)?;
        let metadata_charge = self
            .domain
            .reserve(ResourceAmount::ZERO.with(ResourceKind::KernelMemoryBytes, metadata_bytes))?
            .commit();
        let mut pages = FixedBitmap::try_new(self.mapped_pages.len())?;
        for page_index in 0..self.mapped_pages.len() {
            let resident = self.backing_page_is_resident(page_index)?;
            if resident {
                // Resolve once before pinning so recoverable address failures
                // cannot occur after a cache-maintenance phase has begun.
                let _ = linear_address(self.backing_physical_page(page_index)?)?;
            }
            pages.set(page_index, resident)?;
        }
        Ok(ResidentInstructionSnapshot {
            pages,
            _metadata_charge: metadata_charge,
        })
    }

    fn publish_instruction_snapshot(
        &mut self,
        pin: &dyn hyper::cpu::PinnedExecution,
        snapshot: &ResidentInstructionSnapshot,
    ) -> Result<(), Error> {
        if snapshot.pages.len() != self.instruction_ready_pages.len() {
            return Err(Error::InvalidRange);
        }
        self.residency
            .check_inactive(self.translation_epoch)
            .map_err(Error::Residency)?;
        // SAFETY: The typed pin keeps both maintenance phases on one CPU. A
        // PendingVirtualMachine is not runnable, direct VMO writers are excluded
        // by the hardware lease, and `snapshot` fixes membership. Every
        // selected physical page and its linear alias remain stable through VM
        // retirement. Failure to resolve a prevalidated address would therefore
        // prove memory-ownership corruption after maintenance has begun.
        let result = unsafe {
            crate::hal::cache::publish_instruction_ranges(pin, |visit| {
                for (page_index, resident) in snapshot.pages.iter().enumerate() {
                    if resident {
                        let address = match self
                            .backing_physical_page(page_index)
                            .and_then(linear_address)
                        {
                            Ok(address) => address,
                            Err(error) => crate::kernel::crash::fatal(format_args!(
                                "HypeR: resident guest instruction page changed during publication: {error:?}"
                            )),
                        };
                        visit(address, PAGE_SIZE as usize);
                    }
                }
            })
        };
        result.map_err(Error::from)?;
        let mut translation_changed = false;
        for (page_index, resident) in snapshot.pages.iter().enumerate() {
            if resident
                && self.mapped_pages.get(page_index).unwrap_or(false)
                && !self
                    .instruction_ready_pages
                    .get(page_index)
                    .unwrap_or(false)
            {
                self.stage2
                    .make_normal_page_executable(self.page_ipa(page_index)?)?;
                translation_changed = true;
            }
        }
        if translation_changed {
            self.commit_translation_change(None);
        }
        let mut published = false;
        for (page_index, resident) in snapshot.pages.iter().enumerate() {
            if resident {
                self.instruction_ready_pages.set(page_index, true)?;
                published = true;
            }
        }
        if published {
            self.advance_instruction_epoch();
        }
        Ok(())
    }

    #[cfg(feature = "kernel-self-test")]
    pub(crate) fn instruction_snapshot_for_test(
        &self,
    ) -> Result<ResidentInstructionSnapshot, Error> {
        self.snapshot_resident_instruction_pages()
    }

    #[cfg(feature = "kernel-self-test")]
    pub(crate) fn publish_instruction_snapshot_for_test(
        &mut self,
        pin: &dyn hyper::cpu::PinnedExecution,
        snapshot: &ResidentInstructionSnapshot,
    ) -> Result<(), Error> {
        self.publish_instruction_snapshot(pin, snapshot)
    }

    #[cfg(feature = "kernel-self-test")]
    pub(crate) fn prepare_active_leaf_page_for_test(
        &mut self,
        page_index: usize,
        pin: &dyn hyper::cpu::PinnedExecution,
    ) -> Result<(), Error> {
        let physical = self.prepare_backing_page(page_index)?;
        self.publish_instruction_page(page_index, physical, pin)
    }

    #[cfg(feature = "kernel-self-test")]
    pub(crate) fn instruction_page_ready_for_test(&self, page_index: usize) -> bool {
        self.instruction_ready_pages
            .get(page_index)
            .unwrap_or(false)
    }

    #[cfg(feature = "kernel-self-test")]
    pub fn statistics(&self) -> GuestMemoryStats {
        GuestMemoryStats {
            addressable_pages: self.mapped_pages.len(),
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

    fn resolve_guest_memory_fault(
        &mut self,
        fault: GuestMemoryFault,
        pin: &dyn hyper::cpu::PinnedExecution,
    ) -> Result<bool, Error> {
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
        match resolve_stage2_fault(
            self.mapped_pages.get(page_index).unwrap_or(false),
            self.instruction_ready_pages
                .get(page_index)
                .unwrap_or(false),
            fault.access(),
            fault.during_guest_page_walk(),
        ) {
            Stage2FaultResolution::PromoteExecute => {
                self.promote_active_page_to_executable(page_index, pin)?;
            }
            Stage2FaultResolution::Refresh => {
                self.repeated_faults = self.repeated_faults.saturating_add(1);
                let ipa = self.page_ipa(page_index)?;
                // SAFETY: Fault dispatch proves this VM's stage-2 is active on
                // the current CPU, and the address-space lock serializes
                // invalidation.
                unsafe { self.stage2.invalidate_page_active(ipa)? };
            }
            Stage2FaultResolution::Map(permissions) => {
                self.commit_page(page_index, LeafPublication::Active(pin), permissions)
                    .inspect_err(|_| {
                        self.failed_faults = self.failed_faults.saturating_add(1);
                    })?;
            }
        }
        Ok(true)
    }

    fn commit_page(
        &mut self,
        page_index: usize,
        publication: LeafPublication<'_>,
        requested_permissions: Stage2PagePermissions,
    ) -> Result<(), Error> {
        self.ensure_healthy()?;
        if self.mapped_pages.get(page_index).is_none() {
            return Err(Error::InvalidRange);
        }
        if self.mapped_pages.get(page_index).unwrap_or(false) {
            return Ok(());
        }
        // A seal snapshot can publish a VMO-resident page before stage-2 has a
        // leaf for it. Preserve that already-earned execute authority when a
        // later data access installs the leaf.
        let permissions = if self
            .instruction_ready_pages
            .get(page_index)
            .unwrap_or(false)
        {
            Stage2PagePermissions::ReadWriteExecute
        } else {
            requested_permissions
        };
        let active_cpu = if publication.is_active() {
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
        let page_charge = self.domain.reserve(self.guest_page_amount())?;
        let physical = self.prepare_backing_page(page_index)?;
        if permissions.is_executable()
            && let LeafPublication::Active(pin) = publication
        {
            self.publish_instruction_page_bytes(physical, pin)?;
        }
        let ipa = self.page_ipa(page_index)?;
        let mapping = {
            let mut allocate_table =
                |pages, alignment| self.table_pages.allocate_zeroed(pages, alignment);
            if publication.is_active() {
                // SAFETY: Called only from a lower-EL translation fault while this
                // VM is active, under the address-space lock.
                unsafe {
                    self.stage2.map_normal_page_active(
                        ipa,
                        physical.get(),
                        permissions,
                        &mut allocate_table,
                    )
                }
            } else {
                // SAFETY: Stage2PagePool preserves the allocation contract stated
                // at construction, and &mut self plus the owning VM's
                // address-space lock serializes all hierarchy mutation.
                let result = unsafe {
                    self.stage2.map_normal_page(
                        ipa,
                        physical.get(),
                        permissions,
                        &mut allocate_table,
                    )
                };
                result.map_err(ActiveMappingError::BeforeInstall)
            }
        };
        let committed_error = match mapping {
            Ok(()) => None,
            Err(ActiveMappingError::BeforeInstall(error)) => {
                return match self.table_pages.take_error() {
                    Some(error) => Err(error),
                    None => Err(error.into()),
                };
            }
            Err(ActiveMappingError::InstalledButInvalidationFailed(error)) => Some(error),
        };
        // Publication commits physical ownership even when the subsequent
        // invalidation fails. Store the owner before reporting that failure so
        // the live descriptor can never point at a page returned to the buddy.
        accumulate_charge(&mut self.guest_page_charge, page_charge.commit());
        self.mapped_pages.set(page_index, true)?;
        self.committed_pages += 1;
        self.commit_translation_change(active_cpu);
        if let Some(error) = committed_error {
            self.poisoned = true;
            // The single-active execution lease excludes a concurrent vCPU,
            // but a failed local invalidation still leaves architectural state
            // ambiguous. Ownership is retained above before global fail-stop.
            crate::kernel::crash::fatal(format_args!(
                "HypeR: committed stage-2 mapping invalidation failed: {error:?}"
            ));
        }
        if requested_permissions.is_executable() {
            self.mark_instruction_page_ready(page_index)?;
        }
        Ok(())
    }

    fn promote_active_page_to_executable(
        &mut self,
        page_index: usize,
        pin: &dyn hyper::cpu::PinnedExecution,
    ) -> Result<(), Error> {
        let physical = self.backing_physical_page(page_index)?;
        self.publish_instruction_page_bytes(physical, pin)?;
        let cpu = crate::kernel::cpu::current_index().ok_or(Error::InvalidCpu)?;
        self.residency
            .check_single_active(cpu.get(), self.translation_epoch)
            .map_err(Error::Residency)?;
        let ipa = self.page_ipa(page_index)?;
        // SAFETY: Fault dispatch proves this exact address space is active on
        // `cpu`, and the address-space lock plus execution lease serialize the
        // break-before-make/permission update.
        let promotion = unsafe { self.stage2.make_normal_page_executable_active(ipa) };
        match promotion {
            Ok(()) => {}
            Err(ActiveMappingError::BeforeInstall(error)) => return Err(error.into()),
            Err(ActiveMappingError::InstalledButInvalidationFailed(error)) => {
                self.poisoned = true;
                crate::kernel::crash::fatal(format_args!(
                    "HypeR: executable stage-2 permission invalidation failed: {error:?}"
                ));
            }
        }
        self.commit_translation_change(Some(cpu));
        self.mark_instruction_page_ready(page_index)
    }

    /// Publishes host-written bytes before an active executable stage-2 leaf
    /// can name this page. Data-only demand pages intentionally skip this path.
    fn publish_instruction_page_bytes(
        &self,
        physical: PhysicalAddress,
        pin: &dyn hyper::cpu::PinnedExecution,
    ) -> Result<(), Error> {
        let address = linear_address(physical)?;
        // SAFETY: The active guest is stopped in its VM-exit callback, the
        // address-space lock serializes this page, and the typed pin prevents
        // migration until both architecture-requested maintenance phases end.
        // The page is complete and aligned, so cache-line rounding stays within
        // its retained allocation.
        unsafe {
            crate::hal::cache::publish_instruction_ranges(pin, |visit| {
                visit(address, PAGE_SIZE as usize)
            })
        }?;
        Ok(())
    }

    #[cfg(feature = "kernel-self-test")]
    fn publish_instruction_page(
        &mut self,
        page_index: usize,
        physical: PhysicalAddress,
        pin: &dyn hyper::cpu::PinnedExecution,
    ) -> Result<(), Error> {
        self.publish_instruction_page_bytes(physical, pin)?;
        self.mark_instruction_page_ready(page_index)
    }

    fn mark_instruction_page_ready(&mut self, page_index: usize) -> Result<(), Error> {
        let ready = self
            .instruction_ready_pages
            .get(page_index)
            .ok_or(Error::InvalidRange)?;
        if !ready {
            self.instruction_ready_pages.set(page_index, true)?;
            self.advance_instruction_epoch();
        }
        Ok(())
    }

    fn commit_translation_change(&mut self, active_cpu: Option<hyper::cpu::CpuIndex>) {
        let previous_epoch = self.translation_epoch;
        self.translation_epoch = match previous_epoch.checked_add(1) {
            Some(epoch) => epoch,
            None => {
                self.poisoned = true;
                crate::kernel::crash::fatal(format_args!("HypeR: stage-2 mapping epoch exhausted"));
            }
        };
        if let Some(cpu) = active_cpu {
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
    }

    fn advance_instruction_epoch(&self) {
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
    }

    fn prepare_backing_page(&mut self, page_index: usize) -> Result<PhysicalAddress, Error> {
        let offset = (page_index as u64)
            .checked_mul(PAGE_SIZE)
            .ok_or(Error::AddressOverflow)?;
        match &mut self.backing {
            #[cfg(feature = "kernel-self-test")]
            GuestMemoryBacking::KernelOwned(pages) => {
                let slot = pages.get_mut(page_index).ok_or(Error::InvalidRange)?;
                if slot.is_none() {
                    let reservation = self.domain.reserve(
                        ResourceAmount::ZERO
                            .with(ResourceKind::KernelMemoryBytes, PAGE_SIZE)
                            .with(ResourceKind::CommittedPages, 1),
                    )?;
                    let page = PageBlock::allocate_for(0, PageOwner::Guest)?;
                    let physical = page.physical();
                    let virtual_address = linear_address(physical)?;
                    // SAFETY: The new page is exclusively VM-owned and fully
                    // covered by the permanent writable linear map.
                    unsafe { write_bytes(virtual_address as *mut u8, 0, PAGE_SIZE as usize) };
                    *slot = Some(page);
                    accumulate_charge(&mut self.backing_page_charge, reservation.commit());
                }
                slot.as_ref()
                    .map(PageBlock::physical)
                    .ok_or(Error::InvalidRange)
            }
            GuestMemoryBacking::SharedVmo(backing) => {
                backing.populate_page(offset)?;
                backing.physical_page(offset).map_err(Into::into)
            }
        }
    }

    fn guest_page_amount(&self) -> ResourceAmount {
        ResourceAmount::ZERO
            .with(ResourceKind::PinnedPages, 1)
            .with(ResourceKind::GuestPages, 1)
    }

    fn backing_page_is_resident(&self, page_index: usize) -> Result<bool, Error> {
        let offset = (page_index as u64)
            .checked_mul(PAGE_SIZE)
            .ok_or(Error::AddressOverflow)?;
        match &self.backing {
            #[cfg(feature = "kernel-self-test")]
            GuestMemoryBacking::KernelOwned(pages) => pages
                .get(page_index)
                .map(Option::is_some)
                .ok_or(Error::InvalidRange),
            GuestMemoryBacking::SharedVmo(backing) => {
                backing.page_is_resident(offset).map_err(Into::into)
            }
        }
    }

    fn backing_physical_page(&self, page_index: usize) -> Result<PhysicalAddress, Error> {
        let offset = (page_index as u64)
            .checked_mul(PAGE_SIZE)
            .ok_or(Error::AddressOverflow)?;
        match &self.backing {
            #[cfg(feature = "kernel-self-test")]
            GuestMemoryBacking::KernelOwned(pages) => pages
                .get(page_index)
                .and_then(Option::as_ref)
                .map(PageBlock::physical)
                .ok_or(Error::InvalidRange),
            GuestMemoryBacking::SharedVmo(backing) => {
                backing.physical_page(offset).map_err(Into::into)
            }
        }
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

    pub(super) fn ensure_healthy(&self) -> Result<(), Error> {
        if self.poisoned {
            Err(Error::Poisoned)
        } else {
            Ok(())
        }
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
        if !self.backing_page_is_resident(page_index)? {
            destination.fill(0);
            return Ok(());
        }
        let object_offset = (page_index as u64)
            .checked_mul(PAGE_SIZE)
            .and_then(|offset| offset.checked_add(page_offset as u64))
            .ok_or(Error::AddressOverflow)?;
        match &self.backing {
            #[cfg(feature = "kernel-self-test")]
            GuestMemoryBacking::KernelOwned(_) => {
                let physical = self.backing_physical_page(page_index)?;
                let source = linear_address(physical)?
                    .checked_add(page_offset)
                    .ok_or(Error::AddressOverflow)?;
                // SAFETY: The generic copy layer bounds this chunk to one
                // VM-owned page, whose mapping remains stable under the
                // address-space lock.
                unsafe {
                    copy_nonoverlapping(
                        source as *const u8,
                        destination.as_mut_ptr(),
                        destination.len(),
                    )
                };
                Ok(())
            }
            GuestMemoryBacking::SharedVmo(backing) => backing
                .read_exposed(object_offset, destination)
                .map_err(Into::into),
        }
    }

    fn write_page(
        &mut self,
        page_index: usize,
        page_offset: usize,
        source: &[u8],
    ) -> Result<(), Self::Error> {
        self.commit_page(
            page_index,
            LeafPublication::Inactive,
            Stage2PagePermissions::ReadWrite,
        )?;
        let object_offset = (page_index as u64)
            .checked_mul(PAGE_SIZE)
            .and_then(|offset| offset.checked_add(page_offset as u64))
            .ok_or(Error::AddressOverflow)?;
        match &self.backing {
            #[cfg(feature = "kernel-self-test")]
            GuestMemoryBacking::KernelOwned(_) => {
                let physical = self.backing_physical_page(page_index)?;
                let destination = linear_address(physical)?
                    .checked_add(page_offset)
                    .ok_or(Error::AddressOverflow)?;
                // SAFETY: The generic copy layer bounds this chunk to one
                // VM-owned page, whose mapping remains stable under the
                // address-space lock.
                unsafe {
                    copy_nonoverlapping(source.as_ptr(), destination as *mut u8, source.len())
                };
                Ok(())
            }
            GuestMemoryBacking::SharedVmo(backing) => backing
                .write_exposed(object_offset, source)
                .map_err(Into::into),
        }
    }
}

pub(in crate::kernel) fn resolve_guest_memory_fault(
    vm: &crate::kernel::vm::registry::VmBinding,
    fault: GuestMemoryFault,
    pin: &dyn hyper::cpu::PinnedExecution,
) -> Result<bool, Error> {
    vm.with_address_space(|address_space| address_space.resolve_guest_memory_fault(fault, pin))
}
