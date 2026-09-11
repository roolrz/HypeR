// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Per-VM image loader and lifetime owner.

use hyper_os::handle::{ByteChannelObject, VirtualCpuObject};
use hyper_os::memory::{MAX_TRANSFER_BYTES, WritableVmo};
use hyper_os::startup::Startup;
use hyper_os::wait::{ObjectSignals, WaitSet};
use hyper_service::vm as vm_contract;
use hyper_vm_image::guest_fdt::{self, GuestHardwareMetadata};
use hyper_vm_image::{Payload, ReadAt};
use hyper_vm_image::{aarch64_linux, linux, riscv64_linux};
use std::fs::File;
#[cfg(target_os = "hyper")]
use std::os::hyper::fs::FileExt;
#[cfg(unix)]
use std::os::unix::fs::FileExt;
use std::process::ExitCode;
use std::time::Instant;

fn application_main(mut startup: Startup<'_>, started: Instant) -> ExitCode {
    if hyper_os::require_core_abi().is_err() {
        return ExitCode::FAILURE;
    }
    let control = match startup.take(vm_contract::INSTANCE_CONTROL) {
        Ok(control) => control,
        Err(_) => return ExitCode::FAILURE,
    };
    let channel = control.as_byte_channel();
    eprintln!("HypeR vm-runtime: starting");
    match run(&mut startup, &channel, started) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = channel.send(&vm_contract::InstanceStatus::Failed(error.failure()).encode());
            ExitCode::FAILURE
        }
    }
}

