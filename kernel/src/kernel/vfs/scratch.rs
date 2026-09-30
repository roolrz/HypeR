// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Request-owned scratch admission, independent of capability creator lifetime.

use crate::kernel::accounting::{
    CommittedCharge, ResourceAmount, ResourceDomain, ResourceError, ResourceKind,
};
use hyper::fs::file_data::StorageBudget;

#[derive(Clone)]
pub(crate) struct ScratchBudget(ResourceDomain);
impl ScratchBudget {
    pub(crate) fn new(sponsor: &ResourceDomain) -> Self {
        Self(sponsor.clone())
    }
}
impl StorageBudget for ScratchBudget {
    type Charge = CommittedCharge;
    type Error = ResourceError;
    fn reserve(&self, bytes: usize) -> Result<CommittedCharge, ResourceError> {
        self.0
            .reserve(ResourceAmount::ZERO.with(ResourceKind::KernelMemoryBytes, bytes as u64))
            .map(|charge| charge.commit())
    }
}
pub(crate) type ScratchVec<T> = hyper::fs::scratch::BudgetedVec<T, ScratchBudget>;
pub(crate) type ScratchString = hyper::fs::scratch::BudgetedString<ScratchBudget>;

/// Prepare byte scratch before backend access or user-copy side effects.
/// Capacity overflow and unrelated quota errors remain terminal; only the
/// exact physical backing or denying memory domain requests scheduled reclaim.
pub(crate) fn resize_bytes(
    values: &mut ScratchVec<u8>,
    length: usize,
) -> Result<(), hyper::fs::scratch::Error<ResourceError>> {
    use crate::kernel::mm::reclaim::{Target, retry_prepare_with};
    use hyper::fs::scratch::Error;

    retry_prepare_with(
        || values.resize(length, 0),
        |error| match error {
            Error::Allocation => core::alloc::Layout::array::<u8>(length)
                .ok()
                .and_then(crate::kernel::mm::cache_memory::allocation_page_bound)
                .map(|pages| Target::PhysicalOrder(pages.trailing_zeros() as usize)),
            Error::Budget(ResourceError::LimitExceeded {
                domain,
                resource: ResourceKind::KernelMemoryBytes,
                ..
            }) => Some(Target::Domain(*domain)),
            _ => None,
        },
    )
}

impl From<hyper::fs::scratch::Error<ResourceError>> for super::Error {
    fn from(error: hyper::fs::scratch::Error<ResourceError>) -> Self {
        match error {
            hyper::fs::scratch::Error::Allocation => Self::Allocation,
            hyper::fs::scratch::Error::Size => Self::InvalidPath,
            hyper::fs::scratch::Error::Budget(error) => Self::Resource(error),
        }
    }
}
impl From<hyper::fs::scratch::Error<ResourceError>> for super::instance::Error {
    fn from(error: hyper::fs::scratch::Error<ResourceError>) -> Self {
        match error {
            hyper::fs::scratch::Error::Allocation => Self::Allocation,
            hyper::fs::scratch::Error::Size => Self::InvalidInput,
            hyper::fs::scratch::Error::Budget(error) => Self::Resource(error),
        }
    }
}
