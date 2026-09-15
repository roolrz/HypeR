// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native ownership of a backend VM's one-shot device mapping.

use crate::kernel::accounting::CommittedCharge;
use crate::kernel::authority::Rights;
use crate::kernel::capability::{HandleFlags, HandleValue, PreparedHandle};
use crate::kernel::object::{KernelObject, ObjectKind, ObjectPublication, TransferClass, private};
use crate::kernel::process::Process;
use crate::kernel::vm::{
    objects::{GuestMemoryObject, VirtualMachineObject},
    registry,
    service::Error,
};

pub(crate) struct GuestMapping {
    backend: registry::VmId,
    token: u64,
    _charge: CommittedCharge,
}
impl private::Sealed for GuestMapping {}
impl private::UserExportable for GuestMapping {}
impl KernelObject for GuestMapping {
    const KIND: ObjectKind = ObjectKind::GUEST_MAPPING;
    const TRANSFER_CLASS: TransferClass = TransferClass::RendezvousOnly;
    const SUPPORTED_RIGHTS: Rights = Rights::WRITE.union(Rights::TRANSFER).union(Rights::INSPECT);
    // Closing a handle cannot establish that a device has stopped DMA. The VM
    // owns the record until explicit guest-proven release or VM retirement.
}

fn classify(error: super::Error) -> Error {
    crate::kernel::vm::objects::Error::MemoryLayout(error).into()
}

pub(crate) fn create(
    process: &Process,
    backend: HandleValue,
    memory: HandleValue,
    frontend: u64,
) -> Result<(HandleValue, u64), Error> {
    if !crate::hal::vm::supports_io_notifications() {
        return Err(Error::NotSupported);
    }
    let vm = process.resolve_handle::<VirtualMachineObject>(backend, Rights::WRITE)?;
    let memory = process.resolve_handle::<GuestMemoryObject>(memory, Rights::MAP)?;
    let domain = process.resource_domain();
    let owner = vm.object().owner();
    let id = owner.io_update_id().map_err(|_| Error::BadState)?;
    let binding = registry::acquire_binding(id).map_err(|_| Error::BadState)?;
    let mut mapping = super::live::Mapping::prepare(memory.object().backing(), frontend, &domain)
        .map_err(classify)?;
    let token = binding
        .with_address_space(|space| space.live.reserve_token())
        .map_err(classify)?;
    mapping.token = token;
    let mut mapping = Some(hyper::mm::try_box(mapping).map_err(|_| Error::NoMemory)?);
    let charge = crate::kernel::vm::objects::reserve_object_charge::<GuestMapping>(&domain)
        .map_err(Error::from)?;
    let reservation = process.reserve_handles::<1>()?;
    let result = (|| {
        let publication = ObjectPublication::try_new(GuestMapping {
            backend: id,
            token,
            _charge: charge,
        })
        .map_err(|_| Error::NoMemory)?;
        let prepared = PreparedHandle::try_from_new_object(
            publication,
            GuestMapping::SUPPORTED_RIGHTS,
            HandleFlags::NONE,
        )
        .map_err(|_| Error::NoMemory)?;
        let mut committed = false;
        for _ in 0..8 {
            binding
                .with_address_space(|space| {
                    space.prepare_live_install(mapping.as_ref().ok_or(super::Error::InvalidRange)?)
                })
                .map_err(classify)?;
            let installed = owner
                .with_io_update(|current| {
                    if current != id {
                        return Err(crate::kernel::vm::io::Error::BadState);
                    }
                    Ok(binding.with_address_space(|space| space.install_live(&mut mapping)))
                })
                .map_err(|_| Error::BadState)?
                .map_err(classify)?;
            if installed.is_some() {
                committed = true;
                break;
            }
        }
        if !committed {
            return Err(Error::WouldBlock);
        }
        Ok((prepared, token))
    })();
    let (prepared, token) = match result {
        Ok(value) => value,
        Err(error) => {
            process.abort_handles(reservation);
            return Err(error);
        }
    };
    match process.publish_handles(reservation, [prepared]) {
        Ok(handles) => Ok((handles[0], token)),
        Err(failure) => {
            // A never-admitted token cannot have published a leaf or acquired
            // a device reference. Cancel under the same lock as guest admit.
            // If the guest already admitted it, retain the record as quarantine;
            // handle publication failure never fabricates a DMA proof.
            let cancelled = binding.with_address_space(|space| {
                let slot = space
                    .live
                    .slots
                    .iter_mut()
                    .find(|slot| slot.as_ref().is_some_and(|record| record.token == token))?;
                if slot.as_ref()?.state != super::grant_state::State::Created {
                    return None;
                }
                slot.take()
            });
            drop(cancelled);
            Err(failure.error.into())
        }
    }
}

pub(crate) fn release(process: &Process, handle: HandleValue) -> Result<(), Error> {
    let mapping = process.resolve_handle::<GuestMapping>(handle, Rights::WRITE)?;
    let mapping = mapping.object();
    let binding = registry::acquire_binding(mapping.backend).map_err(|_| Error::BadState)?;
    let synchronization =
        super::prepare_live_synchronization(&binding).map_err(|error| match error {
            super::SynchronizationError::Unsupported => Error::NotSupported,
            super::SynchronizationError::TransportBusy => Error::WouldBlock,
            super::SynchronizationError::TopologyUnavailable => Error::BadState,
            super::SynchronizationError::Memory(error) => classify(error),
        })?;
    binding.with_address_space(|space| {
        let record = space.live.find_mut(mapping.token).ok_or(Error::BadState)?;
        if !record.state.retire() {
            return Err(Error::Busy);
        }
        for extent in &record.extents {
            for ipa in (extent.alias..extent.alias + extent.length).step_by(4096) {
                // SAFETY: lookup was withdrawn under this address-space lock;
                // the record still owns every page through the all-CPU barrier.
                if unsafe { space.stage2.clear_page(ipa) }.is_err() {
                    crate::kernel::crash::fatal(format_args!(
                        "HypeR: live grant contains non-page stage-2 leaf"
                    ));
                }
            }
        }
        Ok(())
    })?;
    synchronization.execute();
    let retired = binding.with_address_space(|space| {
        space
            .live
            .slots
            .iter_mut()
            .find(|slot| {
                slot.as_ref()
                    .is_some_and(|record| record.token == mapping.token)
            })
            .and_then(Option::take)
    });
    drop(retired);
    Ok(())
}
