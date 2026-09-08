// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Capability-scoped, buffered virtual serial ports.
//!
//! A port keeps guest output independently of physical-console ownership.
//! Userspace may therefore attach late and drain the retained prefix. The VM
//! device binding owns only an internal typed reference; manager policy owns
//! duplication and guarantees that at most one interactive client receives a
//! data-plane handle at a time.

use hyper::log::ByteRing;
use hyper::mm::AllocationError;
use hyper::sync::InterruptSpinLock;

use crate::kernel::accounting::{
    CommittedCharge, ResourceAmount, ResourceDomain, ResourceError, ResourceKind,
};
use crate::kernel::authority::Rights;
use crate::kernel::object::{
    KernelObject, ObjectKind, SignalMask, SignalSource, SignalState, TransferClass,
    object_allocation_size, private,
};
use crate::kernel::task::thread::ThreadId;

const OUTPUT_CAPACITY: usize = 64 * 1024;
const INPUT_CAPACITY: usize = 4 * 1024;
pub(crate) const TRANSFER_BATCH_BYTES: usize = 4 * 1024;

type PortLock<T> = InterruptSpinLock<T, crate::hal::irq::LocalMask>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    Allocation,
    AllocationSize,
    Disconnected,
    Resource(ResourceError),
    WouldBlock,
}

impl From<ResourceError> for Error {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Route {
    pub(crate) vm: super::registry::VmId,
    pub(crate) vcpu: u32,
    pub(crate) thread: ThreadId,
}

struct PortState {
    output: alloc::boxed::Box<ByteRing<OUTPUT_CAPACITY>>,
    input: alloc::boxed::Box<ByteRing<INPUT_CAPACITY>>,
    output_claim: Option<ClaimState>,
    next_claim_generation: u64,
    route: Option<Route>,
}

impl PortState {
    fn try_new() -> Result<Self, AllocationError> {
        Ok(Self {
            output: ByteRing::try_boxed()?,
            input: ByteRing::try_boxed()?,
            output_claim: None,
            next_claim_generation: 0,
            route: None,
        })
    }
}

#[derive(Clone, Copy)]
struct ClaimState {
    generation: u64,
    length: usize,
}

/// One virtual UART stream, independent of any physical console.
pub(crate) struct VirtualSerial {
    state: PortLock<PortState>,
    signals: SignalState,
    _object_charge: CommittedCharge,
}

impl VirtualSerial {
    pub(crate) const READABLE: SignalMask = SignalMask::from_trusted_bits(
        hyper::abi::native::HYPER_NATIVE_SIGNAL_VIRTUAL_SERIAL_READABLE,
    );
    pub(crate) const WRITABLE: SignalMask = SignalMask::from_trusted_bits(
        hyper::abi::native::HYPER_NATIVE_SIGNAL_VIRTUAL_SERIAL_WRITABLE,
    );
    pub(crate) const DISCONNECTED: SignalMask = SignalMask::from_trusted_bits(
        hyper::abi::native::HYPER_NATIVE_SIGNAL_VIRTUAL_SERIAL_DISCONNECTED,
    );
    pub(crate) const SUPPORTED_SIGNALS: SignalMask = Self::READABLE
        .union(Self::WRITABLE)
        .union(Self::DISCONNECTED);

    pub(crate) fn try_new(domain: &ResourceDomain) -> Result<Self, Error> {
        let bytes = object_allocation_size::<Self>()
            .and_then(|bytes| bytes.checked_add(core::mem::size_of::<ByteRing<OUTPUT_CAPACITY>>()))
            .and_then(|bytes| bytes.checked_add(core::mem::size_of::<ByteRing<INPUT_CAPACITY>>()))
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(Error::AllocationSize)?;
        let charge = domain
            .reserve(
                ResourceAmount::ZERO
                    .with(ResourceKind::KernelMemoryBytes, bytes)
                    .with(ResourceKind::KernelObjects, 1),
            )?
            .commit();
        Ok(Self {
            state: InterruptSpinLock::new(PortState::try_new().map_err(|_| Error::Allocation)?),
            signals: SignalState::with_initial_level(Self::DISCONNECTED),
            _object_charge: charge,
        })
    }

    /// Binds one installed VM generation to this port.
    pub(crate) fn bind(&self, route: Route) {
        self.state.with(|state| {
            if state.route.replace(route).is_some() {
                serial_invariant("port bound more than once")
            }
        });
        self.update_signals(Self::DISCONNECTED, Self::WRITABLE);
    }

    /// Disconnects exactly one VM generation without discarding guest output.
    pub(crate) fn disconnect(&self, vm: super::registry::VmId) {
        let disconnected = self.state.with(|state| {
            if state.route.is_some_and(|route| route.vm == vm) {
                state.route = None;
                state.input.clear();
                true
            } else {
                false
            }
        });
        if disconnected {
            self.update_signals(Self::WRITABLE, Self::DISCONNECTED);
        }
    }

    /// Retains one guest-produced byte without blocking the VM-exit path.
    pub(crate) fn publish_guest_output(&self, byte: u8) {
        let became_readable = self.state.with(|state| {
            let was_empty = state.output.is_empty();
            if !state.output.push(byte) && state.output_claim.is_none() {
                let _ = state.output.pop_front();
                if !state.output.push(byte) {
                    serial_invariant("output ring rejected space restored by eviction")
                }
            }
            was_empty && state.output_claim.is_none()
        });
        if became_readable {
            self.update_signals(SignalMask::EMPTY, Self::READABLE);
        }
    }

