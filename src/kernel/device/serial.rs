//! Runtime ownership of the host PL011 selected as the early console.

use hyper::drivers::platform::{DeviceScanner, PlatformDevice, ScanError};
use hyper::drivers::serial::{Pl011, ReceivedByte, pl011_registers as reg};
use hyper::hal::interrupt::{InterruptId, InterruptTrigger};
use hyper::platform::{ConsoleKind, PlatformInterruptTrigger, fdt};
use hyper::sync::InterruptSpinLock;
use hyper::sync::atomic::{AtomicU64, Ordering};

use crate::kernel::irq::interrupt::{self, HandlerResult, Registration, VirtualInterrupt};

type ConsoleLock = InterruptSpinLock<Option<RuntimeConsole>, crate::arch::LocalInterruptMask>;

const IRQ_PRIORITY: u8 = 0x80;
const MAX_DRAIN: usize = 32;

static HOST_CONSOLE: ConsoleLock = InterruptSpinLock::new(None);
static RECEIVE_ERRORS: AtomicU64 = AtomicU64::new(0);

struct RuntimeConsole {
    port: Pl011,
    _registration: Registration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Capabilities {
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
    Scan(ScanError),
}

impl From<interrupt::Error> for Error {
    fn from(error: interrupt::Error) -> Self {
        Self::Interrupt(error)
    }
}

pub fn initialize(
    boot: &super::super::boot::Initialization,
) -> Result<Option<Capabilities>, Error> {
    let Some(console) = boot.early_console() else {
        return Ok(None);
    };
    if console.kind != ConsoleKind::Pl011 {
        return Ok(None);
    }
    let device = discover(boot.linear_dtb(), console.base)?;
    let platform_interrupt = crate::arch::decode_platform_interrupt(device.interrupt_cells())
        .map_err(|_| Error::InvalidInterrupt)?;
    let trigger = match platform_interrupt.trigger {
        PlatformInterruptTrigger::Level => InterruptTrigger::Level,
        PlatformInterruptTrigger::Edge => InterruptTrigger::Edge,
    };
    let mapped_base =
        crate::kernel::mm::memory::mmio_address(console.base).ok_or(Error::MissingDevice)?;
    // SAFETY: Boot validated and permanently mapped the selected PL011 range.
    let port = unsafe { Pl011::from_mmio_base(mapped_base) };
    install(port, platform_interrupt.interrupt, trigger, boot)
}

fn discover(linear_dtb: usize, base: u64) -> Result<PlatformDevice, Error> {
    let mut scanner = DeviceScanner::new(&[]);
    // SAFETY: The DTB is retained in the permanent linear mapping.
    unsafe { fdt::discover_with(linear_dtb, &mut scanner) }.map_err(Error::Fdt)?;
    let devices = scanner.finish().map_err(Error::Scan)?;
    devices
        .into_iter()
        .find(|device| {
            device.is_compatible("arm,pl011")
                && device.registers().first().map(|range| range.start()) == Some(base)
        })
        .ok_or(Error::MissingDevice)
}

fn install(
    port: Pl011,
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
        hardware_interrupt,
        virtual_interrupt: virtual_interrupt.get(),
    }))
}

fn handle_interrupt(_interrupt: VirtualInterrupt, _context: usize) -> HandlerResult {
    let mut received = [ReceivedByte::EMPTY; MAX_DRAIN];
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
        if byte.has_error() {
            RECEIVE_ERRORS.fetch_add(1, Ordering::Relaxed);
        } else if crate::kernel::vm::receive_console_input(byte.byte).is_err() {
            return HandlerResult::HandledAndMaskLocal;
        }
    }
    HandlerResult::Handled
}

fn drain_interrupt(port: &Pl011, received: &mut [ReceivedByte]) -> (usize, bool) {
    let status = port.masked_interrupt_status();
    if status.raw() == 0 {
        return (0, false);
    }
    let mut count = 0;
    while let Some(byte) = port.try_read() {
        if let Some(slot) = received.get_mut(count) {
            *slot = byte;
            count += 1;
        }
    }
    if status.contains(reg::INT_ERROR_MASK) {
        port.clear_receive_errors();
    }
    port.clear_interrupts(status.raw() & (reg::INT_RT | reg::INT_ERROR_MASK));
    (count, true)
}
