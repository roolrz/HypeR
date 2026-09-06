// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Initial Native command shell and capability-scoped command launcher.

#![no_std]
#![no_main]

mod command;

use command::{CommandLine, MAX_LINE_BYTES};
use hyper_os::bootfs::{BootFile, BootFileRights, BootFs};
use hyper_os::channel;
use hyper_os::handle::{
    ByteChannelObject, ObjectInspectorObject, OwnedHandle, ProcessObject, ResourceDomainObject,
    Rights, RightsOffer, TaskFactoryObject, TaskGroupObject, TaskInspectorObject,
};
use hyper_os::startup::{self, Startup};
use hyper_os::task::{ProcessBuilder, ProcessInfo, ProcessTermination};
use hyper_os::wait::{ObjectSignals, WaitItem, wait_many};
use hyper_os::{Error as OsError, Status};
use hyper_rt::ExitCode;
use hyper_service::stdio;

const INPUT_CHUNK_BYTES: usize = 256;
const COMMAND_PATH_BYTES: usize = MAX_LINE_BYTES + 5;
const READY_MESSAGE: &[u8] = b"HypeR session: console ready\n";
const PROMPT: &[u8] = b"hyper> ";

fn application_main(mut startup: Startup<'_>) -> ExitCode {
    match run(&mut startup) {
        Ok(code) => code,
        Err(_) => ExitCode::FAILURE,
    }
}

fn run(startup: &mut Startup<'_>) -> Result<ExitCode, Error> {
    hyper_os::require_core_abi().map_err(Error::from)?;
    let input = startup.take(stdio::STANDARD_INPUT).map_err(Error::from)?;
    let output = startup.take(stdio::STANDARD_OUTPUT).map_err(Error::from)?;
    let error = startup.take(stdio::STANDARD_ERROR).map_err(Error::from)?;
    let boot_fs = startup.take_boot_fs().map_err(Error::from)?;
    let factory = startup.take(startup::TASK_FACTORY).map_err(Error::from)?;
    let group = startup.take(startup::TASK_GROUP).map_err(Error::from)?;
    let domain = startup
        .take(startup::RESOURCE_DOMAIN)
        .map_err(Error::from)?;
    let task_inspector = startup.take(startup::TASK_INSPECTOR).map_err(Error::from)?;
    let object_inspector = startup
        .take(startup::OBJECT_INSPECTOR)
        .map_err(Error::from)?;
    let authorities = CommandAuthorities {
        boot_fs,
        factory,
        group,
        domain,
        task_inspector,
        object_inspector,
    };

    write(&output, READY_MESSAGE)?;
    write(&output, PROMPT)?;

    let mut line = [0_u8; MAX_LINE_BYTES];
    let mut line_length = 0;
    let mut discard_line = false;
    let mut previous_was_carriage_return = false;
    let mut input_chunk = [0_u8; INPUT_CHUNK_BYTES];
    loop {
        let count = input
            .as_byte_channel()
            .receive(&mut input_chunk)
            .map_err(Error::from)?;
        let bytes = input_chunk.get(..count).ok_or(Error::Protocol)?;
        for byte in bytes.iter().copied() {
            if byte == b'\n' && previous_was_carriage_return {
                previous_was_carriage_return = false;
                continue;
            }
            previous_was_carriage_return = byte == b'\r';
            match byte {
                b'\r' | b'\n' => {
                    write(&output, b"\r\n")?;
                    if discard_line {
                        write(&error, b"sh: command line is too long\n")?;
                    } else if line_length != 0 {
                        let command = line.get(..line_length).ok_or(Error::Protocol)?;
                        match execute_line(command, &authorities, &input, &output, &error)? {
                            CommandFlow::Continue => {}
                            CommandFlow::Exit => return Ok(ExitCode::SUCCESS),
                        }
                    }
                    line_length = 0;
                    discard_line = false;
                    write(&output, PROMPT)?;
                }
                0x03 => {
                    line_length = 0;
                    discard_line = false;
                    write(&output, b"^C\r\n")?;
                    write(&output, PROMPT)?;
                }
                0x04 if line_length == 0 => return Ok(ExitCode::SUCCESS),
                0x08 | 0x7f if !discard_line && line_length != 0 => {
                    line_length -= 1;
                    write(&output, b"\x08 \x08")?;
                }
                byte if byte == b'\t' || byte >= 0x20 => {
                    if discard_line {
                        continue;
                    }
                    let Some(slot) = line.get_mut(line_length) else {
                        discard_line = true;
                        continue;
                    };
                    *slot = byte;
                    line_length = line_length.checked_add(1).ok_or(Error::Protocol)?;
                    write(&output, core::slice::from_ref(&byte))?;
                }
                _ => {}
            }
        }
    }
}

