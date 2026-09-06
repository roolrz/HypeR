// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Startup purposes shared by init and the physical Console workers.

use hyper_os::handle::{ByteChannelObject, ConsoleObject};
use hyper_os::startup::StartupPurpose;

/// Direction-attenuated physical Console authority.
pub const SYSTEM_CONSOLE: StartupPurpose<ConsoleObject> = StartupPurpose::new(0x4859_0001);

/// Raw data-plane endpoint owned by one Console worker.
pub const DATA_CHANNEL: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x4859_0002);
