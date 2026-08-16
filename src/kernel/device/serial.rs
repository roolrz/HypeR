//! Runtime ownership of the physical UART selected as the early console.

use hyper::drivers::platform::{DeviceScanner, PlatformDevice, ScanError};
use hyper::drivers::serial::{
    MmioAccess, Ns16550, Ns16550Error, Ns16550FifoTrigger, Pl011, ReceivedByte,
    pl011_registers as reg,
};
use hyper::hal::interrupt::{InterruptId, InterruptTrigger};
use hyper::platform::{
    ConsoleInfo, ConsoleKind, ConsoleRegisterAccess, PlatformInterruptTrigger, fdt,
};
use hyper::sync::InterruptSpinLock;
use hyper::sync::atomic::{AtomicU64, Ordering};

use crate::kernel::irq::interrupt::{self, HandlerResult, Registration, VirtualInterrupt};

type ConsoleLock = InterruptSpinLock<Option<RuntimeConsole>, crate::arch::LocalInterruptMask>;

const IRQ_PRIORITY: u8 = 0x80;
const MAX_DRAIN: usize = 32;

static HOST_CONSOLE: ConsoleLock = InterruptSpinLock::new(None);
static RECEIVE_ERRORS: AtomicU64 = AtomicU64::new(0);

struct RuntimeConsole {
    port: RuntimePort,
    _registration: Registration,
}

#[derive(Clone, Copy)]
enum RuntimePort {
    Pl011(Pl011),
    Ns16550(Ns16550),
}

#[derive(Clone, Copy)]
struct RuntimeReceivedByte {
    byte: u8,
    error: bool,
}

