// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use hyper_os::device;
use hyper_os::guest_io::{Mailbox, Notification};
use hyper_os::handle::{
    GuestMailboxObject, GuestNotificationObject, Rights, VirtualCpuObject, VirtualMachineObject,
};
use hyper_os::memory::WritableVmo;
use hyper_os::startup::{self, Startup};
use hyper_os::virtual_serial::{self, Output};
use hyper_os::vm::{self, PowerOperation};
use hyper_os::wait::{self, ObjectSignals, WaitItem};
use hyper_vm_image::guest_fdt::io::{DmaRange, IoDevices, MmioDevice, SharedMemory};
use hyper_vm_image::{Payload, ReadAt, guest_fdt, linux};
use hyper_vm_runtime::io_backend::{Backend, Completion};
use std::fs::File;
use std::io::{self, Write};
use std::num::NonZeroU64;
use std::os::hyper::fs::FileExt;
use std::process::ExitCode;
use std::time::Duration;

type Result<T> = std::result::Result<T, String>;
const RAM_BASE: u64 = 0x4000_0000;
const RAM_BYTES: u64 = 64 * 1024 * 1024;
const SHARED_BASE: u64 = RAM_BASE + RAM_BYTES;
const FRONT_MMIO: u64 = 0x0a00_0000;
const MAILBOX_MMIO: u64 = 0x0a01_0000;
const NOTIFICATION_MMIO: u64 = 0x0a02_0000;
const PHYSICAL_MMIO: u64 = 0x0b00_0000;
const DISK_PASS: &[u8] = b"HypeR business disk: PASS read/write/flush (32 rounds)";

fn show(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}
fn device_node(base: u64, irq: u32) -> MmioDevice {
    MmioDevice {
        base,
        irq,
        size: 4096,
    }
}
fn deadline(seconds: u64) -> Result<u64> {
    hyper_os::time::deadline_after(Duration::from_secs(seconds))
        .map(|d| d.as_raw())
        .map_err(show)
}

struct Image {
    memory: WritableVmo,
    plan: linux::BootPlan,
    arguments: String,
}
struct Source {
    file: File,
    length: u64,
}
impl ReadAt for Source {
    type Error = io::Error;
    fn length(&self) -> io::Result<u64> {
        Ok(self.length)
    }
    fn read_exact_at(&self, offset: u64, output: &mut [u8]) -> io::Result<()> {
        self.file.read_exact_at(output, offset)
    }
}

impl Image {
    fn load(path: &str) -> Result<Self> {
        let file = File::open(path).map_err(show)?;
        let length = file.metadata().map_err(show)?.len();
        let source = Source { file, length };
        let image = hyper_vm_image::parse(&source).map_err(show)?;
        let plan = linux::validate_reference(&source, image).map_err(show)?;
        if plan.memory_base() != RAM_BASE
            || plan.memory_size() != RAM_BYTES
            || plan.architecture() != hyper_vm_image::Architecture::Aarch64
        {
            return Err("fixture requires 64 MiB AArch64 reference images".into());
        }
        println!("IO-VM-SMOKE: validated {path}, allocating guest RAM");
        let memory = WritableVmo::create_contiguous(RAM_BYTES).map_err(show)?;
        println!("IO-VM-SMOKE: copying {path} kernel");
        copy_payload(&source, &memory, image.kernel)?;
        if let Some(payload) = image.initramfs {
            copy_payload(&source, &memory, payload)?;
        }
        Ok(Self {
            memory,
            plan,
            arguments: image.boot_arguments.as_str().into(),
        })
    }
    fn device_tree(&self, gic_version: u32, devices: IoDevices<'_>) -> Result<()> {
        let mut structure = vec![0; 12 * 1024];
        let mut strings = vec![0; 2048];
        let mut output = vec![0; 16 * 1024];
        let length = guest_fdt::build_aarch64_linux_with_io(
            guest_fdt::Aarch64LinuxBoot {
                memory_base: RAM_BASE,
                memory_size: RAM_BYTES,
                vcpu_count: self.plan.vcpu_count(),
                gic_version,
                initramfs: self
                    .plan
                    .initramfs()
                    .map(|range| (range.start(), range.end())),
                boot_arguments: &self.arguments,
            },
            devices,
            &mut structure,
            &mut strings,
            &mut output,
        )
        .map_err(show)?;
        self.memory
            .write_all_at(
                self.plan.device_tree().start() - RAM_BASE,
                &output[..length],
            )
            .map_err(show)
    }
}

