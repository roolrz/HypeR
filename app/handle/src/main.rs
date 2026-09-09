// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Capability-scoped Native kernel-object and Process-handle listing.

use clap::Parser;
use std::io::Write;

use hyper_os::handle::Rights;
use hyper_os::inspect::{Koid, ObjectHandleState, ObjectInspector, ScanCursor};
use hyper_os::startup;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = hyper_handle::cli::Handle::parse();
    let mut startup = hyper_rt::process::startup()?;
    hyper_os::require_core_abi()?;
    let inspector = ObjectInspector::from_handle(startup.take(startup::OBJECT_INSPECTOR)?);
    let mut output = std::io::stdout().lock();
    match args.process {
        Some(process) => list_handles(&inspector, Koid::from_raw(process.get())?, &mut output),
        None => list_objects(&inspector, &mut output),
    }
}

fn list_objects(
    inspector: &ObjectInspector,
    output: &mut impl Write,
) -> Result<(), Box<dyn std::error::Error>> {
    output.write_all(b"KOID       KIND                    HANDLE-STATE HANDLES REFS PURPOSE\n")?;
    let mut cursor = Some(ScanCursor::START);
    while let Some(position) = cursor {
        let page = inspector.scan_objects(position)?;
        for object in page.entries() {
            let (state, handles) = match object.handles {
                ObjectHandleState::Unpublished => ("unpublished", 0),
                ObjectHandleState::Active(count) => ("active", count),
                ObjectHandleState::Retired => ("retired", 0),
            };
            writeln!(
                output,
                "{:<10} {:<23} {:<12} {:<7} {:<4} {}",
                object.koid.get(),
                object.object_kind.name(),
                state,
                handles,
                object.references.strong,
                object.object_kind.purpose(),
            )?;
        }
        cursor = page.next();
    }
    Ok(())
}

fn list_handles(
    inspector: &ObjectInspector,
    process: Koid,
    output: &mut impl Write,
) -> Result<(), Box<dyn std::error::Error>> {
    output.write_all(b"HANDLE             OBJECT     KIND                    RIGHTS                           PURPOSE\n")?;
    let mut cursor = Some(ScanCursor::START);
    while let Some(position) = cursor {
        let page = inspector.scan_handles(process, position)?;
        for handle in page.entries() {
            writeln!(
                output,
                "0x{:016x} {:<10} {:<23} {:<32} {}",
                handle.handle,
                handle.object_koid.get(),
                handle.object_kind.name(),
                RightsList(handle.rights),
                handle.object_kind.purpose(),
            )?;
        }
        cursor = page.next();
    }
    Ok(())
}

struct RightsList(Rights);

impl std::fmt::Display for RightsList {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names = self.0.names().collect::<Vec<_>>().join("|");
        formatter.pad(if names.is_empty() { "none" } else { &names })
    }
}