    /// Claims a readable output prefix until userspace copy commits.
    pub(crate) fn claim_output(&self, capacity: usize) -> Result<ReadClaim<'_>, Error> {
        if capacity == 0 {
            return Ok(ReadClaim::empty(self));
        }
        self.state.with(|state| {
            if state.output_claim.is_some() || state.output.is_empty() {
                return Err(if state.route.is_none() {
                    Error::Disconnected
                } else {
                    Error::WouldBlock
                });
            }
            let generation = state
                .next_claim_generation
                .checked_add(1)
                .unwrap_or_else(|| serial_invariant("read generation exhausted"));
            let mut claim = ReadClaim {
                port: self,
                generation,
                length: 0,
                bytes: [0; TRANSFER_BATCH_BYTES],
                active: true,
            };
            claim.length = state
                .output
                .peek_into(&mut claim.bytes[..capacity.min(TRANSFER_BATCH_BYTES)]);
            state.next_claim_generation = generation;
            state.output_claim = Some(ClaimState {
                generation,
                length: claim.length,
            });
            if let Err(error) = self.signals.update(Self::READABLE, SignalMask::EMPTY) {
                serial_signal_invariant(error)
            }
            Ok(claim)
        })
    }

    /// Enqueues host input and prompts the currently bound vCPU.
    pub(crate) fn write_input(&self, bytes: &[u8]) -> Result<usize, Error> {
        let (accepted, route, full) = self.state.with(|state| {
            let route = state.route.ok_or(Error::Disconnected)?;
            let mut accepted = 0usize;
            for byte in bytes.iter().copied() {
                if !state.input.push(byte) {
                    break;
                }
                accepted += 1;
            }
            Ok::<_, Error>((accepted, route, state.input.remaining_capacity() == 0))
        })?;
        if accepted == 0 && !bytes.is_empty() {
            return Err(Error::WouldBlock);
        }
        if full {
            self.update_signals(Self::WRITABLE, SignalMask::EMPTY);
        }
        super::device::kick_virtual_serial(route);
        Ok(accepted)
    }

    #[allow(
        dead_code,
        reason = "selected guest UART backends consume input only when they implement receive injection"
    )]
    pub(super) fn pop_guest_input(&self) -> Option<u8> {
        let byte = self.state.with(|state| state.input.pop_front());
        if byte.is_some() {
            self.update_signals(SignalMask::EMPTY, Self::WRITABLE);
        }
        byte
    }

    fn update_signals(&self, clear: SignalMask, set: SignalMask) {
        if let Err(error) = self.signals.update(clear, set) {
            serial_signal_invariant(error)
        }
    }
}

impl private::Sealed for VirtualSerial {}
impl private::UserExportable for VirtualSerial {}

impl KernelObject for VirtualSerial {
    const KIND: ObjectKind = ObjectKind::VIRTUAL_SERIAL;
    const TRANSFER_CLASS: TransferClass = TransferClass::Leaf;
    const SUPPORTED_RIGHTS: Rights = Rights::DUPLICATE
        .union(Rights::TRANSFER)
        .union(Rights::WAIT)
        .union(Rights::INSPECT)
        .union(Rights::READ)
        .union(Rights::WRITE)
        .union(Rights::ASSIGN_DEVICE);

    fn signal_source(&self) -> Option<SignalSource<'_>> {
        Some(SignalSource::new(&self.signals, Self::SUPPORTED_SIGNALS))
    }
}

#[must_use = "virtual serial output must be committed or aborted"]
pub(crate) struct ReadClaim<'port> {
    port: &'port VirtualSerial,
    generation: u64,
    length: usize,
    bytes: [u8; TRANSFER_BATCH_BYTES],
    active: bool,
}

impl<'port> ReadClaim<'port> {
    const fn empty(port: &'port VirtualSerial) -> Self {
        Self {
            port,
            generation: 0,
            length: 0,
            bytes: [0; TRANSFER_BATCH_BYTES],
            active: false,
        }
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes[..self.length]
    }

    pub(crate) fn commit(mut self) {
        if self.active {
            self.finish(true);
            self.active = false;
        }
    }

    fn finish(&self, consume: bool) {
        self.port.state.with(|state| {
            if state.output_claim.is_none_or(|claim| {
                claim.generation != self.generation || claim.length != self.length
            }) {
                serial_invariant("stale output claim")
            }
            if consume && !state.output.discard_front(self.length) {
                serial_invariant("claimed output disappeared")
            }
            state.output_claim = None;
            let readable = !state.output.is_empty();
            let (clear, set) = if readable {
                (SignalMask::EMPTY, VirtualSerial::READABLE)
            } else {
                (VirtualSerial::READABLE, SignalMask::EMPTY)
            };
            self.port.update_signals(clear, set);
        });
    }
}

impl Drop for ReadClaim<'_> {
    fn drop(&mut self) {
        if self.active {
            self.finish(false);
            self.active = false;
        }
    }
}

#[cold]
fn serial_signal_invariant(error: impl core::fmt::Debug) -> ! {
    crate::kernel::crash::fatal(format_args!(
        "HypeR: virtual serial signal invariant failed: {error:?}"
    ))
}

#[cold]
fn serial_invariant(reason: &str) -> ! {
    crate::kernel::crash::fatal(format_args!(
        "HypeR: virtual serial invariant failed: {reason}"
    ))
}
