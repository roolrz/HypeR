//! Kernel device-service orchestration.

pub mod cpu_power;
mod platform_bus;
mod serial;

/// Selects and activates the firmware CPU power-management interface.
pub(crate) fn initialize_cpu_power(boot: &super::boot::Initialization) {
    let info = match boot.essential().cpu_power {
        Some(info) => info,
        None => super::boot::fail("CPU power discovery", "missing firmware interface"),
    };
    let capabilities = match cpu_power::initialize(info) {
        Ok(capabilities) => capabilities,
        Err(error) => super::boot::fail("CPU power initialization", error),
    };
    crate::println!(
        "HypeR: CPU power interface version {}.{}: on={}, off={}, suspend={}, reset={}",
        capabilities.version.major,
        capabilities.version.minor,
        capabilities.cpu_on,
        capabilities.cpu_off,
        capabilities.cpu_suspend,
        capabilities.system_reset
    );
}

/// Enumerates platform devices, promotes earlycon, and binds built-in drivers.
pub(crate) fn initialize_platform_devices(boot: &super::boot::Initialization) {
    let report = match platform_bus::initialize(boot) {
        Ok(report) => report,
        Err(error) => super::boot::fail("driver-framework initialization", error),
    };
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
}
