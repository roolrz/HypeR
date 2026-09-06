// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Startup contract for the initial foreground-session router.

use hyper_os::handle::ByteChannelObject;
use hyper_os::startup::StartupPurpose;

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
