// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Allocation-free init diagnostics.

use hyper_init::diagnostics::write_process_termination;
use hyper_os::handle::{ConsoleObject, OwnedHandle, ProcessObject};

use super::LaunchError;

pub(super) fn report_service_launch_failure(
    output: &OwnedHandle<ConsoleObject>,
    service: &str,
    error: &LaunchError,
) {
    let console = output.as_emergency_console();
    let _ = console.write_all(b"HypeR init: failed to launch service '");
    let _ = console.write_all(service.as_bytes());
    let _ = console.write_all(b"': ");
    let _ = console.write_all(error.reason());
    let _ = console.write_all(b"\n");
}

pub(super) fn report_service_termination(
    output: &OwnedHandle<ConsoleObject>,
    service: &str,
    critical: bool,
    supervisor: &OwnedHandle<ProcessObject>,
) {
    let console = output.as_emergency_console();
    if critical {
        let _ = console.write_all(b"HypeR init: critical service '");
    } else {
        let _ = console.write_all(b"HypeR init: service '");
    }
    let _ = console.write_all(service.as_bytes());
    let _ = console.write_all(b"' terminated: ");
    match supervisor.as_process_supervisor().info() {
        Ok(info) => {
            write_process_termination(info, |fragment| {
                let _ = console.write_all(fragment);
            });
        }
        Err(_) => {
            let _ = console.write_all(b"process-info-unavailable");
        }
    }
    let _ = console.write_all(b"\n");
}

pub(super) fn report_fleet_configured(output: &OwnedHandle<ConsoleObject>) {
    let _ = output
        .as_emergency_console()
        .write_all(b"HypeR init: VM fleet configured\n");
}
