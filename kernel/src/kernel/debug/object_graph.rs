// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Text rendering for the authority-free object and handle snapshot APIs.

use crate::kernel::capability::HandleScanCursor;
use crate::kernel::object::{
    ExportPolicy, ObjectHandleState, ObjectReferenceSnapshot, ObjectScanCursor,
};
use crate::kernel::process::ProcessScanCursor;
use crate::kernel::task::ThreadObjectScanCursor;

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
            let export = match object.export_policy {
                ExportPolicy::KernelOnly => "kernel",
                ExportPolicy::User => "user",
            };
            let references: ObjectReferenceSnapshot = object.references;
            crate::println!(
                "HypeR object: koid={} kind={} state={} export={} handles={} refs={} classes=[service:{},scheduler:{},publication:{},user:{},pin:{},diagnostic:{},retirement:{}] rights={:#x}",
                object.koid.get(),
                object.kind.get(),
                handles,
                export,
                active_handles,
                object.strong_references,
                references.kernel_service,
                references.scheduler,
                references.publication,
                references.user_authority,
                references.operation_pin,
                references.diagnostic,
                references.retirement,
                object.supported_rights.bits()
            );
            object_count = object_count.saturating_add(1);
        }
        object_cursor = page.next();
    }

    let mut thread_count = 0usize;
    if crate::kernel::task::is_ready() {
        let mut thread_cursor = Some(ThreadObjectScanCursor::start());
        while let Some(cursor) = thread_cursor {
            let page = match crate::kernel::task::scheduler::scan_thread_objects(cursor) {
                Ok(page) => page,
                Err(_) => break,
            };
            for thread in page.entries() {
                crate::println!(
                    "HypeR thread-object: thread={} koid={} role={:?} registry={:?}",
                    thread.thread.get(),
                    thread.object.object.koid.get(),
                    thread.object.role,
                    thread.phase,
                );
                thread_count = thread_count.saturating_add(1);
            }
            thread_cursor = page.next();
        }
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
        "HypeR: object diagnostics ready (objects={}, threads={}, processes={}, handles={}, unavailable={})",
        object_count,
        thread_count,
        process_count,
        handle_count,
        unavailable_tables
    );
}
