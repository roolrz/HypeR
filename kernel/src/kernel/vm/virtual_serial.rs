// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! VM input injection and registered userspace output pages.
//!
//! The runtime owns output retention and client transport. Kernel output is
//! a read-only userspace ring with coalesced, level-triggered notification.

use hyper::log::ByteRing;
use hyper::mm::AllocationError;
use hyper::sync::InterruptSpinLock;

use crate::kernel::accounting::{
    CommittedCharge, ResourceAmount, ResourceDomain, ResourceError, ResourceKind,
};
use crate::kernel::authority::Rights;
use crate::kernel::object::{
    KernelObject, ObjectKind, TransferClass, object_allocation_size, private,
};
use crate::kernel::task::thread::ThreadId;

const INPUT_CAPACITY: usize = 4 * 1024;
pub(crate) const TRANSFER_BATCH_BYTES: usize = 4 * 1024;

type PortLock<T> = InterruptSpinLock<T, crate::hal::irq::LocalMask>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    Allocation,
    AllocationSize,
    Disconnected,
    Resource(ResourceError),
    Memory(crate::kernel::mm::user_space::MemoryObjectError),
    WouldBlock,
    InvalidCursor,
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
    input: alloc::boxed::Box<ByteRing<INPUT_CAPACITY>>,
    route: Option<Route>,
}

impl PortState {
    fn try_new() -> Result<Self, AllocationError> {
        Ok(Self {
            input: ByteRing::try_boxed()?,
            route: None,
        })
    }
}

/// One virtual UART stream, independent of any physical console.
pub(crate) struct VirtualSerial {
    state: PortLock<PortState>,
    output: hyper::sync::PublishedOnce<super::serial_output::SharedAtomicOutput>,
    assigned: core::sync::atomic::AtomicBool,
    registering: core::sync::atomic::AtomicBool,
    output_closed: core::sync::atomic::AtomicBool,
    signals: crate::kernel::object::SignalState,
    _object_charge: CommittedCharge,
}

impl VirtualSerial {
    const READABLE: crate::kernel::object::SignalMask =
        crate::kernel::object::SignalMask::from_trusted_bits(
            hyper::abi::native::HYPER_NATIVE_SIGNAL_VIRTUAL_SERIAL_READABLE,
        );
    const WRITABLE: crate::kernel::object::SignalMask =
        crate::kernel::object::SignalMask::from_trusted_bits(
            hyper::abi::native::HYPER_NATIVE_SIGNAL_VIRTUAL_SERIAL_WRITABLE,
        );
    const CLOSED: crate::kernel::object::SignalMask =
        crate::kernel::object::SignalMask::from_trusted_bits(
            hyper::abi::native::HYPER_NATIVE_SIGNAL_VIRTUAL_SERIAL_PEER_CLOSED,
        );

    // Lock order: port -> signal state -> scheduler. Signal observation never
    // takes the port lock. Device kicks occur only after releasing it.
    fn update_signals(
        &self,
        clear: crate::kernel::object::SignalMask,
        set: crate::kernel::object::SignalMask,
    ) {
        if self.signals.update(clear, set).is_err() {
            serial_invariant("signal publication failed")
        }
    }

