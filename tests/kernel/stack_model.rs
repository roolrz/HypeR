// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Guard-page, watermark, thread-stack, and IRQ-stack integration tests.

use core::hint::{black_box, spin_loop};
use core::ptr::write_volatile;

use hyper::sync::atomic::{AtomicUsize, Ordering};

use crate::kernel::mm::stack;
use crate::kernel::sync::Semaphore;
use crate::kernel::task::scheduler;

static IRQ_CALLBACK_SP: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Error {
    GuardMapped,
    IrqCallbackMissing,
    IrqStackMismatch,
    PageAccounting,
    Scheduler(scheduler::Error),
    Stack(stack::Error),
    StackUsageMissing,
    Synchronization(crate::kernel::sync::Error),
}

impl From<scheduler::Error> for Error {
    fn from(error: scheduler::Error) -> Self {
        Self::Scheduler(error)
    }
}

impl From<stack::Error> for Error {
    fn from(error: stack::Error) -> Self {
        Self::Stack(error)
    }
}

impl From<crate::kernel::sync::Error> for Error {
    fn from(error: crate::kernel::sync::Error) -> Self {
        Self::Synchronization(error)
    }
}

struct ThreadProbe {
    ready: Semaphore,
    release: Semaphore,
}

impl ThreadProbe {
    const fn new() -> Self {
        Self {
            ready: Semaphore::new(0),
            release: Semaphore::new(0),
        }
    }
}

pub(super) fn run() -> Result<(), Error> {
    validate_cpu_exception_stacks()?;
    validate_thread_stack()?;
    validate_irq_stack_switch()?;
    Ok(())
}

fn validate_cpu_exception_stacks() -> Result<(), Error> {
    let cpu = crate::kernel::cpu::current_index().ok_or(Error::StackUsageMissing)?;
    let (irq, emergency) =
        stack::cpu_stack_statistics(cpu.get()).ok_or(Error::StackUsageMissing)?;
    for statistics in [irq, emergency] {
        if !statistics.canary_intact || !stack::guard_page_is_unmapped(statistics)? {
            return Err(Error::GuardMapped);
        }
    }
    Ok(())
}

fn validate_thread_stack() -> Result<(), Error> {
    let baseline_pages = crate::kernel::mm::statistics()
        .ok_or(Error::PageAccounting)?
        .runtime
        .kernel_pages
        .pages;
    let probe = ThreadProbe::new();
    let id = scheduler::kthread_create(
        "stack-watermark",
        stack_worker,
        (&probe as *const ThreadProbe) as usize,
    )?;
    scheduler::thread_ready(id)?;
    scheduler::yield_now()?;
    probe.ready.acquire()?;
    let statistics = scheduler::thread_stack_statistics(id)?.ok_or(Error::StackUsageMissing)?;
    let allocated_pages = crate::kernel::mm::statistics()
        .ok_or(Error::PageAccounting)?
        .runtime
        .kernel_pages
        .pages;
    let expected_pages =
        hyper::config::KERNEL_STACK_SIZE_KB as usize * 1024 / hyper::mm::PAGE_SIZE as usize;
    if allocated_pages != baseline_pages + expected_pages {
        return Err(Error::PageAccounting);
    }
    if statistics.used < 8 * 1024 || !statistics.canary_intact {
        return Err(Error::StackUsageMissing);
    }
    if !stack::guard_page_is_unmapped(statistics)? {
        return Err(Error::GuardMapped);
    }
    probe.release.release()?;
    scheduler::yield_now()?;
    scheduler::yield_now()?;
    let final_pages = crate::kernel::mm::statistics()
        .ok_or(Error::PageAccounting)?
        .runtime
        .kernel_pages
        .pages;
    if final_pages != baseline_pages {
        return Err(Error::PageAccounting);
    }
    Ok(())
}

extern "C" fn stack_worker(argument: usize) {
    // SAFETY: The parent retains the probe until this worker is released.
    let probe = unsafe { &*(argument as *const ThreadProbe) };
    let mut consumption = [0u8; 8 * 1024];
    for (index, byte) in consumption.iter_mut().enumerate() {
        // SAFETY: byte is one unique element in the local stack allocation.
        unsafe { write_volatile(byte, index as u8) };
    }
    black_box(&consumption);
    let _ = probe.ready.release();
    let _ = probe.release.acquire();
    black_box(&consumption);
}

fn validate_irq_stack_switch() -> Result<(), Error> {
    IRQ_CALLBACK_SP.store(0, Ordering::Release);
    crate::kernel::time::schedule_after(1, record_irq_stack, 0)
        .map_err(|_| Error::IrqCallbackMissing)?;
    for _ in 0..10_000_000 {
        if IRQ_CALLBACK_SP.load(Ordering::Acquire) != 0 {
            break;
        }
        spin_loop();
    }
    let pointer = IRQ_CALLBACK_SP.load(Ordering::Acquire);
    if pointer == 0 {
        return Err(Error::IrqCallbackMissing);
    }
    let cpu = crate::kernel::cpu::current_index().ok_or(Error::StackUsageMissing)?;
    let (irq, _) = stack::cpu_stack_statistics(cpu.get()).ok_or(Error::StackUsageMissing)?;
    if pointer < irq.bottom || pointer > irq.top || irq.used == 0 || !irq.canary_intact {
        return Err(Error::IrqStackMismatch);
    }
    Ok(())
}

fn record_irq_stack(_event: crate::kernel::time::TimerEvent, _context: usize) {
    let marker = 0usize;
    IRQ_CALLBACK_SP.store((&marker as *const usize) as usize, Ordering::Release);
}
