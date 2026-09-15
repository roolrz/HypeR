// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

#[path = "broker.rs"]
mod broker;
#[path = "managed.rs"]
mod managed;

use hyper_os::guest_io::Mailbox;
use hyper_os::handle::{GuestMailboxObject, VirtualMachineObject};
use hyper_os::startup::{self, Startup};
use hyper_os::wait::{self, ObjectSignals, WaitItem};
use hyper_os::{device, vm};
use hyper_vm_image::guest_fdt::{
    GuestHardwareMetadata,
    io::{DmaRange, IoDevices, MmioDevice, MmioWindow, SdhciDevice, SdhciRevision},
};
use hyper_vm_runtime::io_guest::{self, Image, InstalledGuest, PHYSICAL_MMIO, RAM_BASE, RAM_BYTES};
use hyper_vm_runtime::io_protocol::{Command, MAX_RECORD, Reply, Request, Status};
use std::io::{self, Write};
use std::process::ExitCode;
use std::time::Duration;

type Result<T> = std::result::Result<T, String>;
const MAILBOX_MMIO: u64 = 0x0a01_0000;
fn show(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}
fn deadline(seconds: u64) -> Result<u64> {
    hyper_os::time::deadline_after(Duration::from_secs(seconds))
        .map(|deadline| deadline.as_raw())
        .map_err(show)
}

/// Check operation time independently of wait readiness. Call only after
/// observing its completion, so an already queued reply still wins.
fn check_deadline(limit: u64) -> Result<()> {
    let now = hyper_os::time::monotonic_now()
        .map_err(show)?
        .as_nanoseconds();
    if hyper_io_runtime::deadline::expired(now, limit) {
        Err("I/O operation timed out".into())
    } else {
        Ok(())
    }
}

