// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Board-authored volume identities shared by the Native and Linux supervisors.

use hyper_vm_image::guest_fdt::io::DmaRange;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Client {
    pub id: u32,
    pub volume: String,
}

pub fn parse(bytes: &str) -> Result<Vec<Client>, &'static str> {
    let mut lines = bytes.lines();
    if bytes.len() > 8192 || lines.next() != Some("hyper.clients.v1") {
        return Err("invalid I/O client table header or size");
    }
    let mut clients = Vec::new();
    let mut config = false;
    for line in lines {
        let (id, volume) = line.split_once(' ').ok_or("invalid I/O client entry")?;
        let id: u32 = id.parse().map_err(|_| "invalid I/O client identity")?;
        if id == 0 {
            if config || volume != "config" {
                return Err("invalid configuration client");
            }
            config = true;
            continue;
        }
        if hyper_service::io::encode_connect(id, volume).is_none()
            || clients
                .iter()
                .any(|client: &Client| client.id == id || client.volume == volume)
        {
            return Err("invalid or duplicate I/O client");
        }
        clients.push(Client {
            id,
            volume: volume.into(),
        });
    }
    if !config {
        return Err("configuration client is required");
    }
    Ok(clients)
}

/// The Native connection owner supplies identity and notification epoch; a
/// runtime can request device operations but cannot choose a new DMA session.
pub fn authorize_request(
    request: hyper_vm_runtime::io_protocol::Request,
    identity: u64,
    epoch: u32,
) -> bool {
    use hyper_vm_runtime::io_protocol::Command;
    request.binding == identity
        && request.epoch == epoch
        && matches!(request.command, Command::Hello | Command::Device(_))
}

pub fn dynamic_dma_ranges(excluded: &[DmaRange]) -> Result<Vec<DmaRange>, String> {
    let mut excluded = excluded.to_vec();
    excluded.sort_by_key(|range| range.dma_base);
    let mut cursor = 0;
    let mut ranges = Vec::new();
    for range in excluded {
        let end = range
            .dma_base
            .checked_add(range.size)
            .ok_or("DMA range overflow")?;
        if range.dma_base < cursor || end > hyper_os::vm::DYNAMIC_PHYSICAL_LIMIT {
            return Err("invalid static DMA ranges".into());
        }
        if range.dma_base > cursor {
            ranges.push(DmaRange {
                dma_base: cursor,
                cpu_base: hyper_os::vm::DYNAMIC_ALIAS_OFFSET + cursor,
                size: range.dma_base - cursor,
            });
        }
        cursor = end;
    }
    if cursor < hyper_os::vm::DYNAMIC_PHYSICAL_LIMIT {
        ranges.push(DmaRange {
            dma_base: cursor,
            cpu_base: hyper_os::vm::DYNAMIC_ALIAS_OFFSET + cursor,
            size: hyper_os::vm::DYNAMIC_PHYSICAL_LIMIT - cursor,
        });
    }
    Ok(ranges)
}

#[cfg(test)]
#[path = "../tests/clients.rs"]
mod tests;
