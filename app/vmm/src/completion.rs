// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use hyper_vm_policy::fleet::{Response, State};
use std::time::Duration;

pub struct Completion {
    pub name: String,
    pub target: State,
    pub timeout: Duration,
}

impl Completion {
    /// Check a status response, never confusing admission with completion.
    /// A running VM means vCPUs have started; it does not imply guest OS readiness.
    pub fn observe(&self, response: &Response) -> Result<bool, String> {
        let entries = match response {
            Response::Entries { machines } => machines,
            Response::Error { message } => return Err(message.clone()),
            _ => return Err("invalid VM status response".into()),
        };
        let [machine] = entries.as_slice() else {
            return Err("invalid VM status entry count".into());
        };
        if machine.name != self.name {
            return Err("VM status name mismatch".into());
        }
        match machine.state {
            State::Failed | State::Unavailable => Err(format!(
                "VM '{}' became {}; inspect vmm status and runtime logs",
                self.name, machine.state
            )),
            state => Ok(state == self.target),
        }
    }
}

#[cfg(test)]
#[path = "../tests/completion.rs"]
mod tests;