fn run(startup: &mut Startup<'_>) -> Result<()> {
    let ready = startup
        .take_optional(hyper_service::io::READY)
        .map_err(show)?;
    let mut broker = startup
        .take_optional(hyper_service::io::BROKER_SERVER)
        .map_err(show)?
        .map(broker::Broker::load)
        .transpose()?;
    if broker.is_some() && ready.is_none() {
        return Err("I/O broker requires managed storage readiness".into());
    }
    let authority = startup
        .borrow(startup::DEVICE_ASSIGNMENT_AUTHORITY)
        .map_err(show)?;
    let (physical, sdhci_profile) = if ready.is_some() {
        let policy = hyper_io_runtime::device_policy::load("/etc/hyper/board.json")?;
        match policy.profile() {
            device::Profile::Userspace => {
                let (physical, profile) =
                    hyper_io_runtime::sdhci::claim(authority, policy.identity()).map_err(show)?;
                (physical, Some(profile))
            }
            device::Profile::VirtioMmioScsi => {
                #[cfg(feature = "userspace-device-test")]
                let physical =
                    hyper_io_runtime::sdhci::claim_virtio_test(authority, policy.identity())
                        .map_err(show)?;
                #[cfg(not(feature = "userspace-device-test"))]
                let physical =
                    device::claim_matching(authority, policy.profile(), policy.identity())
                        .map_err(show)?;
                (physical, None)
            }
        }
    } else {
        (device::claim(authority, 0).map_err(show)?, None)
    };
    let profile = device::profile_info(physical.as_handle_ref()).map_err(show)?;
    let mut client = ready
        .map(|ready| managed::Client::prepare(authority, ready))
        .transpose()?;
    let image = Image::load("/vm/io.itb")?;
    let dma =
        device::dma_extent(authority, image.memory.as_handle_ref(), 0, RAM_BYTES).map_err(show)?;
    let lease = vm::derive_creation_lease(
        startup
            .borrow(startup::VIRTUAL_MACHINE_CREATION_AUTHORITY)
            .map_err(show)?,
        startup.borrow(startup::RESOURCE_DOMAIN).map_err(show)?,
    )
    .map_err(show)?;
    let platform = vm::platform_info(lease.as_handle_ref(), vm::PlatformProfile::Aarch64Reference)
        .map_err(show)?;
    let metadata = hyper_vm_runtime::profile::validate_metadata(
        image.plan.architecture(),
        image.plan.platform_profile(),
        platform,
    )
    .map_err(show)?;
    let GuestHardwareMetadata::Aarch64 { gic_version } = metadata else {
        return Err("expected AArch64 platform metadata".into());
    };
    drop(lease);
    let own_dma = DmaRange {
        dma_base: dma.physical_base,
        cpu_base: RAM_BASE,
        size: RAM_BYTES,
    };
    let resource = |index| -> Result<MmioWindow> {
        let window = device::resource_info(physical.as_handle_ref(), index).map_err(show)?;
        if window.kind != index + 1 {
            return Err("unexpected physical resource kind".into());
        }
        Ok(MmioWindow {
            base: PHYSICAL_MMIO + window.offset,
            size: window.length,
        })
    };
    let host = resource(0)?;
    let host = MmioDevice {
        base: host.base,
        size: host.size,
        irq: 40,
    };
    let (physical_node, sdhci) = match (profile.profile, sdhci_profile) {
        (device::Profile::VirtioMmioScsi, None) if profile.resource_count == 1 => {
            (Some(host), None)
        }
        #[cfg(feature = "userspace-device-test")]
        (device::Profile::Userspace, None) if profile.resource_count == 1 => (Some(host), None),
        (device::Profile::Userspace, Some(metadata)) if profile.resource_count == 5 => (
            None,
            Some(SdhciDevice {
                host,
                config: resource(1)?,
                main_pinctrl: resource(2)?,
                aon_pinctrl: resource(3)?,
                aon_gpio: resource(4)?,
                revision: match metadata.revision {
                    0 => SdhciRevision::C0,
                    1 => SdhciRevision::D0,
                    _ => return Err("unsupported SDHCI revision".into()),
                },
                clock_hz: metadata.clock_hz,
                gpio_widths: metadata.gpio_widths,
            }),
        ),
        _ => return Err("invalid physical resource bundle".into()),
    };
    if let Some(client) = client.as_ref() {
        let mut descriptions = vec![client.description()];
        let mut dma_ranges = vec![own_dma, client.dma_range()];
        if let Some(broker) = broker.as_ref() {
            descriptions.extend(broker.descriptions());
            dma_ranges.extend(broker.dma_ranges(&dma_ranges)?);
        }
        image.device_tree(
            gic_version,
            IoDevices {
                clients: &descriptions,
                virtio: physical_node,
                sdhci,
                dma_ranges: &dma_ranges,
                ..IoDevices::empty()
            },
        )?;
    } else {
        image.device_tree(
            gic_version,
            IoDevices {
                clients: &[],
                virtio: physical_node,
                sdhci,
                dma_ranges: &[own_dma],
                mailbox: Some(MmioDevice {
                    base: MAILBOX_MMIO,
                    size: 4096,
                    irq: 41,
                }),
                ..IoDevices::empty()
            },
        )?;
    }
    let grant = vm::create_guest_memory(image.memory.as_handle_ref()).map_err(show)?;
    let mappings = client.as_ref().map(|client| client.mapping());
    let mut guest = io_guest::install_mapped(
        startup,
        &image,
        &grant,
        mappings.as_slice(),
        if broker.is_some() {
            vm::DYNAMIC_ALIAS_OFFSET + vm::DYNAMIC_PHYSICAL_LIMIT - RAM_BASE
        } else {
            RAM_BYTES
                + if client.is_some() {
                    hyper_os::block::MEMORY_BYTES
                } else {
                    0
                }
        },
        Some(&physical),
        0xd000_0000,
    )?;
    drop(grant);
    // Every fallible operation after installation is inside this result scope;
    // retirement runs even if mailbox creation, start or negotiation fails.
    let mut hardware_worker = None;
    let outcome = (|| {
        let mailbox =
            Mailbox::create(guest.machine.as_handle_ref(), MAILBOX_MMIO, 41).map_err(show)?;
        if let Some(client) = client.as_mut() {
            client.install(&guest)?;
        }
        if let Some(broker) = broker.as_mut() {
            broker.install(&guest)?;
        }
        if profile.profile == device::Profile::Userspace {
            use hyper_io_runtime::sdhci::worker;
            vm::register_mmio(
                guest.machine.as_handle_ref(),
                PHYSICAL_MMIO,
                65536,
                core::num::NonZeroU64::new(worker::DEVICE_COOKIE).ok_or("invalid device cookie")?,
            )
            .map_err(|error| format!("register physical MMIO: {error:?}"))?;
            hardware_worker = Some(if sdhci.is_some() {
                worker::start(&physical, &guest)
                    .map_err(|error| format!("start SDHCI worker: {error:?}"))?
            } else {
                #[cfg(feature = "userspace-device-test")]
                {
                    worker::start_virtio_test(&physical, &guest)
                        .map_err(|error| format!("start test device worker: {error:?}"))?
                }
                #[cfg(not(feature = "userspace-device-test"))]
                {
                    return Err("missing userspace physical device driver".into());
                }
            });
        }
        #[cfg(feature = "userspace-device-test")]
        println!("DEVICE-TEST: worker prepared");
        vm::start_vcpu(guest.cpus[0].as_handle_ref()).map_err(show)?;
        supervise(
            startup,
            &mut guest,
            &mailbox,
            client.as_mut(),
            broker.as_mut(),
        )
    })();
    if let Err(error) = &outcome {
        eprintln!("HypeR io-runtime: retiring after failure: {error}");
    }
    let stopped = vm::request_stop(guest.machine.as_handle_ref())
        .map_err(show)
        .and_then(|()| {
            vm::wait_terminated(guest.machine.as_handle_ref(), deadline(20)?).map_err(show)
        });
    let worker_stopped = hardware_worker
        .map(|worker| worker.stop().map_err(show))
        .transpose()
        .map(|_| ());
    let drained = drain(&mut guest);
    // Attempt every cleanup step without hiding the failure that started
    // retirement behind a later timeout or worker shutdown error.
    outcome.and(stopped).and(worker_stopped).and(drained)
}

