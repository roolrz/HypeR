// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Unified VM management and virtual-console client.

#![no_std]
#![no_main]

use core::mem::MaybeUninit;

use hyper_os::capability_channel::{CapabilityChannel, CapabilityReceiveSlot};
use hyper_os::handle::{ByteChannelObject, OwnedHandle, VirtualSerialObject};
use hyper_os::startup::Startup;
use hyper_os::wait::{ObjectSignals, WaitItem, wait_many};
use hyper_rt::ExitCode;
use hyper_service::{stdio, vm};

const ESCAPE: u8 = 0x1d;

fn application_main(mut startup: Startup<'_>) -> ExitCode {
    match run(&mut startup) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}

fn run(startup: &mut Startup<'_>) -> hyper_os::Result<()> {
    hyper_os::require_core_abi()?;
    let input = startup.take(stdio::STANDARD_INPUT)?;
    let output = startup.take(stdio::STANDARD_OUTPUT)?;
    let error = startup.take(stdio::STANDARD_ERROR)?;
    let control = startup.take(vm::CLIENT_CONTROL)?;
    let capabilities = CapabilityChannel::from_handle(startup.take(vm::CLIENT_CAPABILITIES)?);
    let command = parse_command(startup)?;
    control.as_byte_channel().send(&command.encode())?;
    if command == vm::FleetCommand::Console {
        let response = receive_response(&control)?;
        if response != vm::FleetResponse::Accepted {
            write_response(&error, response)?;
            return Ok(());
        }
        let serial = receive_console(&capabilities)?;
        output
            .as_byte_channel()
            .send(b"Connected to default. Press Ctrl-] for the control menu.\r\n")?;
        return console_session(&input, &output, &serial);
    }
    write_response(&output, receive_response(&control)?)
}

fn parse_command(startup: &Startup<'_>) -> hyper_os::Result<vm::FleetCommand> {
    if startup.argument_count() > 2 {
        return Err(hyper_os::Error::InvalidResponse);
    }
    match startup.argument_count() {
        1 => Ok(vm::FleetCommand::List),
        2 => match startup.argument(1)? {
            "list" => Ok(vm::FleetCommand::List),
            "status" => Ok(vm::FleetCommand::Status),
            "start" => Ok(vm::FleetCommand::Start),
            "stop" => Ok(vm::FleetCommand::Stop),
            "restart" => Ok(vm::FleetCommand::Restart),
            "console" => Ok(vm::FleetCommand::Console),
            _ => Err(hyper_os::Error::InvalidResponse),
        },
        _ => Err(hyper_os::Error::InvalidResponse),
    }
}

fn receive_response(
    control: &OwnedHandle<ByteChannelObject>,
) -> hyper_os::Result<vm::FleetResponse> {
    let mut bytes = [0u8; vm::MESSAGE_BYTES];
    let length = control.as_byte_channel().receive(&mut bytes)?;
    bytes
        .get(..length)
        .and_then(vm::FleetResponse::decode)
        .ok_or(hyper_os::Error::InvalidResponse)
}

fn receive_console(
    capabilities: &CapabilityChannel,
) -> hyper_os::Result<OwnedHandle<VirtualSerialObject>> {
    let mut bytes = [MaybeUninit::<u8>::uninit(); vm::MESSAGE_BYTES];
    let mut slots = [CapabilityReceiveSlot::new::<VirtualSerialObject>(
        vm::VIRTUAL_SERIAL_SESSION_RIGHTS,
    )];
    let message = capabilities.receive(hyper_os::DEADLINE_INFINITE, &mut bytes, &mut slots)?;
    if vm::ConsoleCapability::decode(message.bytes()).is_none() || message.capability_count() != 1 {
        return Err(hyper_os::Error::InvalidResponse);
    }
    slots[0]
        .take::<VirtualSerialObject>()?
        .ok_or(hyper_os::Error::MissingHandle)
}

