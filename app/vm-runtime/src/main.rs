// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Per-VM image loader and lifetime owner.

mod disk;

#[cfg(feature = "test-power-crash")]
#[path = "../tests/power_crash.rs"]
mod power_crash;

use hyper_os::handle::{ByteChannelObject, VirtualCpuObject, VirtualMachineObject};
use hyper_os::memory::WritableVmo;
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
            eprintln!("HypeR vm-runtime: failed: {error:?}");
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
    let disk_session = startup
        .take_optional(hyper_service::io::SESSION)
        .map_err(Error::OperatingSystem)?
        .map(hyper_os::capability_channel::CapabilityChannel::from_handle);
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
    let profile = hyper_vm_support::profile::native_profile(plan.platform_profile())
        .map_err(|_| Error::UnsupportedConfiguration)?;
    let platform_info = hyper_os::vm::platform_info(lease.as_handle_ref(), profile)
        .map_err(classify_platform_error)?;
    let metadata = hyper_vm_support::profile::validate_metadata(
        plan.architecture(),
        plan.platform_profile(),
        platform_info,
    )
    .map_err(|_| Error::UnsupportedConfiguration)?;
    metadata
        .validate_for(&plan)
        .map_err(|_| Error::UnsupportedConfiguration)?;
    #[cfg(feature = "startup-profile")]
    eprintln!(
        "HypeR startup profile: image validated at {} us",
        started.elapsed().as_micros()
    );
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
    #[cfg(feature = "startup-profile")]
    eprintln!(
        "HypeR startup profile: RAM allocated at {} us",
        started.elapsed().as_micros()
    );
    copy_payload(&source, &memory, plan.memory_base(), image.kernel)?;
    if let Some(initramfs) = image.initramfs {
        copy_payload(&source, &memory, plan.memory_base(), initramfs)?;
    }
    build_device_tree(
        &memory,
        &plan,
        image.boot_arguments.as_str(),
        metadata,
        disk_session.is_some(),
    )?;
    #[cfg(feature = "startup-profile")]
    eprintln!(
        "HypeR startup profile: memory prepared at {} us",
        started.elapsed().as_micros()
    );
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
    let shared_memory = if disk_session.is_some() {
        let grant = hyper_os::vm::create_guest_memory(memory.as_handle_ref())
            .map_err(Error::OperatingSystem)?;
        hyper_os::vm::map_guest_memory(
            pending.as_handle_ref(),
            grant.as_handle_ref(),
            0,
            0,
            plan.memory_size(),
        )
        .map_err(Error::OperatingSystem)?;
        Some(grant)
    } else {
        hyper_os::vm::set_memory(pending.as_handle_ref(), memory.as_handle_ref())
            .map_err(Error::OperatingSystem)?;
        None
    };
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
    // Keep inspection capabilities through retirement, including secondary CPU
    // failures. No registry lookup or allocation is needed on the stop path.
    let mut vcpus = Vec::with_capacity(plan.vcpu_count() as usize);
    vcpus.push(vcpu);
    for index in 1..plan.vcpu_count() {
        vcpus.push(
            hyper_os::vm::open_vcpu(machine.as_handle_ref(), index)
                .map_err(Error::OperatingSystem)?,
        );
    }
    // Installation, not image loading, opens the broker admission window.
    // The manager retains the session endpoint until this status is observed.
    #[cfg(feature = "broker-test")]
    if disk_session.is_some() {
        // Exceed the broker's 60-second handshake budget before publishing
        // readiness. Ordinary runtime artifacts never include this delay.
        std::thread::sleep(std::time::Duration::from_secs(65));
    }
    #[cfg(feature = "startup-profile")]
    eprintln!(
        "HypeR startup profile: installed at {} us",
        started.elapsed().as_micros()
    );
    publish_status(control, vm_contract::InstanceStatus::Installed)?;
    let (machine, mut disk) = if let Some(session) = disk_session {
        let grant = shared_memory.as_ref().ok_or(Error::InvalidControl)?;
        let (machine, disk) = disk::bind(
            session,
            machine,
            grant,
            plan.memory_base(),
            plan.memory_size(),
        )?;
        (machine, Some(disk))
    } else {
        (machine, None)
    };
    #[cfg(feature = "test-power-crash")]
    power_crash::at("dormant");
    hyper_os::vm::start_vcpu(vcpus[0].as_handle_ref()).map_err(Error::OperatingSystem)?;
    // This ends at the successful start request, not the first guest entry:
    // scheduling and the EL1/VS transition happen asynchronously in the kernel.
    let elapsed = started.elapsed();
    eprintln!(
        "HypeR vm-runtime: vCPU start submitted in {}.{:03} ms (from main)",
        elapsed.as_millis(),
        elapsed.subsec_micros() % 1_000,
    );
    publish_status(control, vm_contract::InstanceStatus::Running)?;
    supervise_guest(&machine, &vcpus, control, &mut console, disk.as_mut())
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
    vcpus: &[hyper_os::OwnedHandle<VirtualCpuObject>],
    control: &hyper_os::channel::ByteChannel<'_>,
    console: &mut hyper_vm_runtime::console::Console,
    mut disk: Option<&mut disk::Disk>,
) -> Result<(), Error> {
    let waits = WaitSet::new(6 + vcpus.len()).map_err(Error::OperatingSystem)?;
    let control_wait = waits
        .add(
            control.as_handle_ref(),
            ObjectSignals::<ByteChannelObject>::READABLE
                .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED),
        )
        .map_err(Error::OperatingSystem)?;
    let machine_wait = waits
        .add(
            machine.as_handle_ref(),
            ObjectSignals::<VirtualMachineObject>::POWER_REQUEST
                .union(ObjectSignals::<VirtualMachineObject>::VCPU_TERMINATED),
        )
        .map_err(Error::OperatingSystem)?;
    let mut control_consumed = false;
    loop {
        if let Some(disk) = disk.as_mut() {
            disk.service(vcpus)?;
            disk.prepare_wait(&waits, vcpus)?;
        }
        console.service().map_err(Error::OperatingSystem)?;
        console
            .prepare_wait(&waits)
            .map_err(Error::OperatingSystem)?;
        if control_consumed {
            waits.rearm(control_wait).map_err(Error::OperatingSystem)?;
            control_consumed = false;
        }
        let observation = waits
            .wait(
                disk.as_ref()
                    .map_or(hyper_os::DEADLINE_INFINITE, |disk| disk.deadline()),
            )
            .map_err(Error::OperatingSystem)?;
        if disk
            .as_ref()
            .is_some_and(|disk| disk.owns(observation.registration))
        {
            continue;
        }
        if observation.registration != control_wait && observation.registration != machine_wait {
            console.observe(observation.registration, observation.signals);
            continue;
        }
        if observation.registration == control_wait {
            control_consumed = true;
        }
        if observation.registration == control_wait
            && ObjectSignals::<ByteChannelObject>::READABLE.is_present_in(observation.signals)
        {
            let mut message = [0u8; vm_contract::CONTROL_BYTES];
            let length = match control.try_receive(&mut message) {
                Ok(length) => length,
                Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK)) => continue,
                Err(error) => return Err(Error::OperatingSystem(error)),
            };
            if let Some(request) = vm_contract::ObservationRequest::decode(&message[..length]) {
                let info = hyper_os::vm::machine_info(machine.as_handle_ref())
                    .map_err(Error::OperatingSystem)?;
                let reply = vm_contract::Observation {
                    request,
                    vcpus: info.vcpu_count,
                    capacity_bytes: info.memory_size,
                    resident_bytes: info.resident_memory_bytes,
                }
                .encode();
                // Inspection must never block guest service or retirement if a
                // manager times out or stops consuming replies.
                match control.try_send(&reply) {
                    Ok(()) | Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK)) => {}
                    Err(error) => return Err(Error::OperatingSystem(error)),
                }
                continue;
            }
            if let Some(request) = vm_contract::VcpuControlRequest::decode(&message[..length]) {
                let reply = hyper_vm_runtime::control::handle(
                    request,
                    vcpus.len(),
                    |vcpu, words| {
                        hyper_os::vm::set_vcpu_affinity(vcpus[vcpu as usize].as_handle_ref(), words)
                    },
                    |vcpu| {
                        hyper_os::vm::vcpu_info(vcpus[vcpu as usize].as_handle_ref())
                            .map(|info| (info.host_cpu, info.migration_target))
                    },
                );
                match control.try_send(&reply.encode()) {
                    Ok(()) | Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK)) => {}
                    Err(error) => return Err(Error::OperatingSystem(error)),
                }
                continue;
            }
            let command = message
                .get(..length)
                .and_then(vm_contract::InstanceCommand::decode)
                .ok_or(Error::InvalidControl)?;
            if command == vm_contract::InstanceCommand::AttachConsole {
                console.attach(&waits).map_err(Error::OperatingSystem)?;
                continue;
            }
            let terminal = stop_and_retire(machine, vcpus)?;
            if terminal != hyper_os::vm::VirtualCpuTermination::Administrative {
                return Err(Error::Guest(terminal));
            }
            publish_status(control, vm_contract::InstanceStatus::Stopped)?;
            return Ok(());
        }
        if observation.registration == control_wait {
            let _terminal = stop_and_retire(machine, vcpus)?;
            return Err(Error::InvalidControl);
        }
        if observation.registration != machine_wait {
            return Err(Error::InvalidControl);
        }
        if ObjectSignals::<VirtualMachineObject>::VCPU_TERMINATED.is_present_in(observation.signals)
        {
            return Err(Error::Guest(stop_and_retire(machine, vcpus)?));
        }
        // A guest may submit another request immediately after completion.
        // Bound each drain so control/console observations remain serviceable.
        for _ in 0..vcpus.len() {
            let Some(request) = hyper_os::vm::pending_power_request(machine.as_handle_ref())
                .map_err(Error::OperatingSystem)?
            else {
                break;
            };
            use hyper_os::vm::PowerOperation;
            match request.operation {
                PowerOperation::CpuOn | PowerOperation::CpuOff => {
                    #[cfg(feature = "test-power-crash")]
                    if request.operation == PowerOperation::CpuOn {
                        power_crash::at("pending");
                    }
                    hyper_os::vm::complete_power_request(machine.as_handle_ref(), request.id, true)
                        .map_err(Error::OperatingSystem)?;
                    #[cfg(feature = "test-power-crash")]
                    if request.operation == PowerOperation::CpuOff {
                        power_crash::at("powered-off");
                    }
                }
                PowerOperation::SystemOff | PowerOperation::SystemReset => {
                    // Never resume the requesting CPU after a successful system
                    // power operation. Retire every CPU before reporting terminal
                    // status; the manager owns authority for a fresh reboot lease.
                    let terminal = stop_and_retire(machine, vcpus)?;
                    if terminal != hyper_os::vm::VirtualCpuTermination::Administrative {
                        return Err(Error::Guest(terminal));
                    }
                    let status = if request.operation == PowerOperation::SystemReset {
                        vm_contract::InstanceStatus::RebootRequested
                    } else {
                        vm_contract::InstanceStatus::Stopped
                    };
                    publish_status(control, status)?;
                    return Ok(());
                }
            }
        }
        waits.rearm(machine_wait).map_err(Error::OperatingSystem)?;
    }
}

