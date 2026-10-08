// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use std::fmt;
use std::io::{self, Write};

use hyper_os::handle::{ObjectKind, Rights};
use hyper_os::inspect::{HandleObservation, ObjectHandleState, ObjectObservation};

use crate::cli::Handle;

pub fn catalog(args: &Handle, out: &mut impl Write) -> io::Result<()> {
    if args.list_kinds {
        if !args.no_headers {
            writeln!(out, "{:<10} {:<29} PURPOSE", "KIND-ID", "KIND")?;
        }
        for kind in ObjectKind::KNOWN {
            writeln!(
                out,
                "0x{:08x} {:<29} {}",
                kind.as_raw(),
                kind.name(),
                kind.purpose()
            )?;
        }
    } else {
        for name in Rights::known_names() {
            writeln!(out, "{name}")?;
        }
    }
    Ok(())
}

pub fn object_header(args: &Handle, out: &mut impl Write) -> io::Result<()> {
    if !args.no_headers {
        writeln!(
            out,
            "{:<18} {:<29} {:<11} {:>7} {:>7}",
            "KOID", "KIND", "STATE", "HANDLES", "REFS"
        )?;
    }
    Ok(())
}

pub fn object(args: &Handle, item: &ObjectObservation, out: &mut impl Write) -> io::Result<()> {
    let (state, handles) = match item.handles {
        ObjectHandleState::Unpublished => ("unpublished", 0),
        ObjectHandleState::Active(count) => ("active", count),
        ObjectHandleState::Retired => ("retired", 0),
    };
    writeln!(
        out,
        "0x{:016x} {:<29} {:<11} {:>7} {:>7}",
        item.koid.get(),
        KindName(item.object_kind),
        state,
        handles,
        item.references.strong
    )?;
    if args.verbose {
        writeln!(out, "  purpose: {}", item.object_kind.purpose())?;
        writeln!(
            out,
            "  supported-rights: {} (0x{:016x})",
            RightsList(item.supported_rights),
            item.supported_rights.bits()
        )?;
        let refs = &item.references;
        writeln!(
            out,
            "  refs: kernel-service={} vm-device-binding={} scheduler={} operation={}",
            refs.kernel_service, refs.vm_device_binding, refs.scheduler, refs.operation
        )?;
        writeln!(
            out,
            "        user-authority={} publication={} diagnostic={} retirement={}",
            refs.user_authority, refs.publication, refs.diagnostic, refs.retirement
        )?;
    }
    Ok(())
}

pub fn handle_header(args: &Handle, out: &mut impl Write) -> io::Result<()> {
    if !args.no_headers {
        if args.all {
            write!(out, "{:<18} ", "PROCESS")?;
        }
        write!(
            out,
            "{:<18} {:<18} {:<29} RIGHTS",
            "HANDLE", "OBJECT", "KIND"
        )?;
        writeln!(out, "{}", if args.all { " NAME" } else { "" })?;
    }
    Ok(())
}

pub fn handle(
    args: &Handle,
    item: &HandleObservation,
    name: &str,
    out: &mut impl Write,
) -> io::Result<()> {
    if args.all {
        write!(out, "0x{:016x} ", item.process_koid.get())?;
    }
    write!(
        out,
        "0x{:016x} 0x{:016x} {:<29} {}",
        item.handle,
        item.object_koid.get(),
        KindName(item.object_kind),
        RightsList(item.rights)
    )?;
    if args.all {
        write!(out, " {name:?}")?;
    }
    writeln!(out)?;
    if args.verbose {
        writeln!(out, "  purpose: {}", item.object_kind.purpose())?;
        writeln!(
            out,
            "  granted-rights: 0x{:016x}; flags: 0x{:08x}",
            item.rights.bits(),
            item.flags
        )?;
    }
    Ok(())
}

struct KindName(ObjectKind);

impl fmt::Display for KindName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.name() == "unknown" {
            f.pad(&format!("unknown(0x{:08x})", self.0.as_raw()))
        } else {
            f.pad(self.0.name())
        }
    }
}

struct RightsList(Rights);

impl fmt::Display for RightsList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut names = self.0.names();
        match names.next() {
            None => f.write_str("none"),
            Some(first) => {
                f.write_str(first)?;
                for name in names {
                    write!(f, "|{name}")?;
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
#[path = "../tests/output.rs"]
mod tests;
