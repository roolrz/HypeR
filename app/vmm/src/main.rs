// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Unified VM management and virtual-console client.

use std::mem::MaybeUninit;

use clap::Parser;
use hyper_os::capability_channel::{CapabilityChannel, CapabilityReceiveSlot};
use hyper_os::handle::{ByteChannelObject, OwnedHandle};
use hyper_os::startup::Startup;
use hyper_os::wait::{ObjectSignals, WaitItem, wait_many};
use hyper_service::vm;
use hyper_vm_policy::fleet::{self, Action, Request, Response};
use std::io::Write;
use std::process::ExitCode;

const ESCAPE: u8 = 0x1d;

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
    hyper_os::require_core_abi()?;
    let input = hyper_rt::process::stdin()?;
    let mut output = std::io::stdout().lock();
    let control = startup.take(vm::CLIENT_CONTROL)?;
    let capabilities = CapabilityChannel::from_handle(startup.take(vm::CLIENT_CAPABILITIES)?);
    let args = hyper_vmm::cli::Vmm::parse();
    let command = args.command.unwrap_or(hyper_vmm::cli::VmCommand::List);
    let save_path = match &command {
        hyper_vmm::cli::VmCommand::Save { path } => Some(path.clone()),
        _ => None,
    };
    let command = command.request()?;
    let bytes = fleet::encode(&command).map_err(std::io::Error::other)?;
    control.as_byte_channel().send(&bytes)?;
    let response = receive_response(&control)?;
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
        let console_channel = receive_console(&capabilities)?;
        writeln!(
            output,
            "Connected to {name}. Press Ctrl-] for the control menu.\r"
        )?;
        return console_session(input, &mut output, &console_channel);
    }
    if let Some(path) = save_path {
        let Response::Entries { machines } = response else {
            return Err(std::io::Error::other("invalid list response").into());
        };
        let definitions = machines
            .into_iter()
            .map(|machine| fleet::Definition {
                name: machine.name,
                image: machine.image,
                autostart: machine.autostart,
            })
            .collect();
        hyper_vmm::save_config(&path, definitions)?;
        writeln!(
            output,
            "Saved {} (ramfs changes last until reboot).",
            path.display()
        )?;
        return Ok(());
    }
    write_response(&mut output, response)
}

fn receive_response(
    control: &OwnedHandle<ByteChannelObject>,
) -> Result<Response, Box<dyn std::error::Error>> {
    let mut bytes = vec![0; fleet::MAX_MESSAGE_BYTES];
    let length = control.as_byte_channel().receive(&mut bytes)?;
    fleet::response(&bytes[..length]).map_err(|error| std::io::Error::other(error).into())
}

fn receive_console(
    capabilities: &CapabilityChannel,
) -> hyper_os::Result<OwnedHandle<ByteChannelObject>> {
    let mut bytes = [MaybeUninit::<u8>::uninit(); vm::MESSAGE_BYTES];
    let mut slots = [CapabilityReceiveSlot::new::<ByteChannelObject>(
        vm::CONSOLE_SESSION_RIGHTS,
    )];
    let message = capabilities.receive(hyper_os::DEADLINE_INFINITE, &mut bytes, &mut slots)?;
    if vm::ConsoleCapability::decode(message.bytes()).is_none() || message.capability_count() != 1 {
        return Err(hyper_os::Error::InvalidResponse);
    }
    slots[0]
        .take::<ByteChannelObject>()?
        .ok_or(hyper_os::Error::MissingHandle)
}

fn write_response(
    output: &mut impl Write,
    response: Response,
) -> Result<(), Box<dyn std::error::Error>> {
    match response {
        Response::Accepted => writeln!(output, "accepted")?,
        Response::Error { message } => return Err(std::io::Error::other(message).into()),
        Response::Entries { mut machines } => {
            machines.sort_by(|a, b| a.name.cmp(&b.name));
            writeln!(
                output,
                "NAME                              STATE      AUTOSTART  IMAGE"
            )?;
            if machines.is_empty() {
                writeln!(
                    output,
                    "(no virtual machines; use 'vmm create NAME --image PATH')"
                )?;
            }
            for machine in machines {
                writeln!(
                    output,
                    "{:<32}  {:<9}  {:<9}  {}",
                    machine.name,
                    machine.state,
                    if machine.autostart { "yes" } else { "no" },
                    machine.image
                )?;
            }
        }
    }
    Ok(())
}

