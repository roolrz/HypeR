// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Validated registration of caller-owned virtual-serial ring pages.

use crate::kernel::accounting::{CommittedCharge, ResourceAmount, ResourceDomain, ResourceKind};
use crate::kernel::mm::user_space::{
    DomainAccount, KernelPageBackend, MemoryObjectError, VmoObject, WritableMappingLease,
};
use core::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use hyper::abi::native::{
    HYPER_NATIVE_VIRTUAL_SERIAL_OUTPUT_BYTES as BYTES,
    HYPER_NATIVE_VIRTUAL_SERIAL_OUTPUT_CAPACITY as CAPACITY,
    HYPER_NATIVE_VIRTUAL_SERIAL_OUTPUT_HEADER_BYTES as HEADER,
};

const PAGES: usize = (BYTES / hyper::mm::PAGE_SIZE) as usize;

/// A registration pins caller-owned VMO pages independently of mappings and
/// handles. Its writable lease excludes ordinary kernel copy/snapshot access;
/// concurrent user writes are untrusted and never supply kernel addresses.
pub(crate) struct SharedAtomicOutput {
    pages: [usize; PAGES],
    _lease: WritableMappingLease<KernelPageBackend, DomainAccount>,
    _pinned: CommittedCharge,
    produced: AtomicU64,
    consumed: AtomicU64,
    dropped: AtomicU64,
}

impl SharedAtomicOutput {
    pub(crate) fn try_register(
        object: &VmoObject,
        domain: &ResourceDomain,
    ) -> Result<Self, MemoryObjectError> {
        if object.size() != BYTES {
            return Err(MemoryObjectError::Vmo(
                crate::kernel::mm::user_space::VmoError::InvalidRange,
            ));
        }
        let pinned = domain
            .reserve(ResourceAmount::ZERO.with(ResourceKind::PinnedPages, PAGES as u64))?
            .commit();
        let storage = object.writable().ok_or(MemoryObjectError::WrongVariant)?;
        let lease = storage.try_mapping_write_lease()?;
        storage
            .populate(0, BYTES)
            .map_err(|failure| MemoryObjectError::Vmo(failure.cause))?;
        let mut pages = [0; PAGES];
        for (index, address) in pages.iter_mut().enumerate() {
            let physical = storage.resident_physical_page(index as u64 * hyper::mm::PAGE_SIZE)?;
            *address = crate::kernel::mm::memory::linear_address(physical.get())
                .ok_or(MemoryObjectError::WrongVariant)?;
        }
        let output = Self {
            pages,
            _pinned: pinned,
            _lease: lease,
            produced: AtomicU64::new(0),
            consumed: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
        };
        output.word(0).store(0, Ordering::Relaxed);
        output.word(8).store(0, Ordering::Relaxed);
        output.word(4096).store(0, Ordering::Relaxed);
        Ok(output)
    }

    fn word(&self, offset: usize) -> &AtomicU64 {
        let page_size = hyper::mm::PAGE_SIZE as usize;
        let address = self.pages[offset / page_size] + offset % page_size;
        // SAFETY: fixed ABI offsets are aligned and inside pinned pages. The
        // lease excludes ordinary kernel copies and snapshots. User contents
        // may change arbitrarily; all kernel accesses use bounded atomics.
        unsafe { &*core::ptr::with_exposed_provenance::<AtomicU64>(address) }
    }

    /// Called only by the one admitted vCPU of the bound VM, including after
    /// migration. Multi-vCPU UART production requires a new publication proof.
    pub(crate) fn publish(&self, byte: u8) {
        let produced = self.produced.load(Ordering::Relaxed);
        let consumed = self.consumed.load(Ordering::Relaxed);
        let candidate = self.word(4096).load(Ordering::Acquire);
        let consumed = super::serial_ring::accept_consumer(produced, consumed, candidate);
        self.consumed.store(consumed, Ordering::Relaxed);
        let Some(slot) = super::serial_ring::writable_slot(produced, consumed, CAPACITY) else {
            let dropped = self.dropped.load(Ordering::Relaxed).saturating_add(1);
            self.dropped.store(dropped, Ordering::Relaxed);
            self.word(8).store(dropped, Ordering::Relaxed);
            return;
        };
        let offset = HEADER as usize + slot;
        let page_size = hyper::mm::PAGE_SIZE as usize;
        let address = self.pages[offset / page_size] + offset % page_size;
        // SAFETY: the offset is bounded by the fixed ring capacity. Output
        // pages stay pinned independently of userspace mappings. Atomic bytes keep a
        // malicious premature consumer acknowledgement from creating a Rust
        // data race with a still-reading user. Correct consumers acknowledge
        // only after reading; Acquire above then permits slot reuse.
        unsafe { &*core::ptr::with_exposed_provenance::<AtomicU8>(address) }
            .store(byte, Ordering::Relaxed);
        self.word(0).store(produced + 1, Ordering::Release);
        self.produced.store(produced + 1, Ordering::Relaxed);
    }
}
