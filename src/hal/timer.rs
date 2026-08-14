#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PeriodicTimerProperties {
    pub counter_frequency_hz: u64,
    pub interval_ticks: u64,
}

/// Architecture policy for the kernel's per-CPU periodic tick source.
pub trait PeriodicTimer {
    type Error;

    fn start(ticks_per_second: u32) -> Result<PeriodicTimerProperties, Self::Error>;
    fn handle_interrupt() -> Result<(), Self::Error>;
    fn stop();
}
