//! Kernel device-service orchestration.

pub mod cpu_power;
mod platform_bus;
mod serial;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InitializationError {
    CpuPower(cpu_power::Error),
    DriverFramework(platform_bus::Error),
    MissingCpuPower,
}

/// Selects and activates the firmware CPU power-management interface.
pub(crate) fn initialize_cpu_power(
    boot: &super::boot::Initialization,
) -> Result<(), InitializationError> {
    let info = boot
        .essential()
        .cpu_power()
        .ok_or(InitializationError::MissingCpuPower)?;
    let capabilities = cpu_power::initialize(info).map_err(InitializationError::CpuPower)?;
    crate::println!(
        "HypeR: CPU power interface version {}.{}: on={}, off={}, suspend={}, reset={}",
        capabilities.version.major,
        capabilities.version.minor,
        capabilities.cpu_on,
        capabilities.cpu_off,
        capabilities.cpu_suspend,
        capabilities.system_reset
    );
    Ok(())
}

/// Enumerates platform devices, promotes earlycon, and binds built-in drivers.
pub(crate) fn initialize_platform_devices(
    boot: &super::boot::Initialization,
) -> Result<(), InitializationError> {
    let report = platform_bus::initialize(boot).map_err(InitializationError::DriverFramework)?;
    match report.console {
        Ok(Some(capabilities)) => crate::println!(
            "HypeR: {} runtime input active: INTID {}, VIRQ {}",
            capabilities.driver,
            capabilities.hardware_interrupt,
            capabilities.virtual_interrupt
        ),
        Ok(None) => crate::pr_warn!(
            "HypeR: selected early console has no runtime input driver; console input disabled"
        ),
        Err(error) => crate::pr_warn!(
            "HypeR: early console remains output-only; runtime input unavailable: {error:?}"
        ),
    }
    crate::println!(
        "HypeR: platform bus: {} bound, {} unmatched, {} deferred, {} failed",
        report.drivers.bound,
        report.drivers.unmatched,
        report.drivers.deferred,
        report.drivers.failed
    );
    Ok(())
}
