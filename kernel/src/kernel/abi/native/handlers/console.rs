// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native console syscall validation.

use crate::kernel::abi::native::Arguments;
use crate::kernel::abi::native::services::{ConsoleServices, DeferredAction};
use crate::kernel::abi::native::status::{console_io_result, status_from_console_service_error};
use crate::kernel::abi::native::wire::parse_console_io;
use hyper::abi::native::{HYPER_NATIVE_SYS_CONSOLE_READ, HYPER_NATIVE_SYS_CONSOLE_WRITE};

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_console_read(
    services: &impl ConsoleServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_console_io(arguments).and_then(|(console, bytes)| {
        services
            .read_console(console, bytes)
            .map_err(status_from_console_service_error)
    });
    DeferredAction::Return(console_io_result(HYPER_NATIVE_SYS_CONSOLE_READ, result))
}

#[inline(never)]
pub(in crate::kernel::abi::native) fn sys_console_write(
    services: &impl ConsoleServices,
    arguments: &Arguments,
) -> DeferredAction {
    let result = parse_console_io(arguments).and_then(|(console, bytes)| {
        services
            .write_console(console, bytes)
            .map_err(status_from_console_service_error)
    });
    DeferredAction::Return(console_io_result(HYPER_NATIVE_SYS_CONSOLE_WRITE, result))
}
