// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use hyper_vm_policy::fleet::{self, Response};
use std::io::Write;

pub(crate) fn write_response(
    output: &mut impl Write,
    response: Response,
) -> Result<(), Box<dyn std::error::Error>> {
    match response {
        Response::AffinityAccepted { vcpu } => writeln!(
            output,
            "vCPU {vcpu}: affinity accepted; inspect vmm status for placement"
        )?,
        Response::Accepted => writeln!(output, "accepted")?,
        Response::Error { message } => return Err(std::io::Error::other(message).into()),
        Response::Entries { mut machines } => {
            machines.sort_by(|a, b| a.name.cmp(&b.name));
            writeln!(
                output,
                "NAME                              STATE      AUTOSTART  ACCESS      IMAGE"
            )?;
            if machines.is_empty() {
                writeln!(
                    output,
                    "(no virtual machines; use 'vmm create NAME --config PATH')"
                )?;
            }
            for machine in machines {
                writeln!(
                    output,
                    "{:<32}  {:<11}  {:<9}  {:<10}  {}",
                    machine.name,
                    machine.state,
                    if machine.autostart { "yes" } else { "no" },
                    if machine.read_only {
                        "read-only"
                    } else {
                        "managed"
                    },
                    machine.image
                )?;
                if let Some(network) = &machine.network {
                    writeln!(
                        output,
                        "  network: {}; MAC: {}; I/O client: {}",
                        network.network, network.mac, network.client
                    )?;
                }
                for placement in &machine.placement {
                    writeln!(
                        output,
                        "  vCPU {}: pCPU {}",
                        placement.vcpu,
                        placement
                            .host_cpu
                            .map_or_else(|| "unassigned".into(), |cpu| cpu.to_string())
                    )?;
                }
                if let (Some(vcpus), Some(bytes)) = (machine.vcpus, machine.memory_bytes) {
                    writeln!(
                        output,
                        "  vCPUs: {vcpus}; RAM capacity: {} MiB",
                        bytes / (1024 * 1024)
                    )?;
                    if let Some(resident) = machine.resident_memory_bytes {
                        writeln!(
                            output,
                            "  allocated VM backing: {resident} bytes ({} KiB)",
                            resident / 1024
                        )?;
                    } else {
                        writeln!(output, "  allocated VM backing: unavailable")?;
                    }
                } else if !matches!(machine.state, fleet::State::Stopped | fleet::State::Failed) {
                    writeln!(output, "  VM metrics: unavailable")?;
                }
            }
        }
    }
    Ok(())
}
