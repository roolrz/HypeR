// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Allocation-free init diagnostics.

use hyper_app::diagnostics::write_process_termination;
use hyper_os::handle::{ConsoleObject, OwnedHandle, ProcessObject};
use hyper_service::vm as vm_contract;

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

pub(super) fn report_vm_event(
    output: &OwnedHandle<ConsoleObject>,
    event: vm_contract::InstanceEvent,
) {
    let console = output.as_emergency_console();
    match event {
        vm_contract::InstanceEvent::Stopped => {
            let _ = console.write_all(b"HypeR init: initial VM stopped cleanly\n");
        }
        vm_contract::InstanceEvent::Failed(reason) => {
            let _ = console.write_all(b"HypeR init: initial VM failed: ");
            let _ = console.write_all(vm_failure_reason(reason));
            let _ = console.write_all(b"\n");
        }
    }
}

pub(super) fn report_vm_protocol_failure(output: &OwnedHandle<ConsoleObject>, reason: &[u8]) {
    let console = output.as_emergency_console();
    let _ = console.write_all(b"HypeR init: initial VM protocol failed: ");
    let _ = console.write_all(reason);
    let _ = console.write_all(b"\n");
}

const fn vm_failure_reason(reason: vm_contract::InstanceFailure) -> &'static [u8] {
    match reason {
        vm_contract::InstanceFailure::Runtime => b"runtime process failed",
        vm_contract::InstanceFailure::InvalidImage => b"guest image is invalid",
        vm_contract::InstanceFailure::GuestMemoryFault => b"guest memory access failed",
        vm_contract::InstanceFailure::GuestMmio => b"guest MMIO emulation failed",
        vm_contract::InstanceFailure::GuestSynchronous => b"guest synchronous exit failed",
        vm_contract::InstanceFailure::UnexpectedAdministrativeStop => {
            b"guest stopped without a lifecycle request"
        }
        vm_contract::InstanceFailure::InvalidControlProtocol => b"control protocol is invalid",
        vm_contract::InstanceFailure::MissingTerminalStatus => b"terminal status is missing",
        vm_contract::InstanceFailure::UnsupportedConfiguration => {
            b"requested configuration is unsupported"
        }
    }
}