fn copy_payload(source: &Source, memory: &WritableVmo, payload: Payload) -> Result<()> {
    let mut buffer = vec![0; hyper_os::memory::MAX_TRANSFER_BYTES];
    let offset = payload
        .load_address
        .checked_sub(RAM_BASE)
        .ok_or("payload below RAM")?;
    let mut copied = 0;
    while copied < payload.length {
        let length = (payload.length - copied).min(buffer.len() as u64) as usize;
        source
            .read_exact_at(payload.file_offset + copied, &mut buffer[..length])
            .map_err(show)?;
        memory
            .write_all_at(offset + copied, &buffer[..length])
            .map_err(show)?;
        copied += length as u64;
    }
    Ok(())
}

struct Guest {
    machine: hyper_os::OwnedHandle<VirtualMachineObject>,
    cpus: Vec<hyper_os::OwnedHandle<VirtualCpuObject>>,
    output: Output,
    started: bool,
    retired: bool,
    tail: Vec<u8>,
    disk_pass: bool,
}
impl Guest {
    fn drain(&mut self) -> Result<()> {
        let mut bytes = [0; 2048];
        for _ in 0..16 {
            let length = self.output.try_read(&mut bytes).map_err(show)?;
            if length == 0 {
                break;
            }
            io::stdout().write_all(&bytes[..length]).map_err(show)?;
            self.tail.extend_from_slice(&bytes[..length]);
            self.disk_pass |= self
                .tail
                .windows(DISK_PASS.len())
                .any(|window| window == DISK_PASS);
            if self.tail.len() > 4096 {
                self.tail.drain(..self.tail.len() - 4096);
            }
        }
        Ok(())
    }
    fn stop(&mut self) -> Result<()> {
        if self.retired {
            return Ok(());
        }
        vm::request_stop(self.machine.as_handle_ref())
            .map_err(|error| format!("request stop: {error:?}"))?;
        vm::wait_terminated(self.machine.as_handle_ref(), deadline(20)?)
            .map_err(|error| format!("wait terminated: {error:?}"))?;
        self.retired = true;
        self.drain()
    }
    fn power(&mut self) -> Result<bool> {
        if self.retired {
            return Ok(true);
        }
        for _ in 0..self.cpus.len() {
            let Some(request) =
                vm::pending_power_request(self.machine.as_handle_ref()).map_err(show)?
            else {
                break;
            };
            match request.operation {
                PowerOperation::CpuOn | PowerOperation::CpuOff => {
                    vm::complete_power_request(self.machine.as_handle_ref(), request.id, true)
                        .map_err(show)?;
                }
                PowerOperation::SystemOff | PowerOperation::SystemReset => {
                    self.stop()?;
                    return Ok(true);
                }
            }
        }
        for cpu in &self.cpus {
            if let Some(reason) = vm::vcpu_info(cpu.as_handle_ref()).map_err(show)?.terminal {
                return Err(format!("guest vCPU terminated unexpectedly: {reason:?}"));
            }
        }
        Ok(false)
    }
}

