// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Read-only scans. Identifiers select observations, never acquire authority.

use std::io::{self, Write};

use hyper_os::inspect::{
    DetailTarget, HandleObservation, InspectionPage, Koid, ObjectInspector, ProcessObservation,
    ScanCursor, TaskInspector,
};
use hyper_os::{Error, Status};

use crate::cli::{Handle, ProcessSelector};
use crate::output;
use crate::summary::Summary;

pub fn inspect(
    args: &Handle,
    objects: &ObjectInspector,
    tasks: Option<&TaskInspector>,
    out: &mut impl Write,
    errors: &mut impl Write,
) -> io::Result<()> {
    let mut summary = Summary::default();
    inspect_entries(args, objects, tasks, out, errors, &mut summary)?;
    if args.summary {
        summary.write(
            args.all || args.process_selector().is_some(),
            !args.no_headers,
            out,
        )?;
    }
    Ok(())
}

fn inspect_entries(
    args: &Handle,
    objects: &ObjectInspector,
    tasks: Option<&TaskInspector>,
    out: &mut impl Write,
    errors: &mut impl Write,
    summary: &mut Summary,
) -> io::Result<()> {
    if args.all {
        let processes = processes(require_tasks(tasks)?)?;
        let mut count = 0;
        let mut skipped = 0;
        for process in processes {
            match list_handles(
                args,
                objects,
                process.koid,
                process.name.as_str(),
                out,
                &mut count,
                summary,
            ) {
                Ok(()) => (),
                Err(error) if process_unavailable(&error) => skipped += 1,
                Err(error) => return Err(context(process.koid, error)),
            }
        }
        if skipped != 0 {
            writeln!(
                errors,
                "handle: skipped {skipped} process(es): exited during the scan or outside the object inspector scope"
            )?;
        }
        empty(args, count, "handles in visible processes", out)
    } else if let Some(selector) = args.process_selector() {
        let koid = match selector {
            ProcessSelector::Koid(id) => Koid::from_raw(id.get()).map_err(io::Error::other)?,
            ProcessSelector::Name(name) => {
                let processes = processes(require_tasks(tasks)?)?;
                named_process(name, processes.iter().map(|p| (p.koid, p.name.as_str())))?
            }
        };
        let name = match selector {
            ProcessSelector::Name(name) => name.as_str(),
            ProcessSelector::Koid(_) => "",
        };
        let mut count = 0;
        list_handles(args, objects, koid, name, out, &mut count, summary)
            .map_err(|e| context(koid, e))?;
        empty(args, count, "handles", out)?;
        if count != 0
            && !args.no_headers
            && !args.summary
            && let Some(handle) = args.handle
        {
            crate::details::inspect(
                objects,
                tasks,
                DetailTarget::Handle {
                    process: koid,
                    handle,
                },
                out,
            )?;
        }
        Ok(())
    } else {
        let count = list_objects(args, objects, out, summary)?;
        if count != 0
            && !args.no_headers
            && !args.summary
            && let Some(object) = args.object
        {
            let koid = Koid::from_raw(object.get()).map_err(io::Error::other)?;
            crate::details::inspect(objects, tasks, DetailTarget::Object(koid), out)?;
        }
        Ok(())
    }
}

fn require_tasks(tasks: Option<&TaskInspector>) -> io::Result<&TaskInspector> {
    tasks.ok_or_else(|| io::Error::other("process names and --all require a task-inspector capability; use a process KOID or launch from the system shell"))
}

pub(crate) fn processes(inspector: &TaskInspector) -> io::Result<Vec<ProcessObservation>> {
    let mut processes = Vec::new();
    scan(
        |cursor| inspector.scan_processes(cursor),
        |process| {
            processes.push(*process);
            Ok(())
        },
    )?;
    Ok(processes)
}

