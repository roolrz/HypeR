// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Conventional byte-channel purposes for Native command processes.

use hyper_os::handle::ByteChannelObject;
use hyper_os::startup::StartupPurpose;

pub const STANDARD_INPUT_NAME: &str = "stdio.input";
pub const STANDARD_OUTPUT_NAME: &str = "stdio.output";
pub const STANDARD_ERROR_NAME: &str = "stdio.error";

pub const STANDARD_INPUT: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x8003_0001);
pub const STANDARD_OUTPUT: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x8003_0002);
pub const STANDARD_ERROR: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x8003_0003);