fn install(
    startup: &Startup<'_>,
    image: &Image,
    own: &hyper_os::OwnedHandle<hyper_os::handle::GuestMemoryObject>,
    shared: Option<&hyper_os::OwnedHandle<hyper_os::handle::GuestMemoryObject>>,
    physical: Option<&hyper_os::OwnedHandle<hyper_os::handle::PhysicalDeviceObject>>,
    serial_address: u64,
) -> Result<Guest> {
    let lease = vm::derive_creation_lease(
        startup
            .borrow(startup::VIRTUAL_MACHINE_CREATION_AUTHORITY)
            .map_err(show)?,
        startup.borrow(startup::RESOURCE_DOMAIN).map_err(show)?,
    )
    .map_err(show)?;
    let pending = vm::create(
        lease,
        vm::Configuration {
            guest_physical_base: RAM_BASE,
            memory_size: if shared.is_some() {
                RAM_BYTES * 2
            } else {
                RAM_BYTES
            },
            vcpu_count: image.plan.vcpu_count(),
            architecture: vm::Architecture::Aarch64,
            platform_profile: vm::PlatformProfile::Aarch64Reference,
        },
    )
    .map_err(|error| show(error.error()))?;
    vm::map_guest_memory(
        pending.as_handle_ref(),
        own.as_handle_ref(),
        0,
        0,
        RAM_BYTES,
    )
    .map_err(show)?;
    if let Some(shared) = shared {
        vm::map_guest_memory(
            pending.as_handle_ref(),
            shared.as_handle_ref(),
            RAM_BYTES,
            0,
            RAM_BYTES,
        )
        .map_err(show)?;
    }
    if let Some(physical) = physical {
        device::assign(
            pending.as_handle_ref(),
            physical.as_handle_ref(),
            PHYSICAL_MMIO,
            40,
        )
        .map_err(show)?;
    }
    let serial = virtual_serial::create().map_err(show)?;
    let output = Output::register(
        &serial,
        startup.borrow(startup::ROOT_VMAR).map_err(show)?,
        serial_address,
        WritableVmo::create(virtual_serial::BUFFER_BYTES).map_err(show)?,
    )
    .map_err(show)?;
    let binding = serial
        .duplicate(Rights::ASSIGN_DEVICE.union(Rights::TRANSFER))
        .map_err(show)?;
    vm::set_virtual_serial(pending.as_handle_ref(), binding)
        .map_err(|error| show(error.error()))?;
    vm::set_bootstrap(
        pending.as_handle_ref(),
        vm::VirtualCpuBootstrap {
            entry: image.plan.kernel_entry(),
            stack: 0,
            arguments: image.plan.bootstrap_arguments(),
        },
    )
    .map_err(show)?;
    vm::seal(pending.as_handle_ref()).map_err(show)?;
    let (machine, cpu) = vm::install(pending).map_err(|error| show(error.error()))?;
    let mut cpus = vec![cpu];
    for index in 1..image.plan.vcpu_count() {
        cpus.push(vm::open_vcpu(machine.as_handle_ref(), index).map_err(show)?);
    }
    Ok(Guest {
        machine,
        cpus,
        output,
        started: false,
        retired: false,
        tail: Vec::new(),
        disk_pass: false,
    })
}

fn complete(guest: &Guest, completion: Completion) -> Result<()> {
    let cpu = guest
        .cpus
        .get(completion.vcpu)
        .ok_or("invalid completion CPU")?;
    vm::complete_mmio(cpu.as_handle_ref(), completion.id, completion.result).map_err(show)
}

