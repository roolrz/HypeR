// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Explicit authority to share frozen writable backing among guest address spaces.

use super::{Error, reserve_object_charge};
use crate::kernel::accounting::{CommittedCharge, ResourceDomain};
use crate::kernel::authority::Rights;
use crate::kernel::mm::user_space::{GuestMemoryBacking, VmoObject};
use crate::kernel::object::{KernelObject, ObjectKind, TransferClass, private};

pub(crate) struct GuestMemoryObject {
    backing: GuestMemoryBacking,
    _charge: CommittedCharge,
}

impl GuestMemoryObject {
    pub(crate) fn try_new(vmo: &VmoObject, domain: &ResourceDomain) -> Result<Self, Error> {
        let charge = reserve_object_charge::<Self>(domain)?;
        Ok(Self {
            backing: GuestMemoryBacking::try_from_vmo(vmo)?,
            _charge: charge,
        })
    }
    pub(crate) fn backing(&self) -> GuestMemoryBacking {
        self.backing.clone()
    }
}
impl private::Sealed for GuestMemoryObject {}
impl private::UserExportable for GuestMemoryObject {}
impl KernelObject for GuestMemoryObject {
    const KIND: ObjectKind = ObjectKind::GUEST_MEMORY;
    const TRANSFER_CLASS: TransferClass = TransferClass::RendezvousOnly;
    const SUPPORTED_RIGHTS: Rights = Rights::TRANSFER
        .union(Rights::DUPLICATE)
        .union(Rights::MAP)
        .union(Rights::INSPECT);
}
