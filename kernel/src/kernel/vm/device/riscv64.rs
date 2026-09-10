// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! RISC-V reference UART and PLIC service.

use super::super::super::registry::{VmBinding, VmId};
use crate::kernel::time::{OwnedArmedReservedTimer, ReservedTimer};
use crate::kernel::vm::virtual_serial::Route;
use alloc::boxed::Box;
use hyper::abi::native::*;
use hyper::mm::{FallibleArc, try_box};
use hyper::sync::{InterruptSpinLock, PublishedOnce};
use hyper::vm::device::uart16550::Ns16550;
use hyper::vm::exit::{MmioAccess, MmioAction, MmioOperation};
use hyper::vm::interrupt::VirtualInterruptId;

const UART_BASE: u64 = HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE_UART_BASE;
const UART_SIZE: u64 = HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE_UART_SIZE;
const UART_CLOCK: u64 = HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE_UART_CLOCK_FREQUENCY;
const UART_INTERRUPT: VirtualInterruptId =
    VirtualInterruptId::new(HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE_UART_INTERRUPT as u32);
const PLIC_BASE: u64 = HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE_PLIC_BASE;
const PLIC_SIZE: u64 = HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE_PLIC_SIZE;
type Lock<T> = InterruptSpinLock<T, crate::hal::irq::LocalMask>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Allocation,
    Timer,
    InvalidAccess,
    InvalidInterrupt,
    Closed,
}

struct Console {
    uart: Ns16550,
    closed: bool,
}
struct TimerControl {
    armed: Option<OwnedArmedReservedTimer>,
    deadline: Option<u64>,
}
struct TimeoutContext {
    route: PublishedOnce<Route>,
}

pub(crate) struct VirtualDeviceSet {
    console: Lock<Console>,
    timer_control: Lock<TimerControl>,
    timer: FallibleArc<ReservedTimer>,
    timeout_context: Box<TimeoutContext>,
    virtual_serial: Option<super::super::VirtualSerialBinding>,
}

impl VirtualDeviceSet {
    pub(super) fn write_console_byte(&self, byte: u8) {
        if let Some(output) = &self.virtual_serial {
            output.write_byte(byte);
        }
    }

