// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Construct a fresh shell client without transferring the console itself.

use hyper_os::fs::{Directory, FileRights};
use hyper_os::handle::{
    ByteChannelObject, CapabilityChannelObject, OwnedHandle, ProcessObject, Rights, RightsOffer,
};
use hyper_os::startup::{self, Startup};
use hyper_os::task::ProcessBuilder;
use hyper_service::{process, stdio, vm};

pub fn launch(
    startup: &Startup<'_>,
    root: &Directory,
    image: &str,
    connection: Option<&OwnedHandle<CapabilityChannelObject>>,
    input: OwnedHandle<ByteChannelObject>,
    output: OwnedHandle<ByteChannelObject>,
    error: OwnedHandle<ByteChannelObject>,
) -> hyper_os::Result<OwnedHandle<ProcessObject>> {
    let executable = root.open(image, FileRights::EXECUTE)?;
    let builder = ProcessBuilder::create(
        startup.borrow(startup::TASK_FACTORY)?,
        startup.borrow(startup::TASK_GROUP)?,
        startup.borrow(startup::RESOURCE_DOMAIN)?,
        executable.as_handle_ref(),
    )?;
    builder.set_name("shell")?;
    builder.add_argument(image)?;
    macro_rules! delegate {
        ($purpose:expr, $contract:expr) => {
            builder.add_handle_duplicate(
                startup.borrow($purpose)?,
                $purpose.as_raw(),
                RightsOffer::Exact($contract.allowed_rights()),
            )?;
        };
    }
    builder.add_handle_duplicate(
        root.as_handle_ref(),
        startup::ROOT_DIRECTORY.as_raw(),
        RightsOffer::Exact(process::SHELL_ROOT_DIRECTORY_CONTRACT.allowed_rights()),
    )?;
    delegate!(startup::TASK_FACTORY, process::SHELL_TASK_FACTORY_CONTRACT);
    delegate!(startup::TASK_GROUP, process::SHELL_TASK_GROUP_CONTRACT);
    delegate!(
        startup::RESOURCE_DOMAIN,
        process::SHELL_RESOURCE_DOMAIN_CONTRACT
    );
    delegate!(
        startup::TASK_INSPECTOR,
        process::SHELL_TASK_INSPECTOR_CONTRACT
    );
    delegate!(
        startup::OBJECT_INSPECTOR,
        process::SHELL_OBJECT_INSPECTOR_CONTRACT
    );
    delegate!(
        startup::MEMORY_INSPECTOR,
        process::SHELL_MEMORY_INSPECTOR_CONTRACT
    );
    delegate!(
        startup::CPU_INSPECTOR,
        process::SHELL_CPU_INSPECTOR_CONTRACT
    );
    delegate!(
        process::CHILD_LIBRARY_DIRECTORY,
        process::CHILD_LIBRARY_DIRECTORY_CONTRACT
    );
    builder.add_handle_duplicate(
        startup.borrow(process::CHILD_LIBRARY_DIRECTORY)?,
        startup::DYNAMIC_LIBRARY_DIRECTORY.as_raw(),
        RightsOffer::Exact(Rights::READ.union(Rights::EXECUTE)),
    )?;
    // Native-only profiles intentionally provide no VM client authority.
    if let Some(connection) = connection {
        builder.add_handle_duplicate(
            connection.as_handle_ref(),
            vm::MANAGER_CONNECTION.as_raw(),
            RightsOffer::Exact(Rights::WAIT.union(Rights::WRITE)),
        )?;
    }
    builder.add_handle_duplicate(
        input.as_handle_ref(),
        stdio::TERMINAL_INPUT.as_raw(),
        RightsOffer::Exact(stdio::TERMINAL_INPUT_CONTRACT.allowed_rights()),
    )?;
    for (channel, contract) in [
        (input, stdio::STANDARD_INPUT_CONTRACT),
        (output, stdio::STANDARD_OUTPUT_CONTRACT),
        (error, stdio::STANDARD_ERROR_CONTRACT),
    ] {
        builder
            .add_handle_move(
                channel,
                contract.purpose(),
                RightsOffer::Exact(contract.allowed_rights()),
            )
            .map_err(|failure| failure.error())?;
    }
    builder.seal()?;
    builder.start().map_err(|failure| failure.error())
}
