// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Startup contract for the initial foreground-session router.

use hyper_os::handle::{ByteChannelObject, Rights};
use hyper_os::startup::StartupPurpose;

use crate::StartupContract;

pub const CONSOLE_INPUT_NAME: &str = "session.console-input";
pub const CONSOLE_OUTPUT_NAME: &str = "session.console-output";
pub const CLIENT_INPUT_NAME: &str = "session.client-input";
pub const CLIENT_OUTPUT_NAME: &str = "session.client-output";
pub const CLIENT_ERROR_NAME: &str = "session.client-error";

pub const CONSOLE_INPUT: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x8002_0001);
pub const CONSOLE_OUTPUT: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x8002_0002);
pub const CLIENT_INPUT: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x8002_0003);
pub const CLIENT_OUTPUT: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x8002_0004);
pub const CLIENT_ERROR: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x8002_0005);

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
pub const CLIENT_INPUT_CONTRACT: StartupContract = StartupContract::exact(
    CLIENT_INPUT_NAME,
    CLIENT_INPUT,
    Rights::WAIT.union(Rights::WRITE),
);
pub const CLIENT_OUTPUT_CONTRACT: StartupContract = StartupContract::exact(
    CLIENT_OUTPUT_NAME,
    CLIENT_OUTPUT,
    Rights::WAIT.union(Rights::READ),
);
pub const CLIENT_ERROR_CONTRACT: StartupContract = StartupContract::exact(
    CLIENT_ERROR_NAME,
    CLIENT_ERROR,
    Rights::WAIT.union(Rights::READ),
);

pub const STARTUP_CONTRACTS: &[StartupContract] = &[
    CONSOLE_INPUT_CONTRACT,
    CONSOLE_OUTPUT_CONTRACT,
    CLIENT_INPUT_CONTRACT,
    CLIENT_OUTPUT_CONTRACT,
    CLIENT_ERROR_CONTRACT,
];