impl RuntimeReceivedByte {
    const EMPTY: Self = Self {
        byte: 0,
        error: false,
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Capabilities {
    pub driver: &'static str,
    pub hardware_interrupt: u32,
    pub virtual_interrupt: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    AlreadyInitialized,
    Fdt(fdt::Error),
    Interrupt(interrupt::Error),
    InvalidInterrupt,
    MissingDevice,
    Serial(Ns16550Error),
    Scan(ScanError),
}

impl From<interrupt::Error> for Error {
    fn from(error: interrupt::Error) -> Self {
        Self::Interrupt(error)
    }
}

impl From<Ns16550Error> for Error {
    fn from(error: Ns16550Error) -> Self {
        Self::Serial(error)
    }
}

pub fn initialize(
    boot: &super::super::boot::Initialization,
) -> Result<Option<Capabilities>, Error> {
    let Some(console) = boot.early_console() else {
        return Ok(None);
    };
    let device = discover(boot.linear_dtb(), console)?;
    let platform_interrupt = crate::arch::decode_platform_interrupt(device.interrupt_cells())
        .map_err(|_| Error::InvalidInterrupt)?;
    let trigger = match platform_interrupt.trigger {
        PlatformInterruptTrigger::Level => InterruptTrigger::Level,
        PlatformInterruptTrigger::Edge => InterruptTrigger::Edge,
    };
    let mapped_base =
        crate::kernel::mm::memory::mmio_address(console.base).ok_or(Error::MissingDevice)?;
    // SAFETY: Boot validated and permanently mapped the selected UART range.
    let port = unsafe { bind_runtime_port(console, mapped_base)? };
    install(port, platform_interrupt.interrupt, trigger, boot)
}

fn discover(linear_dtb: usize, console: ConsoleInfo) -> Result<PlatformDevice, Error> {
    let mut scanner = DeviceScanner::new(&[]);
    // SAFETY: The DTB is retained in the permanent linear mapping.
    unsafe { fdt::discover_with(linear_dtb, &mut scanner) }.map_err(Error::Fdt)?;
    let devices = scanner.finish().map_err(Error::Scan)?;
    devices
        .into_iter()
        .find(|device| {
            let compatible = match console.kind {
                ConsoleKind::Pl011 => device.is_compatible("arm,pl011"),
                ConsoleKind::Ns16550 => ["ns16550a", "ns16550", "uart8250", "snps,dw-apb-uart"]
                    .iter()
                    .any(|compatible| device.is_compatible(compatible)),
            };
            compatible
                && device.registers().first().map(|range| range.start()) == Some(console.base)
        })
        .ok_or(Error::MissingDevice)
}

unsafe fn bind_runtime_port(
    console: ConsoleInfo,
    mapped_base: usize,
) -> Result<RuntimePort, Error> {
    match console.kind {
        ConsoleKind::Pl011 => Ok(RuntimePort::Pl011(unsafe {
            Pl011::from_mmio_base(mapped_base)
        })),
        ConsoleKind::Ns16550 => {
            let access = match console.access {
                ConsoleRegisterAccess::Mmio8 { register_shift } => {
                    MmioAccess::Byte { register_shift }
                }
                ConsoleRegisterAccess::Mmio32 { register_shift } => {
                    MmioAccess::Word { register_shift }
                }
                ConsoleRegisterAccess::Native => MmioAccess::BYTE,
            };
            Ok(RuntimePort::Ns16550(unsafe {
                Ns16550::from_mmio(mapped_base, access)?
            }))
        }
    }
}

fn install(
    port: RuntimePort,
    hardware_interrupt: u32,
    trigger: InterruptTrigger,
    boot: &super::super::boot::Initialization,
) -> Result<Option<Capabilities>, Error> {
    if HOST_CONSOLE.with(|slot| slot.is_some()) {
        return Err(Error::AlreadyInitialized);
    }
    port.disable_interrupts();
    let interrupt_id = InterruptId::new(hardware_interrupt);
    let virtual_interrupt = interrupt::map(
        boot.interrupts().root_domain,
        interrupt_id,
        IRQ_PRIORITY,
        trigger,
    )?;
    let registration = match interrupt::register_shared(virtual_interrupt, 0, handle_interrupt) {
        Ok(registration) => registration,
        Err(error) => {
            let _ = interrupt::unmap(virtual_interrupt);
            return Err(error.into());
        }
    };
    HOST_CONSOLE.with(|slot| {
        *slot = Some(RuntimeConsole {
            port,
            _registration: registration,
        });
    });
    port.enable_runtime_input();
    Ok(Some(Capabilities {
        driver: port.name(),
        hardware_interrupt,
        virtual_interrupt: virtual_interrupt.get(),
    }))
}

fn handle_interrupt(_interrupt: VirtualInterrupt, _context: usize) -> HandlerResult {
    let mut received = [RuntimeReceivedByte::EMPTY; MAX_DRAIN];
    let (count, handled) = HOST_CONSOLE.with(|slot| {
        let Some(runtime) = slot.as_ref() else {
            return (0, false);
        };
        drain_interrupt(&runtime.port, &mut received)
    });
    if !handled {
        return HandlerResult::NotHandled;
    }
    for byte in &received[..count] {
        if byte.error {
            RECEIVE_ERRORS.fetch_add(1, Ordering::Relaxed);
        } else if crate::kernel::vm::receive_console_input(byte.byte).is_err() {
            return HandlerResult::HandledAndMaskLocal;
        }
    }
    HandlerResult::Handled
}

impl RuntimePort {
    const fn name(self) -> &'static str {
        match self {
            Self::Pl011(_) => "PL011",
            Self::Ns16550(_) => "NS16550",
        }
    }

    fn disable_interrupts(self) {
        match self {
            Self::Pl011(port) => port.disable_interrupts(),
            Self::Ns16550(port) => {
                port.set_interrupt_mask(hyper::drivers::serial::Ns16550InterruptMask::NONE)
            }
        }
    }

    fn enable_runtime_input(self) {
        match self {
            Self::Pl011(port) => port.enable_runtime_input(),
            Self::Ns16550(port) => port.enable_runtime_input(Ns16550FifoTrigger::OneByte),
        }
    }

    fn drain_interrupt(self, received: &mut [RuntimeReceivedByte]) -> (usize, bool) {
        match self {
            Self::Pl011(port) => drain_pl011(port, received),
            Self::Ns16550(port) => drain_ns16550(port, received),
        }
    }
}

fn drain_interrupt(port: &RuntimePort, received: &mut [RuntimeReceivedByte]) -> (usize, bool) {
    port.drain_interrupt(received)
}

fn drain_pl011(port: Pl011, received: &mut [RuntimeReceivedByte]) -> (usize, bool) {
    let status = port.masked_interrupt_status();
    if status.raw() == 0 {
        return (0, false);
    }
    let count = drain_received(received, || {
        port.try_read()
            .map(|byte: ReceivedByte| RuntimeReceivedByte {
                byte: byte.byte,
                error: byte.has_error(),
            })
    });
    if status.contains(reg::INT_ERROR_MASK) {
        port.clear_receive_errors();
    }
    port.clear_interrupts(status.raw() & (reg::INT_RT | reg::INT_ERROR_MASK));
    (count, true)
}

fn drain_ns16550(port: Ns16550, received: &mut [RuntimeReceivedByte]) -> (usize, bool) {
    if port.interrupt_identification()
        == hyper::drivers::serial::Ns16550InterruptIdentification::None
    {
        return (0, false);
    }
    let count = drain_received(received, || {
        port.try_read().map(|byte| RuntimeReceivedByte {
            byte: byte.byte,
            error: byte.has_error(),
        })
    });
    (count, true)
}

fn drain_received(
    received: &mut [RuntimeReceivedByte],
    mut next: impl FnMut() -> Option<RuntimeReceivedByte>,
) -> usize {
    let mut count = 0;
    while let Some(byte) = next() {
        let Some(slot) = received.get_mut(count) else {
            break;
        };
        *slot = byte;
        count += 1;
    }
    count
}