fn execute_line(
    bytes: &[u8],
    authorities: &CommandAuthorities,
    input: &OwnedHandle<ByteChannelObject>,
    output: &OwnedHandle<ByteChannelObject>,
    error: &OwnedHandle<ByteChannelObject>,
) -> Result<CommandFlow, Error> {
    let command = match CommandLine::parse(bytes) {
        Ok(command) => command,
        Err(_) => {
            write(error, b"sh: invalid command syntax\n")?;
            return Ok(CommandFlow::Continue);
        }
    };
    if command.is_empty() {
        return Ok(CommandFlow::Continue);
    }
    let Some(name) = command.argument(0) else {
        write(error, b"sh: command is not valid UTF-8\n")?;
        return Ok(CommandFlow::Continue);
    };
    match name {
        "help" => write(
            output,
            b"builtins: clear echo exit help\nexternal commands: /bin/echo /bin/handle /bin/ps\n",
        )?,
        "echo" => builtin_echo(&command, output)?,
        "clear" => write(output, b"\x1b[2J\x1b[H")?,
        "exit" => return Ok(CommandFlow::Exit),
        _ => launch_command(&command, authorities, input, output, error)?,
    }
    Ok(CommandFlow::Continue)
}

fn builtin_echo(
    command: &CommandLine,
    output: &OwnedHandle<ByteChannelObject>,
) -> Result<(), Error> {
    for index in 1..command.len() {
        if index != 1 {
            write(output, b" ")?;
        }
        let argument = command.argument(index).ok_or(Error::InvalidCommand)?;
        write(output, argument.as_bytes())?;
    }
    write(output, b"\n")
}

fn launch_command(
    command: &CommandLine,
    authorities: &CommandAuthorities,
    input: &OwnedHandle<ByteChannelObject>,
    output: &OwnedHandle<ByteChannelObject>,
    error: &OwnedHandle<ByteChannelObject>,
) -> Result<(), Error> {
    let name = command.argument(0).ok_or(Error::InvalidCommand)?;
    let executable = match open_command(&authorities.boot_fs, name) {
        Ok(executable) => executable,
        Err(_) => {
            write(error, b"sh: command not found\n")?;
            return Ok(());
        }
    };
    let builder = ProcessBuilder::create(
        authorities.factory.as_handle_ref(),
        authorities.group.as_handle_ref(),
        authorities.domain.as_handle_ref(),
        executable.as_handle_ref(),
    )
    .map_err(Error::from)?;
    builder.set_name(name).map_err(Error::from)?;
    for index in 0..command.len() {
        builder
            .add_argument(command.argument(index).ok_or(Error::InvalidCommand)?)
            .map_err(Error::from)?;
    }

    let (parent_input, child_input) = channel::create_pair().map_err(Error::from)?;
    let (child_output, parent_output) = channel::create_pair().map_err(Error::from)?;
    let (child_error, parent_error) = channel::create_pair().map_err(Error::from)?;
    add_child_channel(
        &builder,
        child_input,
        stdio::STANDARD_INPUT.as_raw(),
        Rights::WAIT.union(Rights::READ),
    )?;
    add_child_channel(
        &builder,
        child_output,
        stdio::STANDARD_OUTPUT.as_raw(),
        Rights::WAIT.union(Rights::WRITE),
    )?;
    match name {
        "ps" | "/bin/ps" => builder
            .add_handle_duplicate(
                authorities.task_inspector.as_handle_ref(),
                startup::TASK_INSPECTOR.as_raw(),
                RightsOffer::Exact(Rights::INSPECT),
            )
            .map_err(|_| Error::InvalidCommand)?,
        "handle" | "/bin/handle" => builder
            .add_handle_duplicate(
                authorities.object_inspector.as_handle_ref(),
                startup::OBJECT_INSPECTOR.as_raw(),
                RightsOffer::Exact(Rights::INSPECT),
            )
            .map_err(|_| Error::InvalidCommand)?,
        _ => {}
    }
    add_child_channel(
        &builder,
        child_error,
        stdio::STANDARD_ERROR.as_raw(),
        Rights::WAIT.union(Rights::WRITE),
    )?;
    builder.seal().map_err(Error::from)?;
    let process = builder
        .start()
        .map_err(|failure| Error::from(failure.error()))?;
    supervise_command(
        process,
        ChildChannels {
            input: parent_input,
            output: parent_output,
            error: parent_error,
        },
        ShellChannels {
            input,
            output,
            error,
        },
    )
}

fn add_child_channel(
    builder: &ProcessBuilder,
    channel: OwnedHandle<ByteChannelObject>,
    purpose: u32,
    rights: Rights,
) -> Result<(), Error> {
    builder
        .add_handle_move(channel, purpose, RightsOffer::Exact(rights))
        .map_err(|failure| Error::from(failure.error()))
}