fn supervise(guests: &mut [Guest], backend: &mut Backend) -> Result<()> {
    let limit = deadline(180)?;
    vm::start_vcpu(guests[0].cpus[0].as_handle_ref()).map_err(show)?;
    guests[0].started = true;
    loop {
        for guest in &mut *guests {
            guest.drain()?;
        }
        if guests[0].power()? {
            return Err("I/O VM stopped before business completed".into());
        }
        if guests[1].started && guests[1].power()? {
            if !guests[1].disk_pass {
                return Err("business powered off without disk PASS".into());
            }
            return Ok(());
        }
        if let Some(completion) = backend.progress().map_err(show)? {
            complete(&guests[1], completion)?;
        }
        if backend.ready() && !guests[1].started {
            vm::start_vcpu(guests[1].cpus[0].as_handle_ref()).map_err(show)?;
            guests[1].started = true;
            println!("IO-VM-SMOKE: backend negotiated; business guest started");
        }
        if !backend.busy() && guests[1].started {
            for (index, cpu) in guests[1].cpus.iter().enumerate() {
                if let Some(request) = vm::pending_mmio(cpu.as_handle_ref()).map_err(show)? {
                    if let Some(completion) = backend.mmio(index, request).map_err(show)? {
                        complete(&guests[1], completion)?;
                    }
                    if backend.busy() {
                        break;
                    }
                }
            }
        }
        // Level waits contain only work we can service. Pending MMIO is omitted
        // during a backend transaction, and TX_SPACE is omitted after sending.
        let mut items = Vec::new();
        for guest in &*guests {
            if !guest.retired {
                items.push(guest.output.wait_item());
                items.push(WaitItem::new(
                    guest.machine.as_handle_ref(),
                    ObjectSignals::<VirtualMachineObject>::POWER_REQUEST
                        .union(ObjectSignals::<VirtualMachineObject>::VCPU_TERMINATED),
                ));
            }
        }
        let mut mailbox_signals = ObjectSignals::<GuestMailboxObject>::PEER_CLOSED;
        if backend.busy() {
            mailbox_signals = mailbox_signals.union(ObjectSignals::<GuestMailboxObject>::READABLE);
        }
        if backend.wants_write() {
            mailbox_signals = mailbox_signals.union(ObjectSignals::<GuestMailboxObject>::WRITABLE);
        }
        let mailbox_index = items.len();
        items.push(WaitItem::new(
            backend.mailbox().as_handle_ref(),
            mailbox_signals,
        ));
        let closed_index = items.len();
        items.push(WaitItem::new(
            backend.notification().as_handle_ref(),
            ObjectSignals::<GuestNotificationObject>::PEER_CLOSED,
        ));
        if !backend.busy() && guests[1].started {
            for cpu in &guests[1].cpus {
                items.push(WaitItem::new(
                    cpu.as_handle_ref(),
                    ObjectSignals::<VirtualCpuObject>::MMIO_REQUEST,
                ));
            }
        }
        let observed = wait::wait_many(&items, limit).map_err(show)?;
        if observed.index == mailbox_index
            && ObjectSignals::<GuestMailboxObject>::PEER_CLOSED.is_present_in(observed.observed)
        {
            return Err("control mailbox disconnected".into());
        }
        if observed.index == closed_index {
            backend.disconnected().map_err(show)?;
            return Err("notification backend disconnected".into());
        }
    }
}