#[inline(never)]
fn run(
    startup: &mut Startup<'_>,
    control: &hyper_os::channel::ByteChannel<'_>,
    started: Instant,
) -> Result<(), Error> {
    let source = ImageSource::new(hyper_os::fs::File::from_handle(
        startup
            .take(vm_contract::IMAGE)
            .map_err(Error::OperatingSystem)?,
    ))?;
    let lease = startup
        .take(vm_contract::CREATION_LEASE)
        .map_err(Error::OperatingSystem)?;
    let image = hyper_vm_image::parse(&source).map_err(classify_image_error)?;
    let plan = linux::validate_reference(&source, image).map_err(classify_reference_error)?;
    let profile = hyper_vm_runtime::profile::native_profile(plan.platform_profile())
        .map_err(|_| Error::UnsupportedConfiguration)?;
    let platform_info = hyper_os::vm::platform_info(lease.as_handle_ref(), profile)
        .map_err(classify_platform_error)?;
    let metadata = hyper_vm_runtime::profile::validate_metadata(
        plan.architecture(),
        plan.platform_profile(),
        platform_info,
    )
    .map_err(|_| Error::UnsupportedConfiguration)?;
    metadata
        .validate_for(&plan)
        .map_err(|_| Error::UnsupportedConfiguration)?;
    publish_status(control, vm_contract::InstanceStatus::ImageValidated)?;

    let connection = hyper_os::capability_channel::CapabilityChannel::from_handle(
        startup
            .take(vm_contract::CONSOLE_CONNECTION)
            .map_err(Error::OperatingSystem)?,
    );
    let root = startup
        .take(hyper_os::startup::ROOT_VMAR)
        .map_err(Error::OperatingSystem)?;
    let virtual_serial = hyper_os::virtual_serial::create().map_err(Error::OperatingSystem)?;
    let serial_memory = WritableVmo::create(hyper_os::virtual_serial::BUFFER_BYTES)
        .map_err(Error::OperatingSystem)?;
    let output = hyper_os::virtual_serial::Output::register(
        &virtual_serial,
        root.as_handle_ref(),
        0xd000_0000,
        serial_memory,
    )
    .map_err(Error::OperatingSystem)?;
    let mut console = hyper_vm_runtime::console::Console::new(connection, virtual_serial, output);
    let serial_binding = console.binding().map_err(Error::OperatingSystem)?;
    let memory = WritableVmo::create(plan.memory_size()).map_err(Error::OperatingSystem)?;
    copy_payload(&source, &memory, plan.memory_base(), image.kernel)?;
    if let Some(initramfs) = image.initramfs {
        copy_payload(&source, &memory, plan.memory_base(), initramfs)?;
    }
    build_device_tree(&memory, &plan, image.boot_arguments.as_str(), metadata)?;
    publish_status(control, vm_contract::InstanceStatus::MemoryPrepared)?;

    let pending = hyper_os::vm::create(
        lease,
        hyper_os::vm::Configuration {
            guest_physical_base: plan.memory_base(),
            memory_size: plan.memory_size(),
            vcpu_count: plan.vcpu_count(),
            architecture: platform_info.architecture,
            platform_profile: profile,
        },
    )
    .map_err(|failure| Error::OperatingSystem(failure.error()))?;
    hyper_os::vm::set_memory(pending.as_handle_ref(), memory.as_handle_ref())
        .map_err(Error::OperatingSystem)?;
    hyper_os::vm::set_bootstrap(
        pending.as_handle_ref(),
        hyper_os::vm::VirtualCpuBootstrap {
            entry: plan.kernel_entry(),
            stack: 0,
            arguments: plan.bootstrap_arguments(),
        },
    )
    .map_err(Error::OperatingSystem)?;
    hyper_os::vm::set_virtual_serial(pending.as_handle_ref(), serial_binding)
        .map_err(|failure| Error::OperatingSystem(failure.error()))?;
    hyper_os::vm::seal(pending.as_handle_ref()).map_err(Error::OperatingSystem)?;
    let (machine, vcpu) = hyper_os::vm::install(pending)
        .map_err(|failure| Error::OperatingSystem(failure.error()))?;
    publish_status(control, vm_contract::InstanceStatus::Installed)?;
    hyper_os::vm::start_vcpu(vcpu.as_handle_ref()).map_err(Error::OperatingSystem)?;
    // This ends at the successful start request, not the first guest entry:
    // scheduling and the EL1/VS transition happen asynchronously in the kernel.
    let elapsed = started.elapsed();
    eprintln!(
        "HypeR vm-runtime: vCPU start submitted in {}.{:03} ms (from main)",
        elapsed.as_millis(),
        elapsed.subsec_micros() % 1_000,
    );
    publish_status(control, vm_contract::InstanceStatus::Running)?;
    supervise_guest(&machine, &vcpu, control, &mut console)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Error {
    OperatingSystem(hyper_os::Error),
    Io(std::io::ErrorKind),
    InvalidImage,
    UnsupportedConfiguration,
    InvalidControl,
    Guest(hyper_os::vm::VirtualCpuTermination),
}

impl Error {
    const fn failure(self) -> vm_contract::InstanceFailure {
        match self {
            Self::InvalidImage => vm_contract::InstanceFailure::InvalidImage,
            Self::UnsupportedConfiguration => {
                vm_contract::InstanceFailure::UnsupportedConfiguration
            }
            Self::InvalidControl => vm_contract::InstanceFailure::InvalidControlProtocol,
            Self::Guest(hyper_os::vm::VirtualCpuTermination::MemoryFault) => {
                vm_contract::InstanceFailure::GuestMemoryFault
            }
            Self::Guest(hyper_os::vm::VirtualCpuTermination::Mmio) => {
                vm_contract::InstanceFailure::GuestMmio
            }
            Self::Guest(hyper_os::vm::VirtualCpuTermination::Synchronous) => {
                vm_contract::InstanceFailure::GuestSynchronous
            }
            Self::Guest(hyper_os::vm::VirtualCpuTermination::Administrative) => {
                vm_contract::InstanceFailure::UnexpectedAdministrativeStop
            }
            Self::OperatingSystem(_) | Self::Io(_) => vm_contract::InstanceFailure::Runtime,
        }
    }
}

fn classify_image_error(error: hyper_vm_image::Error<std::io::Error>) -> Error {
    match error {
        hyper_vm_image::Error::Source(error) => Error::Io(error.kind()),
        hyper_vm_image::Error::UnsupportedImage => Error::UnsupportedConfiguration,
        _ => Error::InvalidImage,
    }
}

fn classify_platform_error(error: hyper_os::Error) -> Error {
    if error == hyper_os::Error::Status(hyper_os::Status::NOT_SUPPORTED) {
        Error::UnsupportedConfiguration
    } else {
        Error::OperatingSystem(error)
    }
}

fn classify_reference_error(error: linux::Error<std::io::Error>) -> Error {
    match error {
        linux::Error::Aarch64(error) => classify_aarch64_reference_error(error),
        linux::Error::Riscv64(error) => classify_riscv64_reference_error(error),
        linux::Error::UnsupportedArchitecture | linux::Error::UnsupportedPlatformProfile => {
            Error::UnsupportedConfiguration
        }
    }
}

fn classify_riscv64_reference_error(
    error: riscv64_linux::ReferenceLayoutError<std::io::Error>,
) -> Error {
    use riscv64_linux::{Error as KernelError, ReferenceLayoutError};
    match error {
        ReferenceLayoutError::Source(error)
        | ReferenceLayoutError::Kernel(KernelError::Source(error)) => Error::Io(error.kind()),
        ReferenceLayoutError::UnsupportedArchitecture
        | ReferenceLayoutError::UnsupportedPlatformProfile
        | ReferenceLayoutError::UnsupportedVcpuCount
        | ReferenceLayoutError::Kernel(
            KernelError::CompressedPayload
            | KernelError::UnsupportedVersion
            | KernelError::UnsupportedFlags,
        ) => Error::UnsupportedConfiguration,
        _ => Error::InvalidImage,
    }
}

fn classify_aarch64_reference_error(
    error: aarch64_linux::ReferenceLayoutError<std::io::Error>,
) -> Error {
    use aarch64_linux::{Error as KernelError, ReferenceLayoutError};

    match error {
        ReferenceLayoutError::Source(error)
        | ReferenceLayoutError::Kernel(KernelError::Source(error)) => Error::Io(error.kind()),
        ReferenceLayoutError::UnsupportedArchitecture
        | ReferenceLayoutError::UnsupportedPlatformProfile
        | ReferenceLayoutError::UnsupportedVcpuCount
        | ReferenceLayoutError::Kernel(KernelError::CompressedPayload) => {
            Error::UnsupportedConfiguration
        }
        _ => Error::InvalidImage,
    }
}

fn publish_status(
    control: &hyper_os::channel::ByteChannel<'_>,
    status: vm_contract::InstanceStatus,
) -> Result<(), Error> {
    control
        .send(&status.encode())
        .map_err(Error::OperatingSystem)
}

fn supervise_guest(
    machine: &hyper_os::OwnedHandle<hyper_os::handle::VirtualMachineObject>,
    vcpu: &hyper_os::OwnedHandle<VirtualCpuObject>,
    control: &hyper_os::channel::ByteChannel<'_>,
    console: &mut hyper_vm_runtime::console::Console,
) -> Result<(), Error> {
    let waits = WaitSet::new(4).map_err(Error::OperatingSystem)?;
    let control_wait = waits
        .add(
            control.as_handle_ref(),
            ObjectSignals::<ByteChannelObject>::READABLE
                .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED),
        )
        .map_err(Error::OperatingSystem)?;
    let vcpu_wait = waits
        .add(
            vcpu.as_handle_ref(),
            ObjectSignals::<VirtualCpuObject>::TERMINATED,
        )
        .map_err(Error::OperatingSystem)?;
    let mut control_consumed = false;
    loop {
        console.service().map_err(Error::OperatingSystem)?;
        console
            .prepare_wait(&waits)
            .map_err(Error::OperatingSystem)?;
        if control_consumed {
            waits.rearm(control_wait).map_err(Error::OperatingSystem)?;
            control_consumed = false;
        }
        let observation = waits
            .wait(hyper_os::DEADLINE_INFINITE)
            .map_err(Error::OperatingSystem)?;
        if observation.registration != control_wait && observation.registration != vcpu_wait {
            console.observe(observation.registration, observation.signals);
            continue;
        }
        if observation.registration == control_wait {
            control_consumed = true;
        }
        if observation.registration == control_wait
            && ObjectSignals::<ByteChannelObject>::READABLE.is_present_in(observation.signals)
        {
            let mut message = [0u8; vm_contract::MESSAGE_BYTES];
            let length = match control.try_receive(&mut message) {
                Ok(length) => length,
                Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK)) => continue,
                Err(error) => return Err(Error::OperatingSystem(error)),
            };
            let command = message
                .get(..length)
                .and_then(vm_contract::InstanceCommand::decode)
                .ok_or(Error::InvalidControl)?;
            if command == vm_contract::InstanceCommand::AttachConsole {
                console.attach(&waits).map_err(Error::OperatingSystem)?;
                continue;
            }
            let terminal = stop_and_retire(machine, vcpu)?;
            if terminal != hyper_os::vm::VirtualCpuTermination::Administrative {
                return Err(Error::Guest(terminal));
            }
            publish_status(control, vm_contract::InstanceStatus::Stopped)?;
            return Ok(());
        }
        if observation.registration == control_wait {
            let _terminal = stop_and_retire(machine, vcpu)?;
            return Err(Error::InvalidControl);
        }
        if observation.registration != vcpu_wait {
            return Err(Error::InvalidControl);
        }
        let terminal = terminated_vcpu_reason(vcpu)?;
        hyper_os::vm::request_stop(machine.as_handle_ref()).map_err(Error::OperatingSystem)?;
        hyper_os::vm::wait_terminated(machine.as_handle_ref(), hyper_os::DEADLINE_INFINITE)
            .map_err(Error::OperatingSystem)?;
        return Err(Error::Guest(terminal));
    }
}

