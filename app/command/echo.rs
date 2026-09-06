// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Minimal external `echo` command for Native shell integration.

#![no_std]
#![no_main]

use hyper_os::startup::Startup;
use hyper_rt::ExitCode;
use hyper_service::stdio;

fn application_main(mut startup: Startup<'_>) -> ExitCode {
    if hyper_os::require_core_abi().is_err() {
        return ExitCode::FAILURE;
    }
    let output = match startup.take(stdio::STANDARD_OUTPUT) {
        Ok(output) => output,
        Err(_) => return ExitCode::FAILURE,
    };
    for index in 1..startup.argument_count() {
        if index != 1 && output.as_byte_channel().send(b" ").is_err() {
            return ExitCode::FAILURE;
        }
        let argument = match startup.argument(index) {
            Ok(argument) => argument,
            Err(_) => return ExitCode::FAILURE,
        };
        if output.as_byte_channel().send(argument.as_bytes()).is_err() {
            return ExitCode::FAILURE;
        }
    }
    if output.as_byte_channel().send(b"\n").is_err() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

hyper_rt::entry!(application_main);
