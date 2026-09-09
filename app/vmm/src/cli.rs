// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(about = "Manage the default virtual machine")]
pub struct Vmm {
    #[command(subcommand)]
    pub command: Option<VmCommand>,
}

#[derive(Debug, Subcommand)]
pub enum VmCommand {
    List,
    Status,
    Start,
    Stop,
    Restart,
    Console,
}

impl From<VmCommand> for hyper_service::vm::FleetCommand {
    fn from(value: VmCommand) -> Self {
        match value {
            VmCommand::List => Self::List,
            VmCommand::Status => Self::Status,
            VmCommand::Start => Self::Start,
            VmCommand::Stop => Self::Stop,
            VmCommand::Restart => Self::Restart,
            VmCommand::Console => Self::Console,
        }
    }
}

#[cfg(test)]
#[path = "../tests/cli.rs"]
mod tests;