fn write_response(
    output: &OwnedHandle<ByteChannelObject>,
    response: vm::FleetResponse,
) -> hyper_os::Result<()> {
    let text: &[u8] = match response {
        vm::FleetResponse::State(vm::FleetState::Stopped) => b"default\tstopped\n",
        vm::FleetResponse::State(vm::FleetState::Starting) => b"default\tstarting\n",
        vm::FleetResponse::State(vm::FleetState::Running) => b"default\trunning\n",
        vm::FleetResponse::State(vm::FleetState::Stopping) => b"default\tstopping\n",
        vm::FleetResponse::State(vm::FleetState::Failed) => b"default\tfailed\n",
        vm::FleetResponse::Accepted => b"accepted\n",
        vm::FleetResponse::Busy => b"vmm: resource is busy\n",
        vm::FleetResponse::Failed => b"vmm: operation failed\n",
    };
    output.as_byte_channel().send(text)
}

fn console_session(
    input: &OwnedHandle<ByteChannelObject>,
    output: &OwnedHandle<ByteChannelObject>,
    serial: &OwnedHandle<VirtualSerialObject>,
) -> hyper_os::Result<()> {
    let waits = [
        WaitItem::new(
            input.as_handle_ref(),
            ObjectSignals::<ByteChannelObject>::READABLE
                .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED),
        ),
        WaitItem::new(
            serial.as_handle_ref(),
            ObjectSignals::<VirtualSerialObject>::READABLE
                .union(ObjectSignals::<VirtualSerialObject>::DISCONNECTED),
        ),
    ];
    let mut bytes = [0u8; hyper_os::virtual_serial::MAX_TRANSFER_BYTES];
    let mut menu = false;
    loop {
        let observation = match wait_many(&waits, hyper_os::DEADLINE_INFINITE) {
            Ok(observation) => observation,
            Err(error) => return console_error(output, b"wait", error),
        };
        if observation.index == 1 {
            if ObjectSignals::<VirtualSerialObject>::READABLE.is_present_in(observation.observed) {
                let count = match hyper_os::virtual_serial::read(serial.as_handle_ref(), &mut bytes)
                {
                    Ok(count) => count,
                    Err(error) => return console_error(output, b"read", error),
                };
                output.as_byte_channel().send(&bytes[..count])?;
                continue;
            }
            output
                .as_byte_channel()
                .send(b"\r\n[vmm] virtual machine disconnected\r\n")?;
            return Ok(());
        }
        if observation.index != 0
            || !ObjectSignals::<ByteChannelObject>::READABLE.is_present_in(observation.observed)
        {
            return Ok(());
        }
        let count = match input.as_byte_channel().receive(&mut bytes) {
            Ok(count) => count,
            Err(error) => return console_error(output, b"input", error),
        };
        for byte in bytes[..count].iter().copied() {
            if menu {
                menu = false;
                match byte {
                    b'd' | b'q' | ESCAPE => {
                        output.as_byte_channel().send(b"\r\n[vmm] detached\r\n")?;
                        return Ok(());
                    }
                    _ => {
                        output.as_byte_channel().send(b"\r\n[vmm] resumed\r\n")?;
                    }
                }
                continue;
            }
            if byte == ESCAPE {
                menu = true;
                output
                    .as_byte_channel()
                    .send(b"\r\n[vmm] d/q: detach, any other key: resume\r\n")?;
                continue;
            }
            if let Err(error) = hyper_os::virtual_serial::write(
                serial.as_handle_ref(),
                core::slice::from_ref(&byte),
            ) {
                return console_error(output, b"write", error);
            }
        }
    }
}

fn console_error(
    output: &OwnedHandle<ByteChannelObject>,
    operation: &[u8],
    error: hyper_os::Error,
) -> hyper_os::Result<()> {
    let _ = output.as_byte_channel().send(b"\r\n[vmm] console ");
    let _ = output.as_byte_channel().send(operation);
    let _ = output.as_byte_channel().send(b" failed: ");
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
    let _ = output.as_byte_channel().send(reason);
    let _ = output.as_byte_channel().send(b"\r\n");
    Err(error)
}

hyper_rt::entry!(application_main);
