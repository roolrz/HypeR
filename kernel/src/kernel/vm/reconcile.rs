// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Durable publication of saved virtual-interrupt model changes.
//!
//! Device producers mutate the VM-owned interrupt model before publishing
//! work here. The running vCPU consumes publication before reconciling its
//! private hardware bank. Publication is deliberately independent of the
//! transient CPU-local `active_vcpu` borrow.

use hyper::sync::atomic::{AtomicBool, Ordering};

pub(super) struct ReconcilePublication {
    pending: AtomicBool,
}

impl ReconcilePublication {
    pub(super) const fn new() -> Self {
        Self {
            pending: AtomicBool::new(false),
        }
    }

    /// Publishes a completed saved-model mutation.
    pub(super) fn publish(&self) {
        self.pending.store(true, Ordering::Release);
    }

    /// Claims all work published before this exchange.
    ///
    /// A producer racing after the exchange leaves `pending` set for the
    /// final pre-entry check or the targeted IRQ-tail reconciliation.
    pub(super) fn take(&self) -> bool {
        self.pending.swap(false, Ordering::AcqRel)
    }

    /// Restores a claim whose hardware reconciliation did not complete.
    pub(super) fn restore(&self) {
        self.publish();
    }

    // Kept available to the architecture-independent model tests; secondary
    // kernels do not observe the bit until they gain an asynchronous producer.
    #[allow(dead_code)]
    pub(super) fn pending(&self) -> bool {
        self.pending.load(Ordering::Acquire)
    }
}
