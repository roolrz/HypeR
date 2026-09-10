// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::{Parser, Subcommand};
use hyper_vm_policy::fleet::{Action, Definition, Request};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    about = "Manage named virtual machines",
    after_help = "Examples:\n  vmm list\n  vmm start alpine\n  vmm console alpine\n  vmm create test --image /vm/alpine.itb\n  vmm load /etc/hyper/vms.json"
)]
pub struct Vmm {
    #[command(subcommand)]
    pub command: Option<VmCommand>,
}

#[derive(Debug, Subcommand)]
pub enum VmCommand {
    /// List every configured VM.
    List,
    /// Save definitions to a new config file without overwriting an existing file.
    Save { path: PathBuf },
    /// Show one VM's state and image.
    Status { name: String },
    /// Start a stopped VM.
    Start { name: String },
    /// Stop a VM and retire its resources.
    Stop { name: String },
    /// Stop and then start the named VM.
    Restart { name: String },
    /// Attach to the named VM's serial console.
    Console { name: String },
    /// Create an in-memory definition; use 'vmm save' to write it to a config file.
    Create {
        name: String,
        #[arg(long)]
        image: String,
        #[arg(long)]
        start: bool,
    },
    /// Remove a stopped definition (does not delete its image or edit the config file).
    Delete { name: String },
    /// Import definitions from a JSON config; existing names are rejected.
    Load { path: PathBuf },
}
impl VmCommand {
    pub fn request(self) -> Result<Request, Box<dyn std::error::Error>> {
        let (name, action) = match self {
            Self::List | Self::Save { .. } => return Ok(Request::List),
            Self::Create { name, image, start } => {
                let definition = Definition {
                    name,
                    image,
                    autostart: start,
                };
                definition.validate().map_err(std::io::Error::other)?;
                return Ok(Request::Create {
                    definitions: vec![definition],
                });
            }
            Self::Load { path } => {
                use std::io::Read;
                let mut bytes = Vec::new();
                std::fs::File::open(path)?
                    .take(hyper_vm_policy::fleet::MAX_CONFIG_BYTES + 1)
                    .read_to_end(&mut bytes)?;
                let config =
                    hyper_vm_policy::fleet::Config::parse(&bytes).map_err(std::io::Error::other)?;
                return Ok(Request::Create {
                    definitions: config.machines,
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

#[cfg(test)]
#[path = "../tests/cli.rs"]
mod tests;
