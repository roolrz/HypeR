// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Unified VM management and virtual-console client.

// Select the shared parser implementation for Native delivery.
#[cfg(target_os = "hyper")]
extern crate hyper_clap_shared as _;

#[cfg(target_os = "hyper")]
extern crate hyper_vm_policy_shared as hyper_vm_policy;

mod output;
mod session;
mod transport;

use std::mem::MaybeUninit;

use clap::Parser;
use hyper_os::capability_channel::{CapabilityChannel, CapabilityReceiveSlot};
use hyper_os::handle::{ByteChannelObject, OwnedHandle};
use hyper_os::startup::Startup;
use hyper_service::vm;
use hyper_vm_policy::fleet::{Action, Request, Response};
use std::io::Write;
use std::process::ExitCode;

fn application_main(mut startup: Startup<'_>) -> ExitCode {
    match run(&mut startup) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("vmm: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(startup: &mut Startup<'_>) -> Result<(), Box<dyn std::error::Error>> {
    let args = hyper_vmm::cli::Vmm::parse();
    hyper_os::require_core_abi()?;
    let mut output = std::io::stdout().lock();
    let control = startup.take(vm::CLIENT_CONTROL)?;
    let capabilities = CapabilityChannel::from_handle(startup.take(vm::CLIENT_CAPABILITIES)?);
    let command = args.command.unwrap_or(hyper_vmm::cli::VmCommand::List);
    let completion = command.completion();
    let timeout = completion
        .as_ref()
        .map_or(std::time::Duration::from_secs(30), |wait| wait.timeout);
    let deadline = hyper_os::time::deadline_after(timeout)?.as_raw();
    let command = command.request()?;
    let response = transport::exchange(&control, &command, deadline).map_err(command_error)?;
    if let Response::Error { message } = response {
        return Err(std::io::Error::other(message).into());
    }
    if let Request::Control {
        name,
        action: Action::Console,
    } = &command
    {
        if !matches!(response, Response::Accepted) {
            return Err(std::io::Error::other("invalid console response").into());
        }
        let console_channel = receive_console(&capabilities, deadline)?;
        writeln!(
            output,
            "Connected to {name}. Press Ctrl-] for the control menu."
        )?;
        return session::console_session(
            hyper_rt::process::stdin()?,
            &mut output,
            &console_channel,
        );
    }
    if let Some(completion) = completion {
        if !matches!(response, Response::Accepted) {
            return Err(std::io::Error::other("invalid lifecycle response").into());
        }
        transport::complete(&control, &completion, deadline).map_err(command_error)?;
        writeln!(output, "{}: {}", completion.name, completion.target)?;
        Ok(())
    } else {
        output::write_response(&mut output, response)
    }
}

fn command_error(error: Box<dyn std::error::Error>) -> Box<dyn std::error::Error> {
    if matches!(
        error.downcast_ref::<hyper_os::Error>(),
        Some(hyper_os::Error::Status(hyper_os::Status::TIMED_OUT))
    ) {
        std::io::Error::new(std::io::ErrorKind::TimedOut,
            "deadline expired; operation may still be in progress (not cancelled); inspect vmm status").into()
    } else {
        error
    }
}

fn receive_console(
    capabilities: &CapabilityChannel,
    deadline: u64,
) -> hyper_os::Result<OwnedHandle<ByteChannelObject>> {
    let mut bytes = [MaybeUninit::<u8>::uninit(); vm::MESSAGE_BYTES];
    let mut slots = [CapabilityReceiveSlot::new::<ByteChannelObject>(
        vm::CONSOLE_SESSION_RIGHTS,
    )];
    let message = capabilities.receive(deadline, &mut bytes, &mut slots)?;
    if vm::ConsoleCapability::decode(message.bytes()).is_none() || message.capability_count() != 1 {
        return Err(hyper_os::Error::InvalidResponse);
    }
    slots[0]
        .take::<ByteChannelObject>()?
        .ok_or(hyper_os::Error::MissingHandle)
}

fn main() -> ExitCode {
    match hyper_rt::process::startup() {
        Ok(startup) => application_main(startup),
        Err(_) => ExitCode::FAILURE,
    }
}
