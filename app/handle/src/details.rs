// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Formatting object-local records; peer holder discovery stays in user space.
use hyper_os::inspect::{
    DetailCursor, DetailRecord, DetailTarget, Koid, MappingPermissions, ObjectInspector,
    TaskInspector,
};
use hyper_os::{Error, Status};
use std::io::{self, Write};

pub(crate) fn inspect(
    inspector: &ObjectInspector,
    tasks: Option<&TaskInspector>,
    target: DetailTarget,
    out: &mut impl Write,
) -> io::Result<()> {
    let mut cursor = Some(DetailCursor::START);
    let mut identity = None;
    while let Some(position) = cursor {
        let details = match inspector.read_details(target, position) {
            Ok(details) => details,
            Err(Error::Status(Status::NOT_SUPPORTED)) if identity.is_none() => {
                return writeln!(out, "  details: not available for this object kind");
            }
            Err(Error::Status(Status::ACCESS_DENIED)) => {
                return Err(io::Error::other(
                    "object details require inspect-details authority and a visible target; launch /bin/handle from the system shell",
                ));
            }
            Err(error) => return Err(io::Error::other(error)),
        };
        if identity.is_some_and(|id| id != details.koid) {
            return Err(io::Error::other("object changed during detail scan"));
        }
        identity = Some(details.koid);
        write_record(details.record, out)?;
        if let DetailRecord::Channel {
            peer: Some(peer), ..
        } = details.record
        {
            peer_holders(inspector, tasks, peer, out)?;
        }
        cursor = details.next;
    }
    Ok(())
}

fn write_record(record: DetailRecord, out: &mut impl Write) -> io::Result<()> {
    match record {
        DetailRecord::Empty => (),
        DetailRecord::Thread { tid, role, phase } => {
            match tid {
                Some(tid) => writeln!(out, "  scheduler-tid: 0x{tid:016x}; role: {}", role.name())?,
                None => writeln!(out, "  scheduler-tid: unavailable; role: {}", role.name())?,
            }
            if let Some(phase) = phase {
                writeln!(out, "  user-thread-phase: {}", phase.name())?;
            }
        }
        DetailRecord::Vmar { base, length, live } => {
            writeln!(
                out,
                "  vmar: [0x{base:016x}, 0x{:016x}); bytes: {length}; live: {live}",
                base + length
            )?;
        }
        DetailRecord::Mapping {
            base,
            length,
            permissions,
            maximum_permissions,
        } => {
            writeln!(
                out,
                "  mapping: [0x{base:016x}, 0x{:016x}); permissions: {}; maximum: {}",
                base + length,
                protection(permissions),
                protection(maximum_permissions)
            )?;
        }
        DetailRecord::Channel {
            peer,
            local_open,
            peer_open,
            queued,
            bytes,
            byte_queue,
        } => {
            if let Some(peer) = peer {
                writeln!(out, "  peer-koid: 0x{:016x}", peer.get())?;
            } else {
                writeln!(out, "  peer-koid: not yet published")?;
            }
            writeln!(out, "  local-open: {local_open}; peer-open: {peer_open}")?;
            if byte_queue {
                writeln!(out, "  queued-messages: {queued}; queued-bytes: {bytes}")?;
            } else {
                writeln!(out, "  waiting-receivers: {queued}")?;
            }
        }
        DetailRecord::Device {
            profile,
            device_id,
            pci_identity,
            irq_domain,
            interrupt,
            interrupt_count,
            state,
            resource_count,
        } => {
            writeln!(
                out,
                "  device-profile: {} (0x{profile:x}); state: {}",
                profile_name(profile),
                state.name()
            )?;
            writeln!(
                out,
                "  device-id: 0x{device_id:x}; pci-vendor-device: 0x{pci_identity:08x}"
            )?;
            writeln!(
                out,
                "  host-irq-domain: 0x{irq_domain:x}; first-interrupt: {interrupt}; count: {interrupt_count}; resources: {resource_count}"
            )?;
        }
        DetailRecord::DeviceResource {
            kind,
            base,
            length,
            offset,
            flags,
        } => {
            writeln!(
                out,
                "  resource 0x{kind:x}: host-physical [0x{base:016x}, 0x{:016x}); guest-aperture-offset: 0x{offset:x}; attributes: 0x{flags:x}",
                base + length
            )?;
        }
    }
    Ok(())
}

fn protection(permissions: MappingPermissions) -> String {
    [
        if permissions.readable() { 'r' } else { '-' },
        if permissions.writable() { 'w' } else { '-' },
        if permissions.executable() { 'x' } else { '-' },
    ]
    .iter()
    .collect()
}

fn profile_name(profile: u32) -> &'static str {
    match profile {
        1 => "virtio-mmio-scsi",
        2 => "userspace-registers",
        3 => "virtio-mmio-net",
        4 => "pci-function",
        _ => "unknown",
    }
}

fn peer_holders(
    inspector: &ObjectInspector,
    tasks: Option<&TaskInspector>,
    peer: Koid,
    out: &mut impl Write,
) -> io::Result<()> {
    let Some(tasks) = tasks else {
        return writeln!(out, "  peer-holders: task inspector unavailable");
    };
    let mut count = 0;
    for process in crate::query::processes(tasks)? {
        let result = crate::query::scan(
            |cursor| inspector.scan_handles(process.koid, cursor),
            |handle| {
                if handle.object_koid == peer {
                    writeln!(
                        out,
                        "  peer-holder: process 0x{:016x} {:?}; handle 0x{:016x}",
                        process.koid.get(),
                        process.name.as_str(),
                        handle.handle
                    )?;
                    count += 1;
                }
                Ok(())
            },
        );
        match result {
            Err(error) if crate::query::process_unavailable(&error) => (),
            other => other?,
        }
    }
    if count == 0 {
        writeln!(
            out,
            "  peer-holders: none visible (may be closed, in transit, or outside scope)"
        )?;
    }
    Ok(())
}