fn stop_and_retire(
    machine: &hyper_os::OwnedHandle<hyper_os::handle::VirtualMachineObject>,
    vcpu: &hyper_os::OwnedHandle<VirtualCpuObject>,
) -> Result<hyper_os::vm::VirtualCpuTermination, Error> {
    hyper_os::vm::request_stop(machine.as_handle_ref()).map_err(Error::OperatingSystem)?;
    hyper_os::vm::wait_vcpu_terminated(vcpu.as_handle_ref(), hyper_os::DEADLINE_INFINITE)
        .map_err(Error::OperatingSystem)?;
    let terminal = terminated_vcpu_reason(vcpu)?;
    hyper_os::vm::wait_terminated(machine.as_handle_ref(), hyper_os::DEADLINE_INFINITE)
        .map_err(Error::OperatingSystem)?;
    Ok(terminal)
}

fn terminated_vcpu_reason(
    vcpu: &hyper_os::OwnedHandle<VirtualCpuObject>,
) -> Result<hyper_os::vm::VirtualCpuTermination, Error> {
    hyper_os::vm::vcpu_info(vcpu.as_handle_ref())
        .map_err(Error::OperatingSystem)?
        .terminal
        .ok_or(Error::InvalidControl)
}

struct ImageSource {
    file: File,
    length: u64,
}

