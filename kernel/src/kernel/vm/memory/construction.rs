// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Guest address-space construction and VMID activation.

use hyper::mm::AddressSpaceResidency;
use hyper::sync::atomic::AtomicU64;

#[cfg(feature = "kernel-self-test")]
use super::storage::try_exact_capacity_vec;
use super::storage::{
    AdmittedAddressSpaceMetadata, FixedBitmap, GuestMemoryBacking, Stage2PagePool,
    address_space_metadata_layout, admit_metadata, validate_region,
};
use super::{
    ActiveStage2Identifier, Error, GuestAddressSpace, Stage2Identifier, Stage2IdentifierReservation,
};
use crate::hal::vm::Stage2AddressSpace;
use crate::kernel::accounting::ResourceDomain;
#[cfg(feature = "kernel-self-test")]
use crate::kernel::mm::page_block::PageBlock;
use crate::kernel::mm::user_space::GuestMemoryBacking as SharedGuestMemory;
use crate::kernel::vm::residency_state::Stage2Incarnation;

impl GuestAddressSpace {
    #[cfg(feature = "kernel-self-test")]
    pub(crate) fn new(
        hardware_vmid: Stage2IdentifierReservation,
        ipa_base: u64,
        size: u64,
        domain: &ResourceDomain,
    ) -> Result<Self, Error> {
        let page_count = validate_region(ipa_base, size)?;
        let backing_owner_bytes = page_count
            .checked_mul(core::mem::size_of::<Option<PageBlock>>())
            .ok_or(Error::MetadataAllocation)?;
        let metadata =
            address_space_metadata_layout(ipa_base, size, page_count, backing_owner_bytes)?;
        // Admit every address-space-owned metadata allocation, including the
        // KernelOwned page-owner slots, before allocating any of them.
        let metadata = admit_metadata(domain, metadata)?;
        let mut pages = try_exact_capacity_vec(page_count)?;
        pages.resize_with(page_count, || None);

        Self::with_backing(
            hardware_vmid,
            ipa_base,
            size,
            GuestMemoryBacking::KernelOwned(pages),
            domain,
            metadata,
        )
    }

    /// Builds a sparse guest address space over one userspace-owned VMO.
    ///
    /// The backing owns an independent hardware-mapping lease, so closing the
    /// userspace handle or removing the Native mapping cannot release pages
    /// while stage-2 translations still reference them.
    pub(crate) fn from_vmo(
        hardware_vmid: Stage2IdentifierReservation,
        ipa_base: u64,
        size: u64,
        backing: SharedGuestMemory,
        domain: &ResourceDomain,
    ) -> Result<Self, Error> {
        let page_count = validate_region(ipa_base, size)?;
        if backing.size() != size {
            return Err(Error::InvalidRange);
        }
        // SharedVmo owns and accounts its page slots independently. Charge
        // only metadata allocated by this guest address space.
        let metadata = address_space_metadata_layout(ipa_base, size, page_count, 0)?;
        let metadata = admit_metadata(domain, metadata)?;
        Self::with_backing(
            hardware_vmid,
            ipa_base,
            size,
            GuestMemoryBacking::SharedVmo(backing),
            domain,
            metadata,
        )
    }

    fn with_backing(
        hardware_vmid: Stage2IdentifierReservation,
        ipa_base: u64,
        size: u64,
        backing: GuestMemoryBacking,
        domain: &ResourceDomain,
        metadata: AdmittedAddressSpaceMetadata,
    ) -> Result<Self, Error> {
        let AdmittedAddressSpaceMetadata {
            page_count,
            table_capacity,
            charge: metadata_charge,
        } = metadata;
        let mapped_pages = FixedBitmap::try_new(page_count)?;
        let instruction_ready_pages = FixedBitmap::try_new(page_count)?;

        let mut table_pages = Stage2PagePool::with_capacity(table_capacity, domain)?;
        let identifier = hardware_vmid.value();
        let stage2 = {
            let mut allocate_table =
                |pages, alignment| table_pages.allocate_zeroed(pages, alignment);
            // SAFETY: The consumed reservation uniquely owns this VMID.
            // Stage2PagePool returns accounted, uniquely owned, zeroed, aligned
            // RAM and retains every hierarchy page through retirement.
            unsafe { Stage2AddressSpace::new(identifier, &mut allocate_table) }
        };
        let stage2 = match stage2 {
            Ok(stage2) => stage2,
            Err(error) => match table_pages.take_error() {
                Some(error) => return Err(error),
                None => return Err(Error::Stage2(error)),
            },
        };
        Ok(Self {
            ipa_base,
            size,
            domain: domain.clone(),
            backing,
            mapped_pages,
            instruction_ready_pages,
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
            guest_page_charge: None,
            #[cfg(feature = "kernel-self-test")]
            backing_page_charge: None,
            _metadata_charge: metadata_charge,
        })
    }