fn stop_and_retire(
    machine: &hyper_os::OwnedHandle<VirtualMachineObject>,
    vcpus: &[hyper_os::OwnedHandle<VirtualCpuObject>],
) -> Result<hyper_os::vm::VirtualCpuTermination, Error> {
    hyper_os::vm::request_stop(machine.as_handle_ref()).map_err(Error::OperatingSystem)?;
    hyper_os::vm::wait_terminated(machine.as_handle_ref(), hyper_os::DEADLINE_INFINITE)
        .map_err(Error::OperatingSystem)?;
    let mut result = hyper_os::vm::VirtualCpuTermination::Administrative;
    for cpu in vcpus {
        let terminal = terminated_vcpu_reason(cpu)?;
        if terminal != hyper_os::vm::VirtualCpuTermination::Administrative {
            result = terminal;
        }
    }
    Ok(result)
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
    let statistics = hyper_vm_support::image_io::copy(
        payload.file_offset,
        payload.length,
        |offset, bytes| source.read_exact_at(offset, bytes),
        |offset, bytes| {
            let offset = destination.checked_add(offset).ok_or(Error::InvalidImage)?;
            memory
                .write_all_at(offset, bytes)
                .map_err(Error::OperatingSystem)
        },
    )
    .map_err(|error| match error {
        hyper_vm_support::image_io::Error::Read(error)
        | hyper_vm_support::image_io::Error::Thread(error) => Error::Io(error.kind()),
        hyper_vm_support::image_io::Error::Write(error) => error,
        hyper_vm_support::image_io::Error::Allocation => Error::Io(std::io::ErrorKind::OutOfMemory),
        hyper_vm_support::image_io::Error::InvalidRange => Error::InvalidImage,
        hyper_vm_support::image_io::Error::WorkerStopped => Error::Io(std::io::ErrorKind::Other),
    })?;
    #[cfg(feature = "startup-profile")]
    eprintln!(
        "HypeR startup profile: payload {} bytes read={} us write={} us",
        payload.length,
        statistics.read.as_micros(),
        statistics.write.as_micros()
    );
    #[cfg(not(feature = "startup-profile"))]
    let _ = statistics;
    Ok(())
}

