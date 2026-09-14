// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use hyper_os::guest_io::Mailbox;
use hyper_os::handle::{GuestMailboxObject, VirtualMachineObject};
use hyper_os::startup::{self, Startup};
use hyper_os::wait::{self, ObjectSignals, WaitItem};
use hyper_os::{device, vm};
use hyper_vm_image::guest_fdt::{
    GuestHardwareMetadata,
    io::{DmaRange, IoDevices, MmioDevice},
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

fn run(startup: &Startup<'_>) -> Result<()> {
    let authority = startup
        .borrow(startup::DEVICE_ASSIGNMENT_AUTHORITY)
        .map_err(show)?;
    let physical = device::claim(authority, 0).map_err(show)?;
    let info = device::info(physical.as_handle_ref()).map_err(show)?;
    if info.device_id != 8 || info.transport_version != 2 {
        return Err("expected modern virtio-scsi storage device".into());
    }
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
    image.device_tree(
        gic_version,
        IoDevices {
            virtio: Some(MmioDevice {
                base: PHYSICAL_MMIO,
                size: info.mmio_size,
                irq: 40,
            }),
            dma_ranges: &[DmaRange {
                dma_base: dma.physical_base,
                cpu_base: RAM_BASE,
                size: RAM_BYTES,
            }],
            mailbox: Some(MmioDevice {
                base: MAILBOX_MMIO,
                size: 4096,
                irq: 41,
            }),
            ..IoDevices::empty()
        },
    )?;
    let grant = vm::create_guest_memory(image.memory.as_handle_ref()).map_err(show)?;
    let mut guest = io_guest::install(startup, &image, &grant, None, Some(&physical), 0xd000_0000)?;
    drop(grant);
    // Every fallible operation after installation is inside this result scope;
    // retirement runs even if mailbox creation, start or negotiation fails.
    let outcome = (|| {
        let mailbox =
            Mailbox::create(guest.machine.as_handle_ref(), MAILBOX_MMIO, 41).map_err(show)?;
        vm::start_vcpu(guest.cpus[0].as_handle_ref()).map_err(show)?;
        supervise(&mut guest, &mailbox)
    })();
    vm::request_stop(guest.machine.as_handle_ref()).map_err(show)?;
    vm::wait_terminated(guest.machine.as_handle_ref(), deadline(20)?).map_err(show)?;
    drain(&mut guest)?;
    outcome
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

fn supervise(guest: &mut InstalledGuest, mailbox: &Mailbox) -> Result<()> {
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
                    return Ok(());
                }
            }
        }
        if !ready {
            match mailbox.receive(&mut record) {
                Ok(length) => {
                    let reply = Reply::decode(&record[..length], hello).map_err(show)?;
                    if reply.status != Status::Success {
                        return Err("backend refused HELLO".into());
                    }
                    ready = true;
                    println!("HypeR io-runtime: ready; storage backend idle (no client attached)");
                }
                Err(hyper_os::Error::Status(hyper_os::Status::WOULD_BLOCK)) => {}
                Err(error) => return Err(show(error)),
            }
        }
        let items = [
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
        let observed = wait::wait_many(
            &items,
            if ready {
                hyper_os::DEADLINE_INFINITE
            } else {
                startup_deadline
            },
        )
        .map_err(show)?;
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

pub(super) fn main() -> ExitCode {
    let startup = match hyper_rt::process::startup() {
        Ok(startup) => startup,
        Err(error) => {
            eprintln!("HypeR io-runtime: startup failed: {error:?}");
            return ExitCode::FAILURE;
        }
    };
    match run(&startup) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("HypeR io-runtime: failed: {error}");
            ExitCode::FAILURE
        }
    }
}
