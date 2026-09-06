// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Startup contract privately shared by init and the session manager.

use hyper_os::handle::ByteChannelObject;
use hyper_os::startup::StartupPurpose;

/// Raw input messages received from the active Console input worker.
pub const INPUT: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x4859_0001);

/// Raw output messages sent to the active Console output worker.
pub const OUTPUT: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x4859_0002);
