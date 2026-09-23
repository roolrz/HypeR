// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::{Parser, Subcommand};
use hyper_vm_policy::fleet::{Action, Definition, Request};

#[derive(Debug, Parser)]
#[command(
    about = "Manage named virtual machines",
    after_help = "Examples:\n  vmm list\n  vmm start alpine\n  vmm console alpine\n  vmm create test --image /vm/alpine.itb"
)]
pub struct Vmm {
    #[command(subcommand)]
    pub command: Option<VmCommand>,
}

#[derive(Debug, Subcommand)]
pub enum VmCommand {
    /// List every configured VM.
    List,
    /// Show one VM's state and image.
    Status { name: String },
    /// Start a stopped VM.
    Start { name: String },
    /// Stop a VM and retire its resources.
    Stop { name: String },
    /// Stop and then start the named VM.
    Restart { name: String },
    /// Set a vCPU allowed CPU list, for example 0,2-3 (no automatic balancing).
    Affinity {
        name: String,
        vcpu: u32,
        cpus: String,
    },
    /// Attach to the named VM's serial console.
    Console { name: String },
    /// Create a temporary definition in the running manager.
    Create {
        name: String,
        #[arg(long)]
        image: String,
        #[arg(long)]
        start: bool,
        /// Board-authorized exclusive disk volume.
        #[arg(long, requires = "disk_client")]
        disk_volume: Option<String>,
        #[arg(long, requires = "disk_volume", value_parser = clap::value_parser!(u32).range(1..=127))]
        disk_client: Option<u32>,
    },
    /// Remove a stopped definition (does not delete its image or edit the config file).
    Delete { name: String },
}
impl VmCommand {
    pub fn request(self) -> Result<Request, Box<dyn std::error::Error>> {
        let (name, action) = match self {
            Self::Affinity { name, vcpu, cpus } => {
                return Ok(Request::Affinity {
                    name,
                    vcpu,
                    affinity_words: parse_cpu_list(&cpus).map_err(std::io::Error::other)?,
                });
            }
            Self::List => return Ok(Request::List),
            Self::Create {
                name,
                image,
                start,
                disk_volume,
                disk_client,
            } => {
                let definition = Definition {
                    name,
                    image,
                    autostart: start,
                    disk: disk_volume
                        .zip(disk_client)
                        .map(|(volume, client)| hyper_vm_policy::fleet::Disk { client, volume }),
                };
                definition.validate().map_err(std::io::Error::other)?;
                return Ok(Request::Create {
                    definitions: vec![definition],
                });
            }
            Self::Status { name } => (name, Action::Status),
            Self::Start { name } => (name, Action::Start),
            Self::Stop { name } => (name, Action::Stop),
            Self::Restart { name } => (name, Action::Restart),
            Self::Console { name } => (name, Action::Console),
            Self::Delete { name } => (name, Action::Delete),
        };
        Ok(Request::Control { name, action })
    }
}

fn parse_cpu_list(value: &str) -> Result<Vec<u64>, String> {
    let max = hyper_os::vm::VCPU_AFFINITY_MAX_WORDS * 64;
    let mut words = vec![0; hyper_os::vm::VCPU_AFFINITY_MAX_WORDS];
    let parse = |part: &str| -> Result<usize, String> {
        if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err("CPU list must contain comma-separated IDs or ranges, e.g. 0,2-3".into());
        }
        part.parse::<usize>()
            .ok()
            .filter(|cpu| *cpu < max)
            .ok_or_else(|| format!("CPU ID must be below {max}"))
    };
    for item in value.split(',') {
        let (first, last) = match item.split_once('-') {
            Some((first, last)) => (parse(first)?, parse(last)?),
            None => {
                let cpu = parse(item)?;
                (cpu, cpu)
            }
        };
        if first > last {
            return Err("CPU range must be ascending".into());
        }
        for cpu in first..=last {
            words[cpu / 64] |= 1 << (cpu % 64);
        }
    }
    Ok(words)
}

#[cfg(test)]
#[path = "../tests/cli.rs"]
mod tests;