impl ImageSource {
    fn new(file: hyper_os::fs::File) -> Result<Self, Error> {
        // Image delegation grants READ, not INSPECT. Query length using that
        // existing authority before transferring ownership to std for I/O.
        let length = file.size().map_err(Error::OperatingSystem)?;
        Ok(Self {
            file: file.into_std(),
            length,
        })
    }
}

impl ReadAt for ImageSource {
    type Error = std::io::Error;

    fn length(&self) -> Result<u64, Self::Error> {
        Ok(self.length)
    }

    fn read_exact_at(&self, offset: u64, output: &mut [u8]) -> Result<(), Self::Error> {
        self.file.read_exact_at(output, offset)
    }
}

#[inline(never)]
fn copy_payload(
    source: &ImageSource,
    memory: &WritableVmo,
    memory_base: u64,
    payload: Payload,
) -> Result<(), Error> {
    let destination = payload
        .load_address
        .checked_sub(memory_base)
        .ok_or(Error::InvalidImage)?;
    let mut buffer = [0u8; MAX_TRANSFER_BYTES];
    let mut completed = 0u64;
    while completed < payload.length {
        let remaining = payload.length - completed;
        let length = usize::try_from(remaining.min(MAX_TRANSFER_BYTES as u64))
            .map_err(|_| Error::InvalidImage)?;
        let chunk = buffer.get_mut(..length).ok_or(Error::InvalidImage)?;
        let file_offset = payload
            .file_offset
            .checked_add(completed)
            .ok_or(Error::InvalidImage)?;
        let guest_offset = destination
            .checked_add(completed)
            .ok_or(Error::InvalidImage)?;
        source
            .read_exact_at(file_offset, chunk)
            .map_err(|error| Error::Io(error.kind()))?;
        memory
            .write_all_at(guest_offset, chunk)
            .map_err(Error::OperatingSystem)?;
        completed = completed
            .checked_add(length as u64)
            .ok_or(Error::InvalidImage)?;
    }
    Ok(())
}

#[inline(never)]
fn build_device_tree(
    memory: &WritableVmo,
    plan: &linux::BootPlan,
    boot_arguments: &str,
    metadata: GuestHardwareMetadata,
) -> Result<(), Error> {
    let mut structure = [0u8; 8192];
    let mut strings = [0u8; 2048];
    let mut output = [0u8; 12 * 1024];
    let length = guest_fdt::build_linux(
        plan,
        boot_arguments,
        metadata,
        &mut structure,
        &mut strings,
        &mut output,
    )
    .map_err(|_| Error::InvalidImage)?;
    let offset = plan
        .device_tree()
        .start()
        .checked_sub(plan.memory_base())
        .ok_or(Error::InvalidImage)?;
    memory
        .write_all_at(offset, output.get(..length).ok_or(Error::InvalidImage)?)
        .map_err(Error::OperatingSystem)
}

fn main() -> ExitCode {
    let started = Instant::now();
    match hyper_rt::process::startup() {
        Ok(startup) => application_main(startup, started),
        Err(_) => ExitCode::FAILURE,
    }
}
