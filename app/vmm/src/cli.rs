// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::{Parser, Subcommand};
use hyper_vm_policy::fleet::{Action, Config, Request};

#[derive(Debug, Parser)]
#[command(
    about = "Manage named virtual machines",
    after_help = "Examples:\n  vmm list\n  vmm start alpine\n  vmm console alpine\n  vmm create test --config /data/new-vms.json"
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
        /// VM configuration file containing the named definition.
        #[arg(long)]
        config: String,
        /// Use another definition in that file as the template.
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        start: bool,
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
                config,
                from,
                start,
            } => {
                use std::io::Read;
                let mut bytes = Vec::new();
                std::fs::File::open(config)?
                    .take(hyper_vm_policy::fleet::MAX_CONFIG_BYTES + 1)
                    .read_to_end(&mut bytes)?;
                let config = Config::parse(&bytes).map_err(std::io::Error::other)?;
                return create_request(config, name, from, start)
                    .map_err(|error| std::io::Error::other(error).into());
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

fn create_request(
    config: Config,
    name: String,
    from: Option<String>,
    start: bool,
) -> Result<Request, String> {
    let selected = from.as_deref().unwrap_or(&name);
    let mut definition = config
        .machines
        .into_iter()
        .find(|value| value.name == selected)
        .ok_or_else(|| format!("VM definition '{selected}' not found in configuration"))?;
    definition.name = name;
    definition.autostart = start;
    definition.validate()?;
    Ok(Request::Create {
        definitions: vec![definition],
    })
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