    #[cfg(feature = "kernel-self-test")]
    pub(crate) fn metadata_requirements_for_test(
        ipa_base: u64,
        size: u64,
    ) -> Result<(u64, u64), Error> {
        let page_count = validate_region(ipa_base, size)?;
        let common = address_space_metadata_layout(ipa_base, size, page_count, 0)?.bytes;
        let backing_owner_bytes = page_count
            .checked_mul(core::mem::size_of::<Option<PageBlock>>())
            .ok_or(Error::MetadataAllocation)?;
        let kernel_owned =
            address_space_metadata_layout(ipa_base, size, page_count, backing_owner_bytes)?.bytes;
        Ok((common, kernel_owned))
    }

    #[cfg(feature = "kernel-self-test")]
    pub(crate) fn retained_metadata_bytes_for_test(&self) -> Result<u64, Error> {
        let bitmap_bytes = self
            .mapped_pages
            .retained_bytes()
            .checked_add(self.instruction_ready_pages.retained_bytes())
            .ok_or(Error::MetadataAllocation)?;
        let table_owner_bytes = self
            .table_pages
            .pages
            .capacity()
            .checked_mul(core::mem::size_of::<PageBlock>())
            .ok_or(Error::MetadataAllocation)?;
        let backing_owner_bytes = match &self.backing {
            GuestMemoryBacking::KernelOwned(pages) => pages
                .capacity()
                .checked_mul(core::mem::size_of::<Option<PageBlock>>())
                .ok_or(Error::MetadataAllocation)?,
            GuestMemoryBacking::SharedVmo(_) => 0,
        };
        bitmap_bytes
            .checked_add(table_owner_bytes)
            .and_then(|bytes| bytes.checked_add(backing_owner_bytes))
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(Error::MetadataAllocation)
    }

    pub(in crate::kernel::vm) fn activate_identifier_for_install(
        &mut self,
    ) -> Result<(), crate::kernel::vm::registry::Error> {
        use crate::kernel::vm::address_space_state::{IdentifierState, activation_may_begin};

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
            return Err(crate::kernel::vm::registry::Error::InvalidReservation);
        }
        let previous = core::mem::replace(&mut self.identifier, Stage2Identifier::Poisoned);
        let Stage2Identifier::Reserved(Some(reservation)) = previous else {
            // The exclusive preflight above makes this branch impossible.
            crate::hal::cpu::halt()
        };
        let active = reservation
            .activate()
            .map_err(|_| crate::kernel::vm::registry::Error::IdentityExhausted)?;
        self.identifier = Stage2Identifier::Active(active);
        Ok(())
    }

    pub(super) fn active_identifier(&self) -> Result<&ActiveStage2Identifier, Error> {
        match &self.identifier {
            Stage2Identifier::Active(identifier) => Ok(identifier),
            Stage2Identifier::Retiring(_) | Stage2Identifier::Retired => Err(Error::Poisoned),
            Stage2Identifier::Reserved(_) | Stage2Identifier::Poisoned => Err(Error::Poisoned),
        }
    }

    pub(super) fn incarnation(&self) -> Result<Stage2Incarnation, Error> {
        let identifier = self.active_identifier()?;
        Ok(Stage2Incarnation::new(
            self.stage2.root_address(),
            identifier.value(),
            identifier.generation(),
            self.translation_epoch,
        ))
    }
}
