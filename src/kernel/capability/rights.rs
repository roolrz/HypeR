// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Monotonically decreasing authority masks.

use hyper::abi::native;

/// Rights attached to one process-local handle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Rights(u64);

impl Rights {
    pub(crate) const NONE: Self = Self(0);
    pub(crate) const DUPLICATE: Self = Self(native::HYPER_NATIVE_RIGHT_DUPLICATE);
    pub(crate) const TRANSFER: Self = Self(native::HYPER_NATIVE_RIGHT_TRANSFER);
    pub(crate) const WAIT: Self = Self(native::HYPER_NATIVE_RIGHT_WAIT);
    pub(crate) const INSPECT: Self = Self(native::HYPER_NATIVE_RIGHT_INSPECT);
    pub(crate) const READ: Self = Self(native::HYPER_NATIVE_RIGHT_READ);
    pub(crate) const WRITE: Self = Self(native::HYPER_NATIVE_RIGHT_WRITE);
    pub(crate) const MAP: Self = Self(native::HYPER_NATIVE_RIGHT_MAP);
    pub(crate) const EXECUTE: Self = Self(native::HYPER_NATIVE_RIGHT_EXECUTE);
    pub(crate) const RESIZE: Self = Self(native::HYPER_NATIVE_RIGHT_RESIZE);
    pub(crate) const PIN: Self = Self(native::HYPER_NATIVE_RIGHT_PIN);
    pub(crate) const START: Self = Self(native::HYPER_NATIVE_RIGHT_START);
    pub(crate) const REQUEST_STOP: Self = Self(native::HYPER_NATIVE_RIGHT_REQUEST_STOP);
    pub(crate) const RUN_VCPU: Self = Self(native::HYPER_NATIVE_RIGHT_RUN_VCPU);
    pub(crate) const INJECT_INTERRUPT: Self = Self(native::HYPER_NATIVE_RIGHT_INJECT_INTERRUPT);
    pub(crate) const GRANT_MEMORY: Self = Self(native::HYPER_NATIVE_RIGHT_GRANT_MEMORY);
    pub(crate) const ASSIGN_DEVICE: Self = Self(native::HYPER_NATIVE_RIGHT_ASSIGN_DEVICE);
    pub(crate) const MAP_DMA: Self = Self(native::HYPER_NATIVE_RIGHT_MAP_DMA);
    pub(crate) const ACK_INTERRUPT: Self = Self(native::HYPER_NATIVE_RIGHT_ACK_INTERRUPT);
    pub(crate) const REVOKE: Self = Self(native::HYPER_NATIVE_RIGHT_REVOKE);
    pub(crate) const KNOWN: Self = Self(native::HYPER_NATIVE_RIGHTS_MASK);

    pub(crate) const fn from_bits(bits: u64) -> Option<Self> {
        if bits & !Self::KNOWN.0 == 0 {
            Some(Self(bits))
        } else {
            None
        }
    }

    pub(crate) const fn bits(self) -> u64 {
        self.0
    }

    pub(crate) const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub(crate) const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    pub(crate) const fn contains(self, required: Self) -> bool {
        self.intersection(required).0 == required.0
    }

    pub(crate) const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl Default for Rights {
    fn default() -> Self {
        Self::NONE
    }
}
