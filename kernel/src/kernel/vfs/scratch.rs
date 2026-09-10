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