fn named_process<'a>(
    name: &str,
    processes: impl Iterator<Item = (Koid, &'a str)>,
) -> io::Result<Koid> {
    let matches: Vec<_> = processes
        .filter(|(_, candidate)| *candidate == name)
        .map(|(koid, _)| koid)
        .collect();
    match matches.as_slice() {
        [koid] => Ok(*koid),
        [] => Err(io::Error::other(format!(
            "no visible process named {name:?}; use ps to list processes"
        ))),
        _ => Err(io::Error::other(format!(
            "process name {name:?} is ambiguous; select a KOID: {}",
            matches
                .iter()
                .map(|id| format!("0x{:016x}", id.get()))
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

fn list_objects(
    args: &Handle,
    inspector: &ObjectInspector,
    out: &mut impl Write,
    summary: &mut Summary,
) -> io::Result<usize> {
    let mut count = 0;
    let mut selected_exists = false;
    scan(
        |cursor| inspector.scan_objects(cursor),
        |item| {
            selected_exists |= args.object.is_some_and(|id| id.get() == item.koid.get());
            if args.matches_object(item.koid.get(), item.object_kind) {
                if args.summary {
                    summary.record(item.object_kind, item.koid.get(), false);
                } else {
                    if count == 0 {
                        output::object_header(args, out)?;
                    }
                    output::object(args, item, out)?;
                }
                count += 1;
            }
            Ok(())
        },
    )?;
    if let Some(id) = args.object.filter(|_| !selected_exists) {
        return Err(io::Error::other(format!(
            "object 0x{:016x} is not visible or no longer exists",
            id.get()
        )));
    }
    empty(args, count, "objects", out)?;
    Ok(count)
}

fn list_handles(
    args: &Handle,
    inspector: &ObjectInspector,
    process: Koid,
    name: &str,
    out: &mut impl Write,
    count: &mut usize,
    summary: &mut Summary,
) -> io::Result<()> {
    let mut selected_exists = false;
    scan(
        |cursor| inspector.scan_handles(process, cursor),
        |item| {
            selected_exists |= args.handle.is_some_and(|id| id.get() == item.handle);
            if matches_handle(args, item) {
                if args.summary {
                    summary.record(item.object_kind, item.object_koid.get(), true);
                } else {
                    if *count == 0 {
                        if !args.all && !args.no_headers {
                            write!(out, "Process 0x{:016x}", process.get())?;
                            if !name.is_empty() {
                                write!(out, " {name:?}")?;
                            }
                            writeln!(out)?;
                        }
                        output::handle_header(args, out)?;
                    }
                    output::handle(args, item, name, out)?;
                }
                *count += 1;
            }
            Ok(())
        },
    )?;
    if let Some(id) = args.handle.filter(|_| !selected_exists) {
        return Err(io::Error::other(format!(
            "handle 0x{:016x} is not present",
            id.get()
        )));
    }
    Ok(())
}

fn matches_handle(args: &Handle, item: &HandleObservation) -> bool {
    args.matches_object(item.object_koid.get(), item.object_kind)
        && args.handle.is_none_or(|id| id.get() == item.handle)
        && args.right.iter().all(|right| item.rights.contains(*right))
}

pub(crate) fn scan<T: Copy, const N: usize>(
    mut page: impl FnMut(ScanCursor) -> hyper_os::Result<InspectionPage<T, N>>,
    mut visit: impl FnMut(&T) -> io::Result<()>,
) -> io::Result<()> {
    let mut cursor = Some(ScanCursor::START);
    while let Some(position) = cursor {
        let entries = page(position).map_err(io::Error::other)?;
        for entry in entries.entries() {
            visit(entry)?;
        }
        cursor = entries.next();
    }
    Ok(())
}

fn empty(args: &Handle, count: usize, noun: &str, out: &mut impl Write) -> io::Result<()> {
    if count == 0 && !args.no_headers && !args.summary {
        writeln!(out, "(no matching {noun})")?;
    }
    Ok(())
}

pub(crate) fn process_unavailable(error: &io::Error) -> bool {
    matches!(
        error.get_ref().and_then(|e| e.downcast_ref::<Error>()),
        Some(Error::Status(Status::NOT_FOUND))
    )
}

fn context(process: Koid, error: io::Error) -> io::Error {
    if error.kind() == io::ErrorKind::BrokenPipe {
        return error;
    }
    let detail = if process_unavailable(&error) {
        "not visible or no longer exists".to_owned()
    } else {
        error.to_string()
    };
    io::Error::other(format!("process 0x{:016x}: {detail}", process.get()))
}

#[cfg(test)]
#[path = "../tests/query.rs"]
mod tests;
