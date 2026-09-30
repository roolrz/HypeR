// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native object service calls under borrowed Process authority.

use super::DeferredProcessServices;
use crate::kernel::abi::native::{ObjectServiceError, ObjectServices};
use crate::kernel::accounting::{ResourceAmount, ResourceKind};
use crate::kernel::capability::{HandleValue, ResolvedWaitable, Rights};
use crate::kernel::mm::user_space::UserSlice;
use crate::kernel::object::{
    self, Event, KernelObject, SignalWaitManyOutcome, SignalWaitOutcome, SignalWaitRequest,
};
use crate::kernel::process::{ProcessError, UserThreadPhase};
use alloc::vec::Vec;

struct ResolvedWaitEntry {
    object: ResolvedWaitable,
    signals: u64,
}

impl ObjectServices for DeferredProcessServices<'_> {
    fn current_process_id(&self) -> u64 {
        self.process.koid().get()
    }
    fn wait_set_create(&self, capacity: usize) -> Result<HandleValue, ObjectServiceError> {
        let set = object::WaitSet::try_new(capacity, &self.process.resource_domain())?;
        Ok(self.process.create_object(
            set,
            Rights::DUPLICATE
                .union(Rights::WAIT)
                .union(Rights::BIND_WAIT)
                .union(Rights::INSPECT),
        )?)
    }
    fn wait_set_add(
        &self,
        set: HandleValue,
        source: HandleValue,
        signals: u64,
    ) -> Result<u64, ObjectServiceError> {
        let set = self
            .process
            .resolve_handle::<object::WaitSet>(set, Rights::BIND_WAIT)?;
        let source = self.process.resolve_waitable(source, Rights::WAIT)?;
        Ok(set
            .object()
            .add(source, signals, &self.process.resource_domain())?)
    }
    fn wait_set_rearm(
        &self,
        set: HandleValue,
        registration: u64,
    ) -> Result<(), ObjectServiceError> {
        let set = self
            .process
            .resolve_handle::<object::WaitSet>(set, Rights::BIND_WAIT)?;
        Ok(set.object().rearm(registration)?)
    }
    fn wait_set_remove(
        &self,
        set: HandleValue,
        registration: u64,
    ) -> Result<(), ObjectServiceError> {
        let set = self
            .process
            .resolve_handle::<object::WaitSet>(set, Rights::BIND_WAIT)?;
        Ok(set.object().remove(registration)?)
    }
    fn wait_set_wait(
        &self,
        set: HandleValue,
        deadline: u64,
        output: UserSlice,
    ) -> Result<(), ObjectServiceError> {
        let set = self
            .process
            .resolve_handle::<object::WaitSet>(set, Rights::WAIT)?;
        let delivery = set
            .object()
            .wait(deadline, &self.process.resource_domain(), || {
                self.thread.snapshot().phase == UserThreadPhase::StopRequested
            })?;
        self.process.copy_to_user(output, &delivery.record())?;
        delivery.complete();
        Ok(())
    }

    fn create_event(&self) -> Result<HandleValue, ObjectServiceError> {
        let event = Event::try_new(&self.process.resource_domain())?;
        Ok(self
            .process
            .create_object(event, <Event as KernelObject>::SUPPORTED_RIGHTS)?)
    }

    fn signal_event(
        &self,
        value: HandleValue,
        clear: u64,
        set: u64,
    ) -> Result<(), ObjectServiceError> {
        let event = self
            .process
            .resolve_handle::<Event>(value, Rights::SIGNAL)?;
        event.object().signal(clear, set)?;
        Ok(())
    }

    fn wait_one(
        &self,
        value: HandleValue,
        requested: u64,
        deadline: u64,
    ) -> Result<SignalWaitOutcome, ObjectServiceError> {
        let resolved = self.process.resolve_waitable(value, Rights::WAIT)?;
        let domain = self.process.resource_domain();
        Ok(object::wait_one(
            resolved.source(),
            &domain,
            requested,
            deadline,
            || self.thread.snapshot().phase == UserThreadPhase::StopRequested,
        )?)
    }

    fn wait_many(
        &self,
        items: UserSlice,
        item_count: usize,
        deadline: u64,
    ) -> Result<SignalWaitManyOutcome, ObjectServiceError> {
        let record_size = core::mem::size_of::<hyper::abi::native::HyperNativeObjectWaitItem>();
        let input_bytes = item_count
            .checked_mul(record_size)
            .ok_or(ObjectServiceError::InvalidInput)?;
        if items.length() != u64::try_from(input_bytes).map_err(|_| ProcessError::Allocation)? {
            return Err(ObjectServiceError::InvalidInput);
        }
        let scratch_bytes = input_bytes
            .checked_add(
                item_count
                    .checked_mul(
                        core::mem::size_of::<ResolvedWaitEntry>()
                            + core::mem::size_of::<SignalWaitRequest<'_>>(),
                    )
                    .ok_or(ProcessError::Allocation)?,
            )
            .ok_or(ProcessError::Allocation)?;
        let _scratch_charge = self
            .process
            .resource_domain()
            .reserve(ResourceAmount::ZERO.with(
                ResourceKind::KernelMemoryBytes,
                u64::try_from(scratch_bytes).map_err(|_| ProcessError::Allocation)?,
            ))
            .map_err(ProcessError::from)?
            .commit();

        let mut encoded = Vec::new();
        encoded
            .try_reserve_exact(input_bytes)
            .map_err(|_| ProcessError::Allocation)?;
        encoded.resize(input_bytes, 0);
        self.process.copy_from_user(items, &mut encoded)?;

        let mut resolved = Vec::new();
        resolved
            .try_reserve_exact(item_count)
            .map_err(|_| ProcessError::Allocation)?;
        for record in encoded.chunks_exact(record_size) {
            let (raw_handle, signals) = decode_wait_item(record)?;
            let handle = HandleValue::try_from_raw(raw_handle).map_err(ProcessError::from)?;
            resolved.push(ResolvedWaitEntry {
                object: self.process.resolve_waitable(handle, Rights::WAIT)?,
                signals,
            });
        }

        let mut requests = Vec::new();
        requests
            .try_reserve_exact(item_count)
            .map_err(|_| ProcessError::Allocation)?;
        for entry in &resolved {
            requests.push(SignalWaitRequest::new(entry.object.source(), entry.signals));
        }
        let domain = self.process.resource_domain();
        Ok(object::wait_many(&requests, &domain, deadline, || {
            self.thread.snapshot().phase == UserThreadPhase::StopRequested
        })?)
    }
}

fn decode_wait_item(record: &[u8]) -> Result<(u64, u64), ObjectServiceError> {
    type AbiWaitItem = hyper::abi::native::HyperNativeObjectWaitItem;
    let handle = read_u64_field(record, core::mem::offset_of!(AbiWaitItem, handle))
        .ok_or(ObjectServiceError::InvalidInput)?;
    let signals = read_u64_field(record, core::mem::offset_of!(AbiWaitItem, signals))
        .ok_or(ObjectServiceError::InvalidInput)?;
    Ok((handle, signals))
}

fn read_u64_field(record: &[u8], offset: usize) -> Option<u64> {
    let end = offset.checked_add(core::mem::size_of::<u64>())?;
    let bytes: &[u8; core::mem::size_of::<u64>()] = record.get(offset..end)?.try_into().ok()?;
    Some(u64::from_ne_bytes(*bytes))
}
