// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Private ownership of the default VM's registry-minted lifecycle authority.

use hyper::sync::InterruptSpinLock;

use super::registry::VmControl;

type ControlLock = InterruptSpinLock<Option<VmControl>, crate::hal::irq::LocalMask>;

static DEFAULT_VM: ControlLock = InterruptSpinLock::new(None);

/// Retains boot policy's sole default-VM lifecycle authority.
pub(super) fn retain_default(control: VmControl) {
    DEFAULT_VM.with(|slot| {
        if slot.is_some() {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: default VM lifecycle authority was published twice"
            ));
        }
        *slot = Some(control);
    });
}

/// Transfers default-VM lifecycle authority to a private management path.
#[allow(dead_code)]
pub(super) fn take_default() -> Option<VmControl> {
    DEFAULT_VM.with(Option::take)
}
