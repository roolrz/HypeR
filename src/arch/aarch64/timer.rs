use core::arch::asm;

use hyper::hal::timer::{PeriodicTimer, PeriodicTimerProperties};
use hyper::sync::atomic::{AtomicU64, Ordering};

const TIMER_ENABLE: u64 = 1 << 0;
const TIMER_MASK: u64 = 1 << 1;

const MAX_CPUS: usize = hyper::config::MAX_CPUS as usize;

static INTERVAL: [AtomicU64; MAX_CPUS] = [const { AtomicU64::new(0) }; MAX_CPUS];
static NEXT_DEADLINE: [AtomicU64; MAX_CPUS] = [const { AtomicU64::new(0) }; MAX_CPUS];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidFrequency,
    InvalidCpuIndex,
    NotStarted,
}

/// EL2 physical timer backed by CNTHP_EL2 system registers.
pub struct El2PhysicalTimer;

impl PeriodicTimer for El2PhysicalTimer {
    type Error = Error;

    fn start(ticks_per_second: u32) -> Result<PeriodicTimerProperties, Self::Error> {
        let cpu = current_cpu()?;
        let frequency = counter_frequency();
        if frequency == 0 || ticks_per_second == 0 {
            return Err(Error::InvalidFrequency);
        }
        let interval = frequency / u64::from(ticks_per_second);
        if interval == 0 {
            return Err(Error::InvalidFrequency);
        }
        let deadline = physical_count().wrapping_add(interval);
        INTERVAL[cpu].store(interval, Ordering::Release);
        NEXT_DEADLINE[cpu].store(deadline, Ordering::Release);
        write_deadline(deadline);
        write_control(TIMER_ENABLE);
        Ok(PeriodicTimerProperties {
            counter_frequency_hz: frequency,
            interval_ticks: interval,
        })
    }

    fn handle_interrupt() -> Result<(), Self::Error> {
        let cpu = current_cpu()?;
        let interval = INTERVAL[cpu].load(Ordering::Acquire);
        if interval == 0 {
            return Err(Error::NotStarted);
        }
        let previous = NEXT_DEADLINE[cpu].load(Ordering::Relaxed);
        let now = physical_count();
        let elapsed = now.wrapping_sub(previous);
        let periods = if now >= previous {
            elapsed / interval + 1
        } else {
            1
        };
        let deadline = previous.wrapping_add(interval.wrapping_mul(periods));
        NEXT_DEADLINE[cpu].store(deadline, Ordering::Relaxed);
        write_deadline(deadline);
        Ok(())
    }

    fn stop() {
        write_control(TIMER_ENABLE | TIMER_MASK);
        if let Ok(cpu) = current_cpu() {
            INTERVAL[cpu].store(0, Ordering::Release);
        }
    }
}

fn current_cpu() -> Result<usize, Error> {
    let cpu = super::current_cpu_index();
    (cpu < MAX_CPUS)
        .then_some(cpu)
        .ok_or(Error::InvalidCpuIndex)
}

fn counter_frequency() -> u64 {
    let frequency: u64;
    // SAFETY: CNTFRQ_EL0 is readable at EL2 and has no side effects.
    unsafe {
        asm!(
            "mrs {frequency}, CNTFRQ_EL0",
            frequency = out(reg) frequency,
            options(nomem, nostack, preserves_flags)
        );
    }
    frequency
}

fn physical_count() -> u64 {
    let count: u64;
    // SAFETY: CNTPCT_EL0 is readable at EL2 and has no side effects.
    unsafe {
        asm!(
            "mrs {count}, CNTPCT_EL0",
            count = out(reg) count,
            options(nomem, nostack, preserves_flags)
        );
    }
    count
}

fn write_deadline(deadline: u64) {
    // SAFETY: CNTHP_CVAL_EL2 controls only the current CPU's EL2 timer.
    unsafe {
        asm!(
            "msr CNTHP_CVAL_EL2, {deadline}",
            "isb",
            deadline = in(reg) deadline,
            options(nomem, nostack, preserves_flags)
        );
    }
}

fn write_control(control: u64) {
    // SAFETY: CNTHP_CTL_EL2 controls only the current CPU's EL2 timer.
    unsafe {
        asm!(
            "msr CNTHP_CTL_EL2, {control}",
            "isb",
            control = in(reg) control,
            options(nomem, nostack, preserves_flags)
        );
    }
}
