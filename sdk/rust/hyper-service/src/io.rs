// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Explicit startup authority for the trusted physical I/O VM owner.

use crate::{StartupContract, vm};
use hyper_os::{handle::Rights, startup};

pub const DEVICE_AUTHORITY_NAME: &str = "io.device-authority";
pub const STARTUP_CONTRACTS: &[StartupContract] = &[
    vm::MANAGER_CREATION_AUTHORITY_CONTRACT,
    StartupContract::exact(
        DEVICE_AUTHORITY_NAME,
        startup::DEVICE_ASSIGNMENT_AUTHORITY,
        Rights::INSPECT,
    ),
];
