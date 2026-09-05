// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Capability-scoped access to the host system console.
//!
//! The physical serial driver publishes received bytes from IRQ context. This
//! module owns the architecture-neutral FIFO, readiness signal, and the
//! transactional seam which prevents a failed user-memory copy from consuming
//! input. Hardware register ownership remains in [`super::serial`], while the
//! sole normal output writer remains in the kernel log service.

use hyper::log::ByteRing;
use hyper::sync::InterruptSpinLock;
#[cfg(not(feature = "kernel-self-test"))]
use hyper::sync::atomic::AtomicBool;
use hyper::sync::atomic::{AtomicU64, Ordering};

use crate::kernel::accounting::CommittedCharge;
#[cfg(not(feature = "kernel-self-test"))]
use crate::kernel::accounting::{ResourceAmount, ResourceDomain, ResourceError, ResourceKind};
use crate::kernel::authority::Rights;
use crate::kernel::object::{
    KernelObject, ObjectKind, SignalMask, SignalSource, SignalState, private,
};
#[cfg(not(feature = "kernel-self-test"))]
use crate::kernel::object::{ObjectCreationError, ObjectPublication, object_allocation_size};

type InputLock<T> = InterruptSpinLock<T, crate::hal::irq::LocalMask>;

const INPUT_CAPACITY: usize = 4096;
pub(crate) const TRANSFER_BATCH_BYTES: usize = 256;

static INPUT: InputLock<InputState> = InterruptSpinLock::new(InputState::new());
static SIGNALS: SignalState = SignalState::with_initial_level(SignalMask::from_trusted_bits(
    hyper::abi::native::HYPER_NATIVE_SIGNAL_CONSOLE_WRITABLE,
));
#[cfg(not(feature = "kernel-self-test"))]
static OBJECT_PUBLISHED: AtomicBool = AtomicBool::new(false);
static RECEIVE_ERRORS: AtomicU64 = AtomicU64::new(0);

struct InputState {
    bytes: ByteRing<INPUT_CAPACITY>,
    claim: Option<ClaimState>,
    next_generation: u64,
}

impl InputState {
    const fn new() -> Self {
        Self {
            bytes: ByteRing::new(),
            claim: None,
            next_generation: 0,
        }
    }

    fn publish_readable(&self, readable: bool) {
        let (clear, set) = if readable {
            (SignalMask::EMPTY, SystemConsole::READABLE)
        } else {
            (SystemConsole::READABLE, SignalMask::EMPTY)
        };
        if let Err(error) = SIGNALS.update(clear, set) {
            console_invariant("readiness signal update", error)
        }
    }
}

#[derive(Clone, Copy)]
struct ClaimState {
    generation: u64,
    length: usize,
}

/// Failure while constructing the singleton system-console object.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(not(feature = "kernel-self-test"))]
pub(crate) enum ObjectError {
    AllocationSize,
    AlreadyPublished,
    Object(ObjectCreationError),
    Resource(ResourceError),
    Unavailable,
}

#[cfg(not(feature = "kernel-self-test"))]
impl From<ObjectCreationError> for ObjectError {
    fn from(error: ObjectCreationError) -> Self {
        Self::Object(error)
    }
}

#[cfg(not(feature = "kernel-self-test"))]
impl From<ResourceError> for ObjectError {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}

/// Nonblocking console-I/O outcome before ABI status conversion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IoError {
    WouldBlock,
}

/// Userspace authority over the physical host console.
///
/// There is one exported identity. The object contains only its accounting
/// charge; the permanent device service owns the FIFO and readiness state so
/// IRQ delivery never depends on an active userspace handle.
pub(crate) struct SystemConsole {
    _object_charge: CommittedCharge,
}

impl SystemConsole {
    pub(crate) const READABLE: SignalMask =
        SignalMask::from_trusted_bits(hyper::abi::native::HYPER_NATIVE_SIGNAL_CONSOLE_READABLE);
    pub(crate) const WRITABLE: SignalMask =
        SignalMask::from_trusted_bits(hyper::abi::native::HYPER_NATIVE_SIGNAL_CONSOLE_WRITABLE);
    pub(crate) const SUPPORTED_SIGNALS: SignalMask = Self::READABLE.union(Self::WRITABLE);

    /// Reports whether boot selected a physical console with runtime input.
    #[cfg(not(feature = "kernel-self-test"))]
    pub(crate) fn is_available() -> bool {
        super::serial::runtime_input_available()
    }

