//! Optional crash-monitor lifecycle adapter.
//!
//! This module is the only coordination-to-monitor edge. It keeps feature
//! selection and emergency-console availability out of the fail-stop state
//! machine; when the feature is disabled both calls compile to no-ops.

use super::report::StopSummary;

pub(super) fn initialize() {
    #[cfg(CONFIG_CRASH_CONSOLE)]
    super::monitor::initialize();
}

pub(super) fn run(owner: usize, stop: StopSummary) {
    #[cfg(CONFIG_CRASH_CONSOLE)]
    if super::super::log::crash_console_available() {
        super::super::log::emergency(format_args!(
            "crash console enabled; entering interactive monitor"
        ));
        super::monitor::run(owner, stop);
    } else {
        super::super::log::emergency(format_args!(
            "crash console enabled but no emergency console is available"
        ));
    }

    #[cfg(not(CONFIG_CRASH_CONSOLE))]
    let _ = (owner, stop);
}
