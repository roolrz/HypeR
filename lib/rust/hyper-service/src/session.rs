// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Startup contract for the virtual console manager.

use hyper_os::handle::{ByteChannelObject, Rights};
use hyper_os::startup::StartupPurpose;

use crate::StartupContract;

pub const CONSOLE_INPUT_NAME: &str = "session.console-input";
pub const CONSOLE_OUTPUT_NAME: &str = "session.console-output";

pub const CONSOLE_INPUT: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x8002_0001);
pub const CONSOLE_OUTPUT: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x8002_0002);

pub const CONSOLE_INPUT_CONTRACT: StartupContract = StartupContract::exact(
    CONSOLE_INPUT_NAME,
    CONSOLE_INPUT,
    Rights::WAIT.union(Rights::READ),
);
pub const CONSOLE_OUTPUT_CONTRACT: StartupContract = StartupContract::exact(
    CONSOLE_OUTPUT_NAME,
    CONSOLE_OUTPUT,
    Rights::WAIT.union(Rights::WRITE),
);
pub const STARTUP_CONTRACTS: &[StartupContract] = &[
    CONSOLE_INPUT_CONTRACT,
    CONSOLE_OUTPUT_CONTRACT,
    StartupContract::exact(
        crate::vm::MANAGER_CONNECTION_NAME,
        crate::vm::MANAGER_CONNECTION,
        Rights::WAIT
            .union(Rights::WRITE)
            .union(Rights::DUPLICATE)
            .union(Rights::TRANSFER),
    ),
];
