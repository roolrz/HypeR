// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Capability-scoped Native kernel-object and Process-handle listing.

#![no_std]
#![no_main]

#[path = "format.rs"]
mod format;

use core::fmt::Write;

use format::Buffer;
use hyper_os::handle::Rights;
use hyper_os::inspect::{Koid, ObjectHandleState, ObjectInspector, ScanCursor};
use hyper_os::startup::{self, Startup};
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
    let inspector = match startup.take(startup::OBJECT_INSPECTOR) {
        Ok(handle) => ObjectInspector::from_handle(handle),
        Err(_) => return ExitCode::FAILURE,
    };
    let argument = match startup.argument(1) {
        Ok(argument) => argument,
        Err(_) => {
            let _ = output
                .as_byte_channel()
                .send(b"usage: handle --objects | <process-koid>\n");
            return ExitCode::FAILURE;
        }
    };
    let result = if argument == "--objects" {
        list_objects(&inspector, &output)
    } else {
        parse_u64(argument)
            .and_then(Koid::from_raw)
            .and_then(|koid| list_handles(&inspector, koid, &output))
    };
    if result.is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn list_objects(
    inspector: &ObjectInspector,
    output: &hyper_os::OwnedHandle<hyper_os::handle::ByteChannelObject>,
) -> hyper_os::Result<()> {
    output
        .as_byte_channel()
        .send(b"KOID       KIND                    HANDLE-STATE HANDLES REFS PURPOSE\n")?;
    let mut cursor = Some(ScanCursor::START);
    while let Some(position) = cursor {
        let page = inspector.scan_objects(position)?;
        for object in page.entries() {
            let mut line = Buffer::<256>::new();
            let (state, handles) = match object.handles {
                ObjectHandleState::Unpublished => ("unpublished", 0),
                ObjectHandleState::Active(count) => ("active", count),
                ObjectHandleState::Retired => ("retired", 0),
            };
            writeln!(
                line,
                "{:<10} {:<23} {:<12} {:<7} {:<4} {}",
                object.koid.get(),
                object.object_kind.name(),
                state,
                handles,
                object.references.strong,
                object.object_kind.purpose(),
            )
            .map_err(|_| hyper_os::Error::InvalidResponse)?;
            output.as_byte_channel().send(line.bytes())?;
        }
        cursor = page.next();
    }
    Ok(())
}

fn list_handles(
    inspector: &ObjectInspector,
    process: Koid,
    output: &hyper_os::OwnedHandle<hyper_os::handle::ByteChannelObject>,
) -> hyper_os::Result<()> {
    output
        .as_byte_channel()
        .send(b"HANDLE             OBJECT     KIND                    RIGHTS                           PURPOSE\n")?;
    let mut cursor = Some(ScanCursor::START);
    while let Some(position) = cursor {
        let page = inspector.scan_handles(process, position)?;
        for handle in page.entries() {
            let mut line = Buffer::<512>::new();
            writeln!(
                line,
                "0x{:016x} {:<10} {:<23} {:<32} {}",
                handle.handle,
                handle.object_koid.get(),
                handle.object_kind.name(),
                RightsList(handle.rights),
                handle.object_kind.purpose(),
            )
            .map_err(|_| hyper_os::Error::InvalidResponse)?;
            output.as_byte_channel().send(line.bytes())?;
        }
        cursor = page.next();
    }
    Ok(())
}

struct RightsList(Rights);

impl core::fmt::Display for RightsList {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut names = self.0.names();
        let Some(first) = names.next() else {
            return write_padded(formatter, "none", 4);
        };
        formatter.write_str(first)?;
        let mut length = first.len();
        for name in names {
            formatter.write_str("|")?;
            formatter.write_str(name)?;
            length += 1 + name.len();
        }
        write_padding(formatter, length)
    }
}

fn write_padded(
    formatter: &mut core::fmt::Formatter<'_>,
    value: &str,
    length: usize,
) -> core::fmt::Result {
    formatter.write_str(value)?;
    write_padding(formatter, length)
}

fn write_padding(
    formatter: &mut core::fmt::Formatter<'_>,
    content_length: usize,
) -> core::fmt::Result {
    for _ in content_length..formatter.width().unwrap_or(0) {
        formatter.write_str(" ")?;
    }
    Ok(())
}

fn parse_u64(value: &str) -> hyper_os::Result<u64> {
    let mut number = 0_u64;
    if value.is_empty() {
        return Err(hyper_os::Error::InvalidResponse);
    }
    for byte in value.bytes() {
        let digit = byte
            .checked_sub(b'0')
            .filter(|digit| *digit <= 9)
            .ok_or(hyper_os::Error::InvalidResponse)?;
        number = number
            .checked_mul(10)
            .and_then(|number| number.checked_add(u64::from(digit)))
            .ok_or(hyper_os::Error::InvalidResponse)?;
    }
    Ok(number)
}

hyper_rt::entry!(application_main);