fn supervise_command(
    process: OwnedHandle<ProcessObject>,
    child: ChildChannels,
    shell: ShellChannels<'_>,
) -> Result<(), Error> {
    let readable = ObjectSignals::<ByteChannelObject>::READABLE;
    let shell_readable = readable.union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED);
    let terminated = ObjectSignals::<ProcessObject>::TERMINATED;
    let waits = [
        WaitItem::new(child.output.as_handle_ref(), readable),
        WaitItem::new(child.error.as_handle_ref(), readable),
        WaitItem::new(shell.input.as_handle_ref(), shell_readable),
        WaitItem::new(process.as_handle_ref(), terminated),
    ];
    let mut buffer = [0_u8; channel::MAX_MESSAGE_BYTES];
    loop {
        let observation = wait_many(&waits, hyper_os::DEADLINE_INFINITE).map_err(Error::from)?;
        match observation.index {
            0 => route_message(&child.output, shell.output, &mut buffer)?,
            1 => route_message(&child.error, shell.error, &mut buffer)?,
            2 if ObjectSignals::<ByteChannelObject>::READABLE
                .is_present_in(observation.observed) =>
            {
                route_message(shell.input, &child.input, &mut buffer)?;
            }
            2 => {
                let _ = process.as_process_supervisor().request_stop();
                return Err(Error::InputClosed);
            }
            3 => break,
            _ => return Err(Error::Protocol),
        }
    }
    drain_channel(&child.output, shell.output, &mut buffer)?;
    drain_channel(&child.error, shell.error, &mut buffer)?;
    let info = process
        .as_process_supervisor()
        .info()
        .map_err(Error::from)?;
    if !process_succeeded(info) {
        write(shell.error, b"sh: command failed\n")?;
    }
    Ok(())
}

fn route_message(
    source: &OwnedHandle<ByteChannelObject>,
    destination: &OwnedHandle<ByteChannelObject>,
    buffer: &mut [u8],
) -> Result<(), Error> {
    let count = source
        .as_byte_channel()
        .receive(buffer)
        .map_err(Error::from)?;
    write(destination, buffer.get(..count).ok_or(Error::Protocol)?)
}

fn drain_channel(
    source: &OwnedHandle<ByteChannelObject>,
    destination: &OwnedHandle<ByteChannelObject>,
    buffer: &mut [u8],
) -> Result<(), Error> {
    loop {
        match source.as_byte_channel().try_receive(buffer) {
            Ok(count) => write(destination, buffer.get(..count).ok_or(Error::Protocol)?)?,
            Err(OsError::Status(status))
                if status == Status::WOULD_BLOCK || status == Status::PEER_CLOSED =>
            {
                return Ok(());
            }
            Err(error) => return Err(Error::from(error)),
        }
    }
}

fn process_succeeded(info: ProcessInfo) -> bool {
    matches!(
        info.terminal,
        Some(
            ProcessTermination::ThreadExited { status: 0 }
                | ProcessTermination::ProcessExited { status: 0 }
                | ProcessTermination::LastThreadExited { status: 0 }
        )
    )
}

fn open_command(boot_fs: &BootFs, name: &str) -> Result<BootFile, OsError> {
    if name.starts_with('/') {
        return boot_fs.open(name, BootFileRights::EXECUTE);
    }
    let mut path = [0_u8; COMMAND_PATH_BYTES];
    let prefix = b"/bin/";
    let length = prefix
        .len()
        .checked_add(name.len())
        .ok_or(OsError::InvalidPath)?;
    let destination = path.get_mut(..length).ok_or(OsError::InvalidPath)?;
    let (prefix_target, name_target) = destination.split_at_mut(prefix.len());
    prefix_target.copy_from_slice(prefix);
    name_target.copy_from_slice(name.as_bytes());
    let path = core::str::from_utf8(destination).map_err(|_| OsError::InvalidPath)?;
    boot_fs.open(path, BootFileRights::EXECUTE)
}

fn write(destination: &OwnedHandle<ByteChannelObject>, bytes: &[u8]) -> Result<(), Error> {
    destination
        .as_byte_channel()
        .send(bytes)
        .map_err(Error::from)
}

struct CommandAuthorities {
    boot_fs: BootFs,
    factory: OwnedHandle<TaskFactoryObject>,
    group: OwnedHandle<TaskGroupObject>,
    domain: OwnedHandle<ResourceDomainObject>,
    task_inspector: OwnedHandle<TaskInspectorObject>,
    object_inspector: OwnedHandle<ObjectInspectorObject>,
}

struct ChildChannels {
    input: OwnedHandle<ByteChannelObject>,
    output: OwnedHandle<ByteChannelObject>,
    error: OwnedHandle<ByteChannelObject>,
}

struct ShellChannels<'a> {
    input: &'a OwnedHandle<ByteChannelObject>,
    output: &'a OwnedHandle<ByteChannelObject>,
    error: &'a OwnedHandle<ByteChannelObject>,
}

enum CommandFlow {
    Continue,
    Exit,
}

enum Error {
    OperatingSystem,
    InvalidCommand,
    InputClosed,
    Protocol,
}

impl From<OsError> for Error {
    fn from(_error: OsError) -> Self {
        Self::OperatingSystem
    }
}

hyper_rt::entry!(application_main);