fn drain(guest: &mut InstalledGuest) -> Result<()> {
    let mut bytes = [0; 2048];
    for _ in 0..16 {
        let length = guest.output.try_read(&mut bytes).map_err(show)?;
        if length == 0 {
            break;
        }
        io::stdout().write_all(&bytes[..length]).map_err(show)?;
    }
    Ok(())
}

fn supervise(
    startup: &Startup<'_>,
    guest: &mut InstalledGuest,
    mailbox: &Mailbox,
    mut client: Option<&mut managed::Client>,
    mut broker: Option<&mut broker::Broker>,
) -> Result<()> {
    // HELLO has no DMA/queue side effects. There is deliberately no periodic
    // health request once the backend acknowledges its startup contract.
    let hello = Request {
        binding: 1,
        epoch: 1,
        transaction: 1,
        command: Command::Hello,
    };
    let mut record = [0; MAX_RECORD];
    let length = hello.encode(&mut record).map_err(show)?;
    mailbox.send(&record[..length]).map_err(show)?;
    let startup_deadline = deadline(60)?;
    let mut ready = false;
    loop {
        if !pump_guest(guest)? {
            return Ok(());
        }
        if !ready {
            match mailbox.receive(&mut record) {
                Ok(length) => {
                    let reply = Reply::decode(&record[..length], hello).map_err(show)?;
                    if reply.status != Status::Success {
                        return Err("backend refused HELLO".into());
                    }
                    ready = true;
                    if let Some(client) = client.as_mut() {
                        client.mount(
                            startup,
                            guest,
                            mailbox,
                            reply.features.ok_or("HELLO omitted features")?,
                        )?;
                        println!("HypeR io-runtime: ready; configuration volume mounted at /data");
                    } else {
                        println!(
                            "HypeR io-runtime: ready; storage backend idle (no client attached)"
                        );
                    }
                }
                Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK)) => {}
                Err(error) => return Err(show(error)),
            }
        }
        if !ready {
            check_deadline(startup_deadline)?;
        }
        if ready
            && let Some(broker) = broker.as_mut()
            && broker.service(guest)?
        {
            continue;
        }
        let mut items = vec![
            guest.output.wait_item(),
            WaitItem::new(
                guest.machine.as_handle_ref(),
                ObjectSignals::<VirtualMachineObject>::POWER_REQUEST
                    .union(ObjectSignals::<VirtualMachineObject>::VCPU_TERMINATED),
            ),
            WaitItem::new(
                mailbox.as_handle_ref(),
                ObjectSignals::<GuestMailboxObject>::READABLE
                    .union(ObjectSignals::<GuestMailboxObject>::PEER_CLOSED),
            ),
        ];
        if ready && let Some(broker) = broker.as_ref() {
            items.extend(broker.wait_items());
        }
        let observed = match wait::wait_many(
            &items,
            if ready {
                broker
                    .as_ref()
                    .map_or(hyper_os::DEADLINE_INFINITE, |broker| broker.next_deadline())
            } else {
                startup_deadline
            },
        ) {
            Ok(observed) => observed,
            Err(hyper_os::Error::Status(hyper_os::Status::TIMED_OUT)) if ready => continue,
            Err(error) => return Err(show(error)),
        };
        if observed.index == 2 {
            if ObjectSignals::<GuestMailboxObject>::PEER_CLOSED.is_present_in(observed.observed) {
                return Err("backend control channel closed".into());
            }
            if ready {
                return Err("unexpected message from idle backend".into());
            }
        }
    }
}

fn pump_guest(guest: &mut InstalledGuest) -> Result<bool> {
    drain(guest)?;
    for cpu in &guest.cpus {
        if let Some(reason) = vm::vcpu_info(cpu.as_handle_ref()).map_err(show)?.terminal {
            return Err(format!("I/O vCPU terminated: {reason:?}"));
        }
    }
    for _ in 0..guest.cpus.len() {
        let Some(request) =
            vm::pending_power_request(guest.machine.as_handle_ref()).map_err(show)?
        else {
            break;
        };
        match request.operation {
            vm::PowerOperation::CpuOn | vm::PowerOperation::CpuOff => {
                vm::complete_power_request(guest.machine.as_handle_ref(), request.id, true)
                    .map_err(show)?
            }
            vm::PowerOperation::SystemOff | vm::PowerOperation::SystemReset => {
                eprintln!("HypeR io-runtime: backend requested shutdown");
                return Ok(false);
            }
        }
    }
    Ok(true)
}

pub(super) fn main() -> ExitCode {
    let mut startup = match hyper_rt::process::startup() {
        Ok(startup) => startup,
        Err(error) => {
            eprintln!("HypeR io-runtime: startup failed: {error:?}");
            return ExitCode::FAILURE;
        }
    };
    match run(&mut startup) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("HypeR io-runtime: failed: {error}");
            ExitCode::FAILURE
        }
    }
}
