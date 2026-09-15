// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native block initiators. Userspace negotiates a backend once; filesystem
//! requests use shared virtio-scsi queues and scheduler notifications directly.

pub(crate) mod service;
mod wire;

use crate::kernel::accounting::{CommittedCharge, ResourceDomain};
use crate::kernel::authority::Rights;
use crate::kernel::mm::user_space::GuestMemoryBacking;
use crate::kernel::object::{
    KernelObject, ObjectKind, ObjectRetirement, SignalMask, SignalSource, SignalState,
    SignalWaitOutcome, TransferClass, private, wait_one,
};
use crate::kernel::vm::io::Notification;
use core::sync::atomic::{AtomicU16, Ordering, fence};
use hyper::fs::block::{BlockDevice, Error, SECTOR_SIZE};
use hyper::mm::FallibleArc;
use hyper::sync::InterruptSpinLock;

struct State {
    busy: bool,
    failed: bool,
    sectors: u64,
    index: u16,
    tag: u64,
    readonly: bool,
    mounted: bool,
    capability_alive: bool,
}

pub(crate) struct Device {
    // Backend VM mappings independently retain this hardware lease. Disconnect
    // never recycles an in-flight slot: pages survive until VM/DMA retirement.
    memory: GuestMemoryBacking,
    guest_base: u64,
    notification: Notification,
    domain: ResourceDomain,
    state: InterruptSpinLock<State, crate::hal::irq::LocalMask>,
    available: SignalState,

    _charge: CommittedCharge,
}

impl Device {
    fn update_available(&self, state: &State) {
        let bits = u64::from(!state.busy || state.failed);
        if self
            .available
            .update(
                SignalMask::from_trusted_bits(1),
                SignalMask::from_trusted_bits(bits),
            )
            .is_err()
        {
            crate::kernel::crash::fatal(format_args!("block admission signal exhausted"));
        }
    }

    fn acquire(&self, require_active: bool) -> Result<Session<'_>, Error> {
        loop {
            let admitted = self.state.with(|state| {
                if state.failed {
                    return Err(Error::Disconnected);
                }
                if require_active && state.sectors == 0 {
                    return Err(Error::Disconnected);
                }
                if state.busy {
                    return Ok(false);
                }
                state.busy = true;
                self.update_available(state);
                Ok(true)
            })?;
            if admitted {
                return Ok(Session { device: self });
            }
            match wait_one(
                SignalSource::new(&self.available, SignalMask::from_trusted_bits(1)),
                &self.domain,
                1,
                u64::MAX,
                || false,
            )
            .map_err(|_| Error::Exhausted)?
            {
                SignalWaitOutcome::Observed(_) => {}
                _ => return Err(Error::Disconnected),
            }
        }
    }

    fn fail(&self) {
        self.state.with(|state| {
            state.failed = true;
            self.update_available(state);
        });
        self.notification.close_native();
    }

    fn read(&self, offset: u64, output: &mut [u8]) -> Result<(), Error> {
        self.memory
            .read_exposed(offset, output)
            .map_err(|_| Error::Io)
    }
    fn write(&self, offset: u64, input: &[u8]) -> Result<(), Error> {
        self.memory
            .write_exposed(offset, input)
            .map_err(|_| Error::Io)
    }
    fn index_word(&self, offset: u64) -> Result<&AtomicU16, Error> {
        if !offset.is_multiple_of(2) || offset + 2 > wire::MEMORY_BYTES {
            return Err(Error::InvalidRange);
        }
        let physical = self
            .memory
            .physical_page(offset & !4095)
            .map_err(|_| Error::Io)?;
        let address = crate::kernel::mm::memory::linear_address(physical.get()).ok_or(Error::Io)?;
        let address = address
            .checked_add((offset & 4095) as usize)
            .ok_or(Error::Io)?;
        // SAFETY: The retained hardware lease pins this initialized, aligned
        // word and its permanent linear mapping. Only the designated producer
        // stores each index, using an aligned 16-bit access. Other guest-owned
        // bytes are accessed through the HAL's external-memory copy contract.
        Ok(unsafe { &*core::ptr::with_exposed_provenance::<AtomicU16>(address) })
    }
    fn u16(&self, offset: u64) -> Result<u16, Error> {
        Ok(u16::from_le(
            self.index_word(offset)?.load(Ordering::Acquire),
        ))
    }

    fn activate(&self, readonly: bool) -> Result<u64, Error> {
        let session = self.acquire(false)?;
        if self.state.with(|state| state.sectors != 0) {
            return Err(Error::InvalidRange);
        }
        self.notification
            .control(1)
            .map_err(|_| Error::Disconnected)?;
        // A newly established SCSI nexus can report UNIT ATTENTION. Retrying a
        // read-only discovery command consumes it without replaying any writes.
        let mut capacity = [0; 32];
        let mut last = Err(Error::Io);
        for _ in 0..4 {
            last = session.command(&wire::capacity_cdb(), None, Some(&mut capacity));
            if last.is_ok() || matches!(last, Err(Error::Disconnected | Error::Corrupt)) {
                break;
            }
        }
        last?;
        let last_sector = u64::from_be_bytes(capacity[..8].try_into().map_err(|_| Error::Corrupt)?);
        let size = u32::from_be_bytes(capacity[8..12].try_into().map_err(|_| Error::Corrupt)?);
        if size != SECTOR_SIZE as u32 {
            self.fail();
            return Err(Error::Unsupported);
        }
        let sectors = last_sector
            .checked_add(1)
            .filter(|s| *s != 0)
            .ok_or(Error::Corrupt)?;
        self.state.with(|state| {
            state.sectors = sectors;
            state.readonly = readonly;
        });
        Ok(sectors)
    }
}