    pub(in crate::kernel::vm) fn bind_virtual_serial(
        &self,
        vm: VmId,
        vcpu: u32,
        thread: crate::kernel::task::thread::ThreadId,
    ) {
        let route = Route { vm, vcpu, thread };
        if self.timeout_context.route.publish(route).is_err() {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: duplicate UART timer route publication"
            ));
        }
        if let Some(output) = &self.virtual_serial {
            output.bind(route);
        }
    }

    pub(in crate::kernel::vm) fn disconnect_virtual_serial(&self, vm: VmId) {
        if let Some(output) = &self.virtual_serial {
            output.disconnect(vm);
        }
    }

    /// Called after registry lookup is cut, before ownership reclamation.
    pub(in crate::kernel::vm) fn quiesce(&self) -> Result<(), Error> {
        self.timer_control.with(|control| {
            self.console.with(|console| console.closed = true);
            control.deadline = None;
            // Never wait for a callback under the console or PLIC lock.
            if let Some(armed) = control.armed.take() {
                armed.retire().map_err(|_| Error::Timer)?;
            }
            Ok(())
        })
    }

    fn reschedule(&self) -> Result<(), Error> {
        // Lock order: timer control -> console -> PLIC. Callback takes only
        // console -> PLIC, and never timer control. Exact retirement waits only
        // after console unlock, so an in-flight callback can always finish.
        self.timer_control.with(|control| {
            let deadline = self.console.with(|console| {
                if console.closed {
                    None
                } else {
                    console.uart.next_timeout()
                }
            });
            if deadline == control.deadline {
                return Ok(());
            }
            if let Some(armed) = control.armed.take() {
                armed.retire().map_err(|_| Error::Timer)?;
            }
            control.deadline = None;
            if let Some(deadline) = deadline {
                if self.timeout_context.route.get().is_none() {
                    return Err(Error::Closed);
                }
                let context = (&*self.timeout_context as *const TimeoutContext).expose_provenance();
                control.armed = Some(
                    ReservedTimer::arm_owned(self.timer.clone(), deadline, uart_timeout, context)
                        .map_err(|_| Error::Timer)?,
                );
                control.deadline = Some(deadline);
            }
            Ok(())
        })
    }

    fn access(
        &self,
        access: MmioAccess,
        update: impl FnOnce(VirtualInterruptId, bool) -> Result<(), Error>,
    ) -> Result<Option<Option<u64>>, Error> {
        let Some(offset) = access
            .address()
            .get()
            .checked_sub(UART_BASE)
            .filter(|offset| *offset < UART_SIZE)
        else {
            return Ok(None);
        };
        if access.size() != 1 {
            return Err(Error::InvalidAccess);
        }
        let (value, transmitted) = self.console.with(|console| {
            let now = crate::kernel::time::monotonic_ticks();
            if console.closed {
                return Err(Error::Closed);
            }
            pump_input(&mut console.uart, self.virtual_serial.as_ref(), now);
            let (value, transmitted) = match access.operation() {
                MmioOperation::Read => (
                    Some(u64::from(console.uart.read_at(offset as usize, now))),
                    None,
                ),
                MmioOperation::Write(value) => (
                    None,
                    console.uart.write_at(offset as usize, value as u8, now),
                ),
            };
            pump_input(&mut console.uart, self.virtual_serial.as_ref(), now);
            update(UART_INTERRUPT, console.uart.interrupt_asserted())?;
            Ok((value, transmitted))
        })?;
        if let Some(byte) = transmitted {
            self.write_console_byte(byte);
        }
        self.reschedule()?;
        Ok(Some(value))
    }

    fn receive(&self, binding: &VmBinding, route: Route, timeout_only: bool) -> Result<(), Error> {
        self.console.with(|console| {
            let now = crate::kernel::time::monotonic_ticks();
            if console.closed {
                return Ok(());
            }
            if timeout_only {
                console.uart.advance(now);
            } else {
                pump_input(&mut console.uart, self.virtual_serial.as_ref(), now);
            }
            crate::hal::vm::update_saved_guest_device_interrupt(
                binding.interrupts(),
                route.vcpu,
                UART_INTERRUPT,
                console.uart.interrupt_asserted(),
            )
            .map_err(|_| Error::InvalidInterrupt)
        })?;
        if !timeout_only {
            self.reschedule()?;
        }
        binding
            .publish_interrupt_reconcile(route.vcpu, route.thread)
            .map_err(|_| Error::InvalidInterrupt)
    }
}

fn pump_input(uart: &mut Ns16550, binding: Option<&super::super::VirtualSerialBinding>, now: u64) {
    uart.advance(now);
    let Some(binding) = binding else {
        return;
    };
    while uart.can_receive_external() {
        let Some(byte) = binding.pop_guest_input() else {
            break;
        };
        let _ = uart.receive_external(byte, now);
    }
}

fn uart_timeout(context: usize) {
    let pointer = core::ptr::with_exposed_provenance::<TimeoutContext>(context);
    // SAFETY: the device owns this stable box. Quiesce retires its exact timer
    // (including an in-flight callback) before the box can be destroyed.
    let context = unsafe { &*pointer };
    if let Some(route) = context.route.get().copied() {
        let _ = super::super::super::registry::with_binding(route.vm, |binding| {
            binding.devices().receive(binding, route, true)
        });
    }
}

