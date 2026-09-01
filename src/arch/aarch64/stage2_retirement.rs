// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Opaque local request for final guest stage-2 retirement.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(test, allow(dead_code))]
pub(crate) struct Request {
    retiring_vttbr: u64,
    guest_vtcr: u64,
}

#[cfg_attr(test, allow(dead_code))]
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

/// Keeps an unrelated local guest selection intact, but never restores the
/// translation identity being retired.
#[cfg(test)]
pub(crate) const fn restore_vttbr(saved: u64, retiring: u64) -> u64 {
    if saved == retiring { 0 } else { saved }
}

#[cfg(test)]
mod tests {
    use super::restore_vttbr;

    #[test]
    fn exact_retiring_selection_is_parked_at_the_neutral_root() {
        assert_eq!(restore_vttbr(0x1000_4000, 0x1000_4000), 0);
    }

    #[test]
    fn unrelated_selection_is_restored() {
        assert_eq!(restore_vttbr(0x2000_8000, 0x1000_4000), 0x2000_8000);
    }
}