fn console_session(
    input: &OwnedHandle<ByteChannelObject>,
    output: &mut impl Write,
    console_channel: &OwnedHandle<ByteChannelObject>,
) -> Result<(), Box<dyn std::error::Error>> {
    let waits = [
        WaitItem::new(
            input.as_handle_ref(),
            ObjectSignals::<ByteChannelObject>::READABLE
                .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED),
        ),
        WaitItem::new(
            console_channel.as_handle_ref(),
            ObjectSignals::<ByteChannelObject>::READABLE
                .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED),
        ),
    ];
    let mut bytes = [0u8; hyper_os::channel::MAX_MESSAGE_BYTES];
    let mut menu = false;
    loop {
        // stdout is line-buffered. Publish prompts, key echoes and a trailing
        // carriage return before waiting for more input from either peer.
        output.flush()?;
        let observation = match wait_many(&waits, hyper_os::DEADLINE_INFINITE) {
            Ok(observation) => observation,
            Err(error) => return console_error(output, b"wait", error),
        };
        if observation.index == 1 {
            if ObjectSignals::<ByteChannelObject>::READABLE.is_present_in(observation.observed) {
                let count = match console_channel.as_byte_channel().try_receive(&mut bytes) {
                    Ok(count) => count,
                    Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK)) => continue,
                    Err(error) => return console_error(output, b"read", error),
                };
                output.write_all(&bytes[..count])?;
                continue;
            }
            output.write_all(b"\r\n[vmm] virtual machine disconnected\r\n")?;
            return Ok(());
        }
        if observation.index != 0
            || !ObjectSignals::<ByteChannelObject>::READABLE.is_present_in(observation.observed)
        {
            return Ok(());
        }
        let count = match input.as_byte_channel().try_receive(&mut bytes) {
            Ok(count) => count,
            Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK)) => continue,
            Err(error) => return console_error(output, b"input", error),
        };
        let mut guest_input = Vec::with_capacity(count);
        for byte in bytes[..count].iter().copied() {
            if menu {
                menu = false;
                match byte {
                    b'd' | b'q' | ESCAPE => {
                        output.write_all(b"\r\n[vmm] detached\r\n")?;
                        return Ok(());
                    }
                    _ => {
                        output.write_all(b"\r\n[vmm] resumed\r\n")?;
                    }
                }
                continue;
            }
            if byte == ESCAPE {
                if !guest_input.is_empty() {
                    if let Err(error) = console_channel.as_byte_channel().send(&guest_input) {
                        return console_error(output, b"write", error);
                    }
                    guest_input.clear();
                }
                menu = true;
                output.write_all(b"\r\n[vmm] d/q: detach, any other key: resume\r\n")?;
                continue;
            }
            guest_input.push(byte);
        }
        if !guest_input.is_empty()
            && let Err(error) = console_channel.as_byte_channel().send(&guest_input)
        {
            return console_error(output, b"write", error);
        }
    }
}

fn console_error(
    output: &mut impl Write,
    operation: &[u8],
    error: hyper_os::Error,
) -> Result<(), Box<dyn std::error::Error>> {
    let _ = output.write_all(b"\r\n[vmm] console ");
    let _ = output.write_all(operation);
    let _ = output.write_all(b" failed: ");
    let reason: &[u8] = match error {
        hyper_os::Error::Status(hyper_os::Status::ACCESS_DENIED) => b"access denied",
        hyper_os::Error::Status(hyper_os::Status::BAD_HANDLE) => b"bad handle",
        hyper_os::Error::Status(hyper_os::Status::BAD_STATE) => b"bad state",
        hyper_os::Error::Status(hyper_os::Status::BUSY) => b"busy",
        hyper_os::Error::Status(hyper_os::Status::FAULT) => b"fault",
        hyper_os::Error::Status(hyper_os::Status::INVALID_ARGUMENT) => b"invalid argument",
        hyper_os::Error::Status(hyper_os::Status::PEER_CLOSED) => b"disconnected",
        hyper_os::Error::Status(_) => b"kernel error",
        _ => b"invalid response",
    };
    let _ = output.write_all(reason);
    let _ = output.write_all(b"\r\n");
    Err(error.into())
}

fn main() -> ExitCode {
    match hyper_rt::process::startup() {
        Ok(startup) => application_main(startup),
        Err(_) => ExitCode::FAILURE,
    }
}