    /// Prepares the system console's sole userspace object publication.
    #[cfg(not(feature = "kernel-self-test"))]
    pub(crate) fn try_publication(
        domain: &ResourceDomain,
    ) -> Result<ObjectPublication<Self>, ObjectError> {
        if !Self::is_available() {
            return Err(ObjectError::Unavailable);
        }
        if OBJECT_PUBLISHED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(ObjectError::AlreadyPublished);
        }
        let result = Self::try_new(domain)
            .and_then(|payload| ObjectPublication::try_new(payload).map_err(Into::into));
        if result.is_err() {
            OBJECT_PUBLISHED.store(false, Ordering::Release);
        }
        result
    }

    #[cfg(not(feature = "kernel-self-test"))]
    fn try_new(domain: &ResourceDomain) -> Result<Self, ObjectError> {
        let bytes = object_allocation_size::<Self>()
            .and_then(|value| u64::try_from(value).ok())
            .ok_or(ObjectError::AllocationSize)?;
        let charge = domain
            .reserve(
                ResourceAmount::ZERO
                    .with(ResourceKind::KernelObjects, 1)
                    .with(ResourceKind::KernelMemoryBytes, bytes),
            )?
            .commit();
        Ok(Self {
            _object_charge: charge,
        })
    }

    /// Claims a bounded, currently readable FIFO prefix without consuming it.
    pub(crate) fn claim_read(&self, capacity: usize) -> Result<ReadClaim, IoError> {
        if capacity == 0 {
            return Ok(ReadClaim::empty());
        }
        INPUT.with(|state| {
            if state.claim.is_some() || state.bytes.is_empty() {
                return Err(IoError::WouldBlock);
            }
            let generation = match state.next_generation.checked_add(1) {
                Some(generation) => generation,
                None => console_invariant("input claim generation", "exhausted"),
            };
            let mut claim = ReadClaim {
                generation,
                length: 0,
                bytes: [0; TRANSFER_BATCH_BYTES],
                active: true,
            };
            let limit = capacity.min(TRANSFER_BATCH_BYTES);
            claim.length = state.bytes.peek_into(&mut claim.bytes[..limit]);
            if claim.length == 0 {
                console_invariant("input claim", "empty FIFO produced an empty claim")
            }
            state.next_generation = generation;
            state.claim = Some(ClaimState {
                generation,
                length: claim.length,
            });
            // A claimed head is not available to another reader. Producers
            // retain later bytes behind it; commit or abort republishes the
            // accurate level while holding this same lock.
            state.publish_readable(false);
            Ok(claim)
        })
    }

    /// Enqueues as much output as the sole normal writer can currently retain.
    pub(crate) fn try_write(&self, bytes: &[u8]) -> Result<usize, IoError> {
        let accepted = crate::kernel::log::try_write_console(bytes);
        if accepted == 0 && !bytes.is_empty() {
            Err(IoError::WouldBlock)
        } else {
            Ok(accepted)
        }
    }
}

impl private::Sealed for SystemConsole {}
impl private::UserExportable for SystemConsole {}

impl KernelObject for SystemConsole {
    const KIND: ObjectKind = ObjectKind::CONSOLE;
    const SUPPORTED_RIGHTS: Rights = Rights::DUPLICATE
        .union(Rights::TRANSFER)
        .union(Rights::WAIT)
        .union(Rights::INSPECT)
        .union(Rights::READ)
        .union(Rights::WRITE);

    fn signal_source(&self) -> Option<SignalSource<'_>> {
        Some(SignalSource::new(&SIGNALS, Self::SUPPORTED_SIGNALS))
    }
}

/// Linear ownership of one unconsumed console-input prefix.
///
/// Dropping a claim restores readability. Only [`Self::commit`] removes its
/// bytes, after the caller has completed every fallible user-memory operation.
#[must_use = "console input must be committed after user copy or explicitly aborted"]
pub(crate) struct ReadClaim {
    generation: u64,
    length: usize,
    bytes: [u8; TRANSFER_BATCH_BYTES],
    active: bool,
}

impl ReadClaim {
    const fn empty() -> Self {
        Self {
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
        if !self.active {
            return;
        }
        INPUT.with(|state| {
            require_claim(state, self.generation, self.length);
            if !state.bytes.discard_front(self.length) {
                console_invariant("input claim commit", "retained prefix disappeared")
            }
            state.claim = None;
            state.publish_readable(!state.bytes.is_empty());
        });
        self.active = false;
    }

    fn abort(&mut self) {
        if !self.active {
            return;
        }
        INPUT.with(|state| {
            require_claim(state, self.generation, self.length);
            state.claim = None;
            state.publish_readable(!state.bytes.is_empty());
        });
        self.active = false;
    }
}

impl Drop for ReadClaim {
    fn drop(&mut self) {
        self.abort();
    }
}

/// Delivers one validated UART byte from IRQ context.
pub(super) fn receive(byte: u8) {
    INPUT.with(|state| {
        let was_available = state.claim.is_none() && !state.bytes.is_empty();
        if !state.bytes.push(byte) {
            RECEIVE_ERRORS.fetch_add(1, Ordering::Relaxed);
            return;
        }
        if !was_available && state.claim.is_none() {
            state.publish_readable(true);
        }
    });
}

/// Records one byte rejected by the physical UART's framing/error state.
pub(super) fn record_receive_error() {
    RECEIVE_ERRORS.fetch_add(1, Ordering::Relaxed);
}

/// Publishes whether the deferred output queue can accept at least one byte.
pub(crate) fn publish_writable(writable: bool) {
    let (clear, set) = if writable {
        (SignalMask::EMPTY, SystemConsole::WRITABLE)
    } else {
        (SystemConsole::WRITABLE, SignalMask::EMPTY)
    };
    if let Err(error) = SIGNALS.update(clear, set) {
        console_invariant("writability signal update", error)
    }
}

fn require_claim(state: &InputState, generation: u64, length: usize) {
    if state
        .claim
        .is_none_or(|claim| claim.generation != generation || claim.length != length)
    {
        console_invariant("input claim", "stale or duplicated transaction")
    }
}

#[cold]
fn console_invariant(operation: &str, error: impl core::fmt::Debug) -> ! {
    crate::kernel::crash::fatal(format_args!(
        "HypeR: system console {operation} invariant failed: {error:?}"
    ))
}
