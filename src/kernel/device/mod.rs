//! Kernel device-service orchestration.

pub mod cpu_power;
pub(crate) mod platform_bus;

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

/// Enumerates platform devices and binds all built-in drivers.
pub(crate) fn initialize_driver_framework(boot: &super::boot::Initialization) {
    let report = match platform_bus::initialize(boot.linear_dtb(), boot.essential().claims()) {
        Ok(report) => report,
        Err(error) => super::boot::fail("driver-framework initialization", error),
    };
    crate::println!(
        "HypeR: platform bus: {} bound, {} unmatched, {} deferred, {} failed",
        report.bound,
        report.unmatched,
        report.deferred,
        report.failed
    );
}
