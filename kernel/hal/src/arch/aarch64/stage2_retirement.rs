// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Opaque local request for leased live guest stage-2 invalidation.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Request {
    retiring_vttbr: u64,
    guest_vtcr: u64,
}

impl Request {
    pub(super) const fn new(retiring_vttbr: u64, guest_vtcr: u64) -> Self {
        Self {
            retiring_vttbr,
            guest_vtcr,
        }
    }

    pub(super) const fn retiring_vttbr(self) -> u64 {
        self.retiring_vttbr
    }

    pub(super) const fn guest_vtcr(self) -> u64 {
        self.guest_vtcr
    }
}