fn suite(startup: &Startup<'_>) -> Result<()> {
    println!("IO-VM-SMOKE: preparing physical disk deployment");
    let authority = startup
        .borrow(startup::DEVICE_ASSIGNMENT_AUTHORITY)
        .map_err(show)?;
    let physical = device::claim(authority, 0).map_err(show)?;
    println!("IO-VM-SMOKE: physical device claimed");
    let physical_info = device::info(physical.as_handle_ref()).map_err(show)?;
    if physical_info.device_id != 8 || physical_info.transport_version != 2 {
        return Err("expected modern physical virtio-scsi".into());
    }
    println!("IO-VM-SMOKE: loading business image");
    let front =
        Image::load("/vm/business.itb").map_err(|error| format!("load business image: {error}"))?;
    println!("IO-VM-SMOKE: loading I/O image");
    let io = Image::load("/vm/io.itb").map_err(|error| format!("load I/O image: {error}"))?;
    let io_extent =
        device::dma_extent(authority, io.memory.as_handle_ref(), 0, RAM_BYTES).map_err(show)?;
    let front_extent =
        device::dma_extent(authority, front.memory.as_handle_ref(), 0, RAM_BYTES).map_err(show)?;
    println!(
        "IO-VM-SMOKE: DMA RAM host={:#x}/{:#x}, Linux aliases={RAM_BASE:#x}/{SHARED_BASE:#x}",
        io_extent.physical_base, front_extent.physical_base
    );
    if io_extent.physical_base == RAM_BASE || front_extent.physical_base == SHARED_BASE {
        return Err(
            "fixture placement did not exercise nonidentity DMA for both RAM regions".into(),
        );
    }
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
        front.plan.architecture(),
        front.plan.platform_profile(),
        platform,
    )
    .map_err(show)?;
    let guest_fdt::GuestHardwareMetadata::Aarch64 { gic_version } = metadata else {
        return Err("non-AArch64 metadata".into());
    };
    drop(lease);
    let dma = [
        DmaRange {
            dma_base: io_extent.physical_base,
            cpu_base: RAM_BASE,
            size: RAM_BYTES,
        },
        DmaRange {
            dma_base: front_extent.physical_base,
            cpu_base: SHARED_BASE,
            size: RAM_BYTES,
        },
    ];
    io.device_tree(
        gic_version,
        IoDevices {
            virtio: Some(MmioDevice {
                base: PHYSICAL_MMIO,
                size: physical_info.mmio_size,
                irq: 40,
            }),
            dma_ranges: &dma,
            shared_memory: Some(SharedMemory {
                base: SHARED_BASE,
                size: RAM_BYTES,
                guest_base: RAM_BASE,
            }),
            mailbox: Some(device_node(MAILBOX_MMIO, 41)),
            notification: Some(device_node(NOTIFICATION_MMIO, 42)),
        },
    )?;
    front.device_tree(
        gic_version,
        IoDevices {
            virtio: Some(device_node(FRONT_MMIO, 40)),
            ..IoDevices::empty()
        },
    )?;
    println!("IO-VM-SMOKE: device trees prepared");
    let io_grant = vm::create_guest_memory(io.memory.as_handle_ref()).map_err(show)?;
    let front_grant = vm::create_guest_memory(front.memory.as_handle_ref()).map_err(show)?;
    let mut guests = Vec::new();
    let outcome = (|| {
        guests.push(install(
            startup,
            &io,
            &io_grant,
            Some(&front_grant),
            Some(&physical),
            0xd000_0000,
        )?);
        guests.push(install(
            startup,
            &front,
            &front_grant,
            None,
            None,
            0xd000_0000 + virtual_serial::BUFFER_BYTES,
        )?);
        println!("IO-VM-SMOKE: both guests installed");
        // Installed RAM layouts now own every hardware lease. Do not retain a
        // separate grant object across the final VM retirement assertion.
        drop(io_grant);
        drop(front_grant);
        let device_id = NonZeroU64::new(1).ok_or("device identity")?;
        vm::register_mmio(
            guests[1].machine.as_handle_ref(),
            FRONT_MMIO,
            4096,
            device_id,
        )
        .map_err(show)?;
        let mailbox =
            Mailbox::create(guests[0].machine.as_handle_ref(), MAILBOX_MMIO, 41).map_err(show)?;
        let notification = Notification::create(
            guests[1].machine.as_handle_ref(),
            guests[0].machine.as_handle_ref(),
            FRONT_MMIO,
            NOTIFICATION_MMIO,
            40,
            42,
        )
        .map_err(show)?;
        let mut backend = Backend::new(
            mailbox,
            notification,
            (RAM_BASE, RAM_BYTES),
            FRONT_MMIO,
            device_id,
        )
        .map_err(show)?;
        supervise(&mut guests, &mut backend)
    })();
    // Stop frontend first; IO VM retains both grants until physical reset after
    // its final vCPU quiescence. A failed stop cannot free kernel-owned DMA RAM.
    let mut cleanup = Ok(());
    for guest in guests.iter_mut().rev() {
        if let Err(error) = guest.stop() {
            cleanup = Err(error);
        }
    }
    outcome.map_err(|error| format!("supervision: {error}"))?;
    cleanup.map_err(|error| format!("cleanup: {error}"))?;
    println!("IO-VM-SMOKE: both guests stopped; checking DMA admission");
    drop(guests);
    // Retired diagnostic/device handles cannot pin a hardware-write admission.
    io.memory
        .write_all_at(0, &[0])
        .map_err(|error| format!("I/O RAM after retirement: {error:?}"))?;
    front
        .memory
        .write_all_at(0, &[0])
        .map_err(|error| format!("business RAM after retirement: {error:?}"))?;
    println!("IO-VM-SMOKE: DMA backing writable after both VMs retired");
    Ok(())
}

pub(super) fn main() -> ExitCode {
    let startup = match hyper_rt::process::startup() {
        Ok(startup) => startup,
        Err(error) => {
            eprintln!("IO-VM-SMOKE: FAIL startup {error:?}");
            return ExitCode::FAILURE;
        }
    };
    // Keep bootstrap stdout and the self TaskGroup alive until the result has
    // been printed; Startup Drop may initiate this fixture's own teardown.
    match suite(&startup) {
        Ok(()) => {
            println!("IO-VM-SMOKE: PASS");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("IO-VM-SMOKE: FAIL {error}");
            ExitCode::FAILURE
        }
    }
}