pub(super) fn prepare(
    virtual_serial: Option<super::super::VirtualSerialBinding>,
) -> Result<VirtualDeviceSet, Error> {
    let frequency = crate::kernel::time::counter_frequency_hz().map_err(|_| Error::Timer)?;
    let timer = FallibleArc::try_new(ReservedTimer::try_new().map_err(|_| Error::Timer)?)
        .map_err(|_| Error::Allocation)?;
    let timeout_context = try_box(TimeoutContext {
        route: PublishedOnce::new(),
    })
    .map_err(|_| Error::Allocation)?;
    Ok(VirtualDeviceSet {
        console: Lock::new(Console {
            uart: Ns16550::with_clock(frequency, UART_CLOCK),
            closed: false,
        }),
        timer_control: Lock::new(TimerControl {
            armed: None,
            deadline: None,
        }),
        timer,
        timeout_context,
        virtual_serial,
    })
}

pub(super) const fn dynamic_allocation_bytes() -> usize {
    FallibleArc::<ReservedTimer>::allocation_size()
        + ReservedTimer::allocation_size()
        + core::mem::size_of::<TimeoutContext>()
}
pub(super) const fn timer_count() -> u64 {
    1
}

pub(super) fn supports_configuration(profile: u32, memory_base: u64, memory_size: u64) -> bool {
    profile == hyper::abi::native::HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE as u32
        && memory_base == HYPER_NATIVE_VIRTUAL_PLATFORM_RISCV64_REFERENCE_GUEST_RAM_BASE
        && memory_size != 0
        && memory_base.checked_add(memory_size).is_some()
}
pub(super) const fn default_timer_interrupt() -> VirtualInterruptId {
    VirtualInterruptId::new(5)
}

#[must_use]
pub(in crate::kernel) struct MmioDispatch {
    action: MmioAction,
}
impl MmioDispatch {
    pub(in crate::kernel) const fn into_action(self) -> MmioAction {
        self.action
    }
}

pub(super) fn dispatch_mmio(
    execution: &mut crate::kernel::vm::vcpu::VcpuExecution,
    access: MmioAccess,
) -> MmioDispatch {
    let Some((action, report)) = (|| {
        let (binding, hardware, vcpu, interrupts) = execution.device_context()?;
        let result = binding.devices().access(access, |interrupt, asserted| {
            crate::hal::vm::update_guest_device_interrupt(
                hardware, vcpu, interrupts, interrupt, asserted,
            )
            .map_err(|_| Error::InvalidInterrupt)
        });
        let result = match result {
            Ok(None) => match access
                .address()
                .get()
                .checked_sub(PLIC_BASE)
                .filter(|offset| *offset < PLIC_SIZE)
            {
                Some(offset) => crate::hal::vm::access_plic(
                    hardware,
                    interrupts,
                    vcpu,
                    offset,
                    access.size(),
                    access.operation(),
                )
                .map(Some)
                .map_err(|_| Error::InvalidAccess),
                None => Ok(None),
            },
            result => result,
        };
        Some(match result {
            Ok(Some(Some(value))) => (MmioAction::CompleteRead(value), None),
            Ok(Some(None)) => (MmioAction::CompleteWrite, None),
            Ok(None) => (
                MmioAction::Unhandled,
                binding.admit_unhandled_mmio(vcpu, access),
            ),
            Err(_) => (MmioAction::Stop, None),
        })
    })() else {
        return MmioDispatch {
            action: MmioAction::Stop,
        };
    };
    if let Some(report) = report
        && execution.publish_terminal_mmio_report(report).is_err()
    {
        crate::kernel::crash::fatal(format_args!("HypeR: duplicate terminal RISC-V MMIO report"));
    }
    MmioDispatch { action }
}

pub(super) fn kick_virtual_serial(route: Route) {
    let delivery = super::super::super::registry::with_binding(route.vm, |binding| {
        binding.devices().receive(binding, route, false)
    });
    if !matches!(delivery, Ok(Ok(()))) {
        let _ = super::super::super::registry::with_binding(route.vm, |binding| {
            binding.devices().disconnect_virtual_serial(route.vm)
        });
    }
}
