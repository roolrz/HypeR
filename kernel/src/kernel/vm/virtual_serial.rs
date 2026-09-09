// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! VM input injection and registered userspace output pages.
//!
//! The runtime owns output retention and client transport. Kernel output is
//! only a bounded atomic publication window with no reader notification.

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
    _object_charge: CommittedCharge,
}

impl VirtualSerial {
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
    }

    /// Disconnects one VM generation; backing remains owned through retirement.
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
            self.output_closed
                .store(true, core::sync::atomic::Ordering::Release);
        }
    }

    /// Publishes output without allocation, locks, signals, or userspace copies.
    pub(crate) fn publish_guest_output(&self, byte: u8) {
        if !self
            .output_closed
            .load(core::sync::atomic::Ordering::Acquire)
            && let Some(output) = self.output.get()
        {
            output.publish(byte);
        }
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
        self.state.with(|state| state.input.pop_front())
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
        .union(Rights::ASSIGN_DEVICE);

    fn on_zero_active_handles(&self, _retirement: &mut crate::kernel::object::ObjectRetirement) {
        // Stop future publication when the runtime loses its last port handle.
        // An already admitted writer retains the VM device binding and pinned
        // VMO lease until it returns. Never free pages from this callback.
        self.output_closed
            .store(true, core::sync::atomic::Ordering::Release);
    }
}

#[cold]
fn serial_invariant(reason: &str) -> ! {
    crate::kernel::crash::fatal(format_args!(
        "HypeR: virtual serial invariant failed: {reason}"
    ))
}
