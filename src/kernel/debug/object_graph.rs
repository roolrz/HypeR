// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Text rendering for the authority-free object and handle snapshot APIs.

use crate::kernel::capability::HandleScanCursor;
use crate::kernel::object::{ObjectHandleState, ObjectScanCursor};
use crate::kernel::process::ProcessScanCursor;

pub(super) fn report() {
    let mut object_count = 0usize;
    let mut object_cursor = Some(ObjectScanCursor::start());
    while let Some(cursor) = object_cursor {
        let page = crate::kernel::object::scan(cursor);
        for object in page.entries() {
            let handles = match object.handles {
                ObjectHandleState::Unpublished => "unpublished",
                ObjectHandleState::Active(_) => "active",
                ObjectHandleState::Retired => "retired",
            };
            let active_handles = match object.handles {
                ObjectHandleState::Active(count) => count,
                ObjectHandleState::Unpublished | ObjectHandleState::Retired => 0,
            };
            crate::println!(
                "HypeR object: koid={} kind={} state={} handles={} refs={} rights={:#x}",
                object.koid.get(),
                object.kind.get(),
                handles,
                active_handles,
                object.strong_references,
                object.supported_rights.bits()
            );
            object_count = object_count.saturating_add(1);
        }
        object_cursor = page.next();
    }

    let mut process_count = 0usize;
    let mut handle_count = 0usize;
    let mut unavailable_tables = 0usize;
    let mut process_cursor = Some(ProcessScanCursor::start());
    while let Some(cursor) = process_cursor {
        let page = crate::kernel::process::scan(cursor);
        for entry in page.entries() {
            let snapshot = entry.snapshot();
            crate::println!(
                "HypeR process: id={} phase={:?} threads={}/{}",
                snapshot.id.get(),
                snapshot.phase,
                snapshot.active_threads,
                snapshot.pending_threads
            );
            process_count = process_count.saturating_add(1);
            let mut handle_cursor = Some(HandleScanCursor::start());
            while let Some(cursor) = handle_cursor {
                let handle_page = match entry.scan_handles(cursor) {
                    Ok(page) => page,
                    Err(_) => {
                        unavailable_tables = unavailable_tables.saturating_add(1);
                        break;
                    }
                };
                for handle in handle_page.entries() {
                    crate::println!(
                        "HypeR handle: process={} value={:#x} koid={} kind={} rights={:#x} flags={:#x}",
                        snapshot.id.get(),
                        handle.value.get(),
                        handle.info.koid.get(),
                        handle.info.kind.get(),
                        handle.info.rights.bits(),
                        handle.info.flags.bits()
                    );
                    handle_count = handle_count.saturating_add(1);
                }
                handle_cursor = handle_page.next();
            }
        }
        process_cursor = page.next();
    }
    crate::println!(
        "HypeR: object diagnostics ready (objects={}, processes={}, handles={}, unavailable={})",
        object_count,
        process_count,
        handle_count,
        unavailable_tables
    );
}
