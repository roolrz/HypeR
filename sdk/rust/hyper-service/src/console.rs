// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Startup contract for the direction-attenuated physical Console workers.

use hyper_os::handle::{ByteChannelObject, ConsoleObject};
use hyper_os::startup::StartupPurpose;

pub const SYSTEM_CONSOLE_NAME: &str = "console.system";
pub const DATA_CHANNEL_NAME: &str = "console.data";

pub const SYSTEM_CONSOLE: StartupPurpose<ConsoleObject> = StartupPurpose::new(0x8001_0001);
pub const DATA_CHANNEL: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x8001_0002);
