// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Stable ownership boundary between filesystem data and the image loader.

use alloc::borrow::Cow;

use crate::kernel::mm::user_space::{DomainAccount, KernelPageBackend, SnapshotVmo};

pub(super) type FileSnapshotStorage = SnapshotVmo<KernelPageBackend, DomainAccount>;

/// One coherent file generation, with contiguous parser bytes and shared pages.
///
/// Parser storage lasts for the loading transaction. The physical snapshot can
/// outlive it through mappings, retaining its filesystem charge. This value
/// grants no executable authority; callers separately resolve that permission.
pub(crate) struct ExecutableSnapshot {
    storage: FileSnapshotStorage,
    bytes: Cow<'static, [u8]>,
    _charge: Option<crate::kernel::accounting::CommittedCharge>,
}

impl ExecutableSnapshot {
    pub(super) fn borrowed(bytes: &'static [u8], storage: FileSnapshotStorage) -> Self {
        Self {
            storage,
            bytes: Cow::Borrowed(bytes),
            _charge: None,
        }
    }

    pub(super) fn owned(
        bytes: alloc::vec::Vec<u8>,
        charge: crate::kernel::accounting::CommittedCharge,
        storage: FileSnapshotStorage,
    ) -> Self {
        Self {
            storage,
            bytes: Cow::Owned(bytes),
            _charge: Some(charge),
        }
    }

    pub(crate) fn storage(&self) -> &FileSnapshotStorage {
        &self.storage
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}