#[inline(never)]
fn build_device_tree(
    memory: &WritableVmo,
    plan: &linux::BootPlan,
    boot_arguments: &str,
    metadata: GuestHardwareMetadata,
    disk: bool,
) -> Result<(), Error> {
    let mut structure = [0u8; 8192];
    let mut strings = [0u8; 2048];
    let mut output = [0u8; 12 * 1024];
    let length = if disk {
        let GuestHardwareMetadata::Aarch64 { gic_version } = metadata else {
            return Err(Error::UnsupportedConfiguration);
        };
        guest_fdt::build_aarch64_linux_with_io(
            guest_fdt::Aarch64LinuxBoot {
                memory_base: plan.memory_base(),
                memory_size: plan.memory_size(),
                vcpu_count: plan.vcpu_count(),
                gic_version,
                initramfs: plan.initramfs().map(|range| (range.start(), range.end())),
                boot_arguments,
            },
            guest_fdt::io::IoDevices {
                virtio: Some(guest_fdt::io::MmioDevice {
                    base: hyper_service::io::FRONTEND_MMIO,
                    size: 4096,
                    irq: hyper_service::io::FRONTEND_IRQ,
                }),
                ..guest_fdt::io::IoDevices::empty()
            },
            &mut structure,
            &mut strings,
            &mut output,
        )
    } else {
        guest_fdt::build_linux(
            plan,
            boot_arguments,
            metadata,
            &mut structure,
            &mut strings,
            &mut output,
        )
    }
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