struct RequestFlight<'a> {
    device: &'a Device,
    retired: bool,
}
impl Drop for RequestFlight<'_> {
    fn drop(&mut self) {
        // Every error after available publication poisons the entire queue.
        // Never reuse its data buffer while an unacknowledged backend can DMA.
        if !self.retired {
            self.device.fail();
        }
    }
}

struct Session<'a> {
    device: &'a Device,
}
impl Drop for Session<'_> {
    fn drop(&mut self) {
        self.device.state.with(|state| {
            state.busy = false;
            self.device.update_available(state);
        });
    }
}
impl Session<'_> {
    fn command(
        &self,
        cdb: &[u8; 16],
        input: Option<&[u8]>,
        mut output: Option<&mut [u8]>,
    ) -> Result<(), Error> {
        let d = self.device;
        let length = input.map_or_else(|| output.as_ref().map_or(0, |b| b.len()), |b| b.len());
        if length > wire::DATA_BYTES || (input.is_some() && output.is_some()) {
            return Err(Error::InvalidRange);
        }
        let (index, tag) = d.state.with(|state| {
            if state.failed {
                return Err(Error::Disconnected);
            }
            state.tag = state.tag.checked_add(1).ok_or(Error::Exhausted)?;
            Ok((state.index, state.tag))
        })?;
        d.write(wire::REQUEST, &wire::request(tag, cdb))?;
        let mut empty_response = [0; wire::RESPONSE_BYTES];
        empty_response[11] = 0xff; // An unwritten response must never look successful.
        d.write(wire::RESPONSE, &empty_response)?;
        if let Some(bytes) = input {
            d.write(wire::DATA, bytes)?;
        }
        // Descriptor order follows virtio: all device-readable buffers precede
        // device-writable buffers. Only descriptor zero is published as a head.
        let response_id = if input.is_some() { 2 } else { 1 };
        d.write(
            wire::REQUEST_QUEUE,
            &wire::descriptor(d.guest_base + wire::REQUEST, 51, 1, 1),
        )?;
        if input.is_some() {
            d.write(
                wire::REQUEST_QUEUE + 16,
                &wire::descriptor(d.guest_base + wire::DATA, length as u32, 1, 2),
            )?;
        }
        d.write(
            wire::REQUEST_QUEUE + response_id * 16,
            &wire::descriptor(
                d.guest_base + wire::RESPONSE,
                wire::RESPONSE_BYTES as u32,
                if output.is_some() { 3 } else { 2 },
                2,
            ),
        )?;
        if output.is_some() {
            d.write(
                wire::REQUEST_QUEUE + 32,
                &wire::descriptor(d.guest_base + wire::DATA, length as u32, 2, 0),
            )?;
        }
        let slot = u64::from(index % wire::QUEUE_SIZE);
        d.write(wire::AVAILABLE + 4 + slot * 2, &0u16.to_le_bytes())?;
        // Coherent CPU-to-CPU virtqueue publication. Linux owns physical DMA
        // mapping/synchronization; this fence orders its CPU view of the ring.
        fence(Ordering::Release);
        let mut flight = RequestFlight {
            device: d,
            retired: false,
        };
        d.index_word(wire::AVAILABLE + 2)?
            .store(index.wrapping_add(1).to_le(), Ordering::Release);
        fence(Ordering::SeqCst);
        if d.notification.kick_native(2).is_err() {
            d.fail();
            return Err(Error::Disconnected);
        }
        let deadline = crate::kernel::time::monotonic_nanoseconds()
            .map_err(|_| Error::Io)?
            .checked_add(30_000_000_000)
            .ok_or(Error::Io)?;
        loop {
            let complete = d.u16(wire::USED + 2)? != index;
            let now = crate::kernel::time::monotonic_nanoseconds().map_err(|_| Error::Io)?;
            match wire::command_pending(complete, now, deadline) {
                Ok(false) => break,
                Ok(true) => {}
                Err(()) => return Err(Error::Disconnected),
            }
            // Clear the prompt, then recheck the durable used index before
            // registering a waiter. A concurrent completion cannot be lost.
            d.notification.acknowledge_native();
            fence(Ordering::SeqCst);
            if d.u16(wire::USED + 2)? != index {
                break;
            }
            let result = wait_one(
                d.notification.native_signal_source(),
                &d.domain,
                7,
                deadline,
                || false,
            );
            match result {
                Ok(SignalWaitOutcome::Observed(value)) if value.signals().bits() & 5 == 0 => {}
                _ => {
                    d.fail();
                    return Err(Error::Disconnected);
                }
            }
        }
        fence(Ordering::Acquire);
        if d.u16(wire::USED + 2)? != index.wrapping_add(1) {
            d.fail();
            return Err(Error::Corrupt);
        }
        let mut used = [0; 8];
        d.read(wire::USED + 4 + slot * 8, &mut used)?;
        let id = u32::from_le_bytes(used[..4].try_into().map_err(|_| Error::Corrupt)?);
        let written =
            u32::from_le_bytes(used[4..].try_into().map_err(|_| Error::Corrupt)?) as usize;
        let mut response = [0; wire::RESPONSE_BYTES];
        d.read(wire::RESPONSE, &mut response)?;
        let result = wire::validate_completion(
            index,
            d.u16(wire::USED + 2)?,
            id,
            written,
            &response,
            length,
            output.is_some(),
        );
        if result == Err(wire::CompletionError::Corrupt) {
            return Err(Error::Corrupt);
        }
        d.state.with(|state| state.index = index.wrapping_add(1));
        flight.retired = true;
        if result == Err(wire::CompletionError::Scsi) {
            return Err(Error::Io);
        }
        if let Some(bytes) = output.as_mut() {
            d.read(wire::DATA, bytes)?;
        }
        Ok(())
    }
}

