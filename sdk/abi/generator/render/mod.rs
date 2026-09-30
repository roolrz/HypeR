// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Target-specific renderers share only ABI vocabulary.

mod c;
mod reference;
mod rust;

pub(super) use c::render_c;
pub(super) use reference::render_reference;
pub(super) use rust::render_rust;

use super::schema::TransferClass;

const fn transfer_class_constant(class: TransferClass) -> &'static str {
    match class {
        TransferClass::Forbidden => "HYPER_NATIVE_TRANSFER_CLASS_FORBIDDEN",
        TransferClass::General => "HYPER_NATIVE_TRANSFER_CLASS_GENERAL",
        TransferClass::RendezvousOnly => "HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY",
    }
}