    pub(crate) fn try_new(domain: &ResourceDomain) -> Result<Self, Error> {
        let bytes = object_allocation_size::<Self>()
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
            assigned: core::sync::atomic::AtomicBool::new(false),
            registering: core::sync::atomic::AtomicBool::new(false),
            output_closed: core::sync::atomic::AtomicBool::new(false),
            output: hyper::sync::PublishedOnce::new(),
            state: InterruptSpinLock::new(PortState::try_new().map_err(|_| Error::Allocation)?),
            signals: crate::kernel::object::SignalState::new(),
            _object_charge: charge,
        })
    }

    #[cfg(feature = "kernel-self-test")]
    pub(crate) fn observed_signals(&self) -> u64 {
        self.signals
            .observe(Self::READABLE.union(Self::WRITABLE).union(Self::CLOSED))
            .map_or(0, |snapshot| snapshot.signals().bits())
    }

    /// Binds one installed VM generation to this port.
    pub(crate) fn bind(&self, route: Route) {
        self.state.with(|state| {
            if self
                .output_closed
                .load(core::sync::atomic::Ordering::Acquire)
            {
                return;
            }
            if state.route.replace(route).is_some() {
                serial_invariant("port bound more than once")
            }
            self.update_signals(crate::kernel::object::SignalMask::EMPTY, Self::WRITABLE);
        });
    }

    /// Disconnects one VM generation; backing remains owned through retirement.
    pub(crate) fn disconnect(&self, vm: super::registry::VmId) {
        self.state.with(|state| {
            if state.route.is_some_and(|route| route.vm == vm) {
                state.route = None;
                state.input.clear();
                self.close_output(state);
            }
        });
    }

    // Called under the port lock, which also excludes every admitted writer.
    fn close_output(&self, state: &mut PortState) {
        state.route = None;
        state.input.clear();
        self.output_closed
            .store(true, core::sync::atomic::Ordering::Release);
        self.update_signals(Self::WRITABLE, Self::CLOSED);
    }

    /// No allocation or data copy into a kernel queue. Only the first byte of
    /// an unread batch publishes readiness; subsequent bytes do not notify.
    pub(crate) fn publish_guest_output(&self, byte: u8) {
        self.state.with(|_| {
            if !self
                .output_closed
                .load(core::sync::atomic::Ordering::Acquire)
                && let Some(output) = self.output.get()
                && output.publish(byte)
            {
                self.update_signals(crate::kernel::object::SignalMask::EMPTY, Self::READABLE);
            }
        });
    }

    pub(crate) fn acknowledge_output(&self, consumed: u64) -> Result<(), Error> {
        self.state.with(|_| {
            let output = self.output.get().ok_or(Error::Disconnected)?;
            let readable = output
                .acknowledge(consumed)
                .map_err(|()| Error::InvalidCursor)?;
            // Publication and clearing share this lock: data arriving before
            // acknowledgement remains readable; later data sets it again.
            if !readable {
                self.update_signals(Self::READABLE, crate::kernel::object::SignalMask::EMPTY);
            }
            Ok(())
        })
    }

    pub(crate) fn register_output(
        &self,
        buffer: &crate::kernel::mm::user_space::VmoObject,
        domain: &ResourceDomain,
    ) -> Result<(), Error> {
        if self
            .registering
            .swap(true, core::sync::atomic::Ordering::AcqRel)
        {
            return Err(Error::WouldBlock);
        }
        let output = match super::serial_output::SharedAtomicOutput::try_register(buffer, domain) {
            Ok(output) => output,
            Err(error) => {
                self.registering
                    .store(false, core::sync::atomic::Ordering::Release);
                return Err(Error::Memory(error));
            }
        };
        self.output.publish(output).map_err(|_| Error::WouldBlock)
    }

    pub(crate) fn claim_assignment(&self) -> bool {
        self.output.get().is_some()
            && !self
                .assigned
                .swap(true, core::sync::atomic::Ordering::AcqRel)
    }

    /// Enqueues host input and prompts the currently bound vCPU.
    pub(crate) fn write_input(&self, bytes: &[u8]) -> Result<usize, Error> {
        let (accepted, route) = self.state.with(|state| {
            let route = state.route.ok_or(Error::Disconnected)?;
            let mut accepted = 0usize;
            for byte in bytes.iter().copied() {
                if !state.input.push(byte) {
                    break;
                }
                accepted += 1;
            }
            if state.input.remaining_capacity() == 0 {
                self.update_signals(Self::WRITABLE, crate::kernel::object::SignalMask::EMPTY);
            }
            Ok::<_, Error>((accepted, route))
        })?;
        if accepted == 0 && !bytes.is_empty() {
            return Err(Error::WouldBlock);
        }
        super::device::kick_virtual_serial(route);
        Ok(accepted)
    }

    #[allow(
        dead_code,
        reason = "selected guest UART backends consume input only when they implement receive injection"
    )]
    pub(super) fn pop_guest_input(&self) -> Option<u8> {
        self.state.with(|state| {
            let was_full = state.input.remaining_capacity() == 0;
            let byte = state.input.pop_front();
            if was_full && state.route.is_some() {
                self.update_signals(crate::kernel::object::SignalMask::EMPTY, Self::WRITABLE);
            }
            byte
        })
    }
}

impl private::Sealed for VirtualSerial {}
impl private::UserExportable for VirtualSerial {}

impl KernelObject for VirtualSerial {
    const KIND: ObjectKind = ObjectKind::VIRTUAL_SERIAL;
    const TRANSFER_CLASS: TransferClass = TransferClass::Never;
    const SUPPORTED_RIGHTS: Rights = Rights::DUPLICATE
        .union(Rights::TRANSFER)
        .union(Rights::INSPECT)
        .union(Rights::WRITE)
        .union(Rights::READ)
        .union(Rights::WAIT)
        .union(Rights::ASSIGN_DEVICE);

    fn signal_source(&self) -> Option<crate::kernel::object::SignalSource<'_>> {
        Some(crate::kernel::object::SignalSource::new(
            &self.signals,
            Self::READABLE.union(Self::WRITABLE).union(Self::CLOSED),
        ))
    }

    fn on_zero_active_handles(&self, _retirement: &mut crate::kernel::object::ObjectRetirement) {
        // Synchronize with admitted writers before closing. The independent
        // VMO lease survives until the final VM/object reference retires.
        self.state.with(|state| self.close_output(state));
    }
}

#[cold]
fn serial_invariant(reason: &str) -> ! {
    crate::kernel::crash::fatal(format_args!(
        "HypeR: virtual serial invariant failed: {reason}"
    ))
}