pub(crate) struct NativeBlock {
    device: FallibleArc<Device>,
}
impl private::Sealed for NativeBlock {}
impl private::UserExportable for NativeBlock {}
impl KernelObject for NativeBlock {
    const KIND: ObjectKind = ObjectKind::NATIVE_BLOCK;
    const TRANSFER_CLASS: TransferClass = TransferClass::RendezvousOnly;
    const SUPPORTED_RIGHTS: Rights = Rights::WRITE
        .union(Rights::MAP)
        .union(Rights::TRANSFER)
        .union(Rights::INSPECT)
        .union(Rights::WAIT);
    fn signal_source(&self) -> Option<SignalSource<'_>> {
        Some(self.device.notification.closed_signal_source())
    }
    fn on_zero_active_handles(&self, _: &mut ObjectRetirement) {
        // A mounted filesystem is an independent owner; closing its setup
        // capability must not disconnect live filesystem requests.
        let retire = self.device.state.with(|state| {
            state.capability_alive = false;
            !state.mounted
        });
        if retire {
            self.device.fail();
        }
    }
}

pub(crate) struct MountedDevice {
    device: FallibleArc<Device>,
}
impl Drop for MountedDevice {
    fn drop(&mut self) {
        let retire = self.device.state.with(|state| {
            state.mounted = false;
            !state.capability_alive
        });
        if retire {
            self.device.fail();
        }
    }
}
impl BlockDevice for MountedDevice {
    fn is_read_only(&self) -> bool {
        self.device.state.with(|state| state.readonly)
    }
    fn sector_count(&self) -> u64 {
        self.device.state.with(|state| state.sectors)
    }
    fn read_sectors(&mut self, first: u64, output: &mut [u8]) -> Result<(), Error> {
        let session = self.device.acquire(true)?;
        check_range(first, output.len(), self.sector_count())?;
        let mut sector = first;
        for bytes in output.chunks_mut(wire::DATA_BYTES) {
            session.command(
                &wire::transfer_cdb(false, sector, (bytes.len() / SECTOR_SIZE) as u32),
                None,
                Some(bytes),
            )?;
            sector += (bytes.len() / SECTOR_SIZE) as u64;
        }
        Ok(())
    }
    fn write_sectors(&mut self, first: u64, input: &[u8]) -> Result<(), Error> {
        let session = self.device.acquire(true)?;
        if self.device.state.with(|state| state.readonly) {
            return Err(Error::ReadOnly);
        }
        check_range(first, input.len(), self.sector_count())?;
        let mut sector = first;
        for bytes in input.chunks(wire::DATA_BYTES) {
            session.command(
                &wire::transfer_cdb(true, sector, (bytes.len() / SECTOR_SIZE) as u32),
                Some(bytes),
                None,
            )?;
            sector += (bytes.len() / SECTOR_SIZE) as u64;
        }
        Ok(())
    }
    fn flush(&mut self) -> Result<(), Error> {
        let session = self.device.acquire(true)?;
        let mut cdb = [0; 16];
        cdb[0] = 0x91;
        session.command(&cdb, None, None)
    }
}
fn check_range(first: u64, bytes: usize, sectors: u64) -> Result<(), Error> {
    if !bytes.is_multiple_of(SECTOR_SIZE)
        || first
            .checked_add((bytes / SECTOR_SIZE) as u64)
            .is_none_or(|end| end > sectors)
    {
        return Err(Error::InvalidRange);
    }
    Ok(())
}
