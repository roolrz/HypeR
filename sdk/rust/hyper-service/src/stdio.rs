// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Conventional byte-channel purposes for Native command processes.

use hyper_os::handle::{ByteChannelObject, Rights};
use hyper_os::startup::StartupPurpose;

use crate::StartupContract;

pub const STANDARD_INPUT_NAME: &str = "stdio.input";
pub const STANDARD_OUTPUT_NAME: &str = "stdio.output";
pub const STANDARD_ERROR_NAME: &str = "stdio.error";

pub const STANDARD_INPUT: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x8003_0001);
pub const STANDARD_OUTPUT: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x8003_0002);
pub const STANDARD_ERROR: StartupPurpose<ByteChannelObject> = StartupPurpose::new(0x8003_0003);

pub const STANDARD_INPUT_CONTRACT: StartupContract = StartupContract::exact(
    STANDARD_INPUT_NAME,
    STANDARD_INPUT,
    Rights::WAIT
        .union(Rights::READ)
        .union(Rights::DUPLICATE)
        .union(Rights::TRANSFER),
);
pub const STANDARD_OUTPUT_CONTRACT: StartupContract = StartupContract::exact(
    STANDARD_OUTPUT_NAME,
    STANDARD_OUTPUT,
    Rights::WAIT
        .union(Rights::WRITE)
        .union(Rights::DUPLICATE)
        .union(Rights::TRANSFER),
);
pub const STANDARD_ERROR_CONTRACT: StartupContract = StartupContract::exact(
    STANDARD_ERROR_NAME,
    STANDARD_ERROR,
    Rights::WAIT
        .union(Rights::WRITE)
        .union(Rights::DUPLICATE)
        .union(Rights::TRANSFER),
);

pub const STARTUP_CONTRACTS: &[StartupContract] = &[
    STANDARD_INPUT_CONTRACT,
    STANDARD_OUTPUT_CONTRACT,
    STANDARD_ERROR_CONTRACT,
];

/// Ordinary services may keep stdio without authority to delegate it to children.
pub const APPLICATION_STARTUP_CONTRACTS: &[StartupContract] = &[
    StartupContract::exact(
        STANDARD_INPUT_NAME,
        STANDARD_INPUT,
        Rights::WAIT.union(Rights::READ),
    )
    .with_optional_rights(Rights::DUPLICATE.union(Rights::TRANSFER)),
    StartupContract::exact(
        STANDARD_OUTPUT_NAME,
        STANDARD_OUTPUT,
        Rights::WAIT.union(Rights::WRITE),
    )
    .with_optional_rights(Rights::DUPLICATE.union(Rights::TRANSFER)),
    StartupContract::exact(
        STANDARD_ERROR_NAME,
        STANDARD_ERROR,
        Rights::WAIT.union(Rights::WRITE),
    )
    .with_optional_rights(Rights::DUPLICATE.union(Rights::TRANSFER)),
];
