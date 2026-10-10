// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Kernel-only ownership of resident memory shared with a physical backend.

use crate::kernel::{
    accounting::{CommittedCharge, ResourceAmount, ResourceDomain, ResourceKind},
    authority::Rights,
    mm::user_space::{GuestMemoryBacking, MemoryObjectError},
    object::{KernelObject, KernelRef, KernelService, ObjectKind, TransferClass, private},
};
use alloc::vec::Vec;
use hyper::mm::PAGE_SIZE;

/// The lifetime shared by CPU mappings and IOMMU mappings. Closing a source
/// handle, removing a CPU mapping, or dropping a caller's reference cannot
/// release pages still held by a DMA domain. No hardware operation runs in Drop.
pub(crate) struct BackendMemoryLease {
    pages: Vec<u64>,
    backing: GuestMemoryBacking,
    _charge: CommittedCharge,
}

pub(crate) type Lease = KernelRef<BackendMemoryLease, KernelService>;

impl BackendMemoryLease {
    /// Populate and snapshot once, in sleeping context, before any device sees
    /// the buffer. The exclusive hardware lease prevents frame substitution and
    /// incompatible Native writers throughout every derived owner's lifetime.
    pub(crate) fn prepare(
        backing: GuestMemoryBacking,
        domain: &ResourceDomain,
    ) -> Result<Lease, MemoryObjectError> {
        let count = usize::try_from(backing.size() / PAGE_SIZE)
            .map_err(|_| MemoryObjectError::AllocationSize)?;
        if count == 0 || !backing.size().is_multiple_of(PAGE_SIZE) {
            return Err(MemoryObjectError::AllocationSize);
        }
        let bytes = count
            .checked_mul(core::mem::size_of::<u64>())
            .and_then(|bytes| {
                crate::kernel::object::object_allocation_size::<Self>()?.checked_add(bytes)
            })
            .ok_or(MemoryObjectError::AllocationSize)?;
        let charge = domain
            .reserve(
                ResourceAmount::ZERO
                    .with(ResourceKind::KernelObjects, 1)
                    .with(ResourceKind::KernelMemoryBytes, bytes as u64),
            )?
            .commit();
        let mut pages = Vec::new();
        crate::kernel::mm::reclaim::reserve_exact(&mut pages, count)
            .map_err(|_| MemoryObjectError::AllocationSize)?;
        for index in 0..count {
            let offset = index as u64 * PAGE_SIZE;
            backing.populate_page(offset)?;
            pages.push(backing.physical_page(offset)?.get());
        }
        Lease::try_new_service(Self {
            pages,
            backing,
            _charge: charge,
        })
        .map_err(MemoryObjectError::Object)
    }

    pub(crate) fn length(&self) -> u64 {
        self.backing.size()
    }

    pub(crate) fn physical_page(&self, offset: u64) -> u64 {
        // Only validated page offsets from retained mappings reach this path.
        if !offset.is_multiple_of(PAGE_SIZE) || offset >= self.length() {
            hyper::debug::invariant_failure("DMA lease page outside retained buffer");
        }
        self.pages[(offset / PAGE_SIZE) as usize]
    }
}

impl private::Sealed for BackendMemoryLease {}
impl KernelObject for BackendMemoryLease {
    const KIND: ObjectKind = ObjectKind::BACKEND_MEMORY_LEASE;
    const TRANSFER_CLASS: TransferClass = TransferClass::Never;
    const SUPPORTED_RIGHTS: Rights = Rights::NONE;
}

// SAFETY: Construction snapshots only resident pages of a stable VMO protected
// by its exclusive hardware lease. Every KernelRef retains that lease and all
// pages; no borrowed CPU bytes are exposed by this interface.
unsafe impl hyper::drivers::iommu::smmuv3::DmaBuffer for Lease {
    fn length(&self) -> u64 {
        self.object().length()
    }
    fn physical_page(&self, offset: u64) -> u64 {
        self.object().physical_page(offset)
    }
}
