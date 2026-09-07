// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Stable ownership boundary between filesystem data and the image loader.

use alloc::borrow::Cow;

/// Immutable executable contents retained for one loader transaction.
///
/// `RamFs` can lend permanent archive storage without copying. A remote or
/// mutable backend can instead return owned bytes through the same type, so
/// process construction never depends on a backend-specific lifetime.
pub(crate) struct ExecutableSnapshot {
    bytes: Cow<'static, [u8]>,
}

impl ExecutableSnapshot {
    pub(super) const fn borrowed(bytes: &'static [u8]) -> Self {
        Self {
            bytes: Cow::Borrowed(bytes),
        }
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}
