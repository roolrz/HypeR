// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Shared image preparation and installation for physical-I/O deployments.

use hyper_os::handle::{Rights, VirtualCpuObject, VirtualMachineObject};
use hyper_os::memory::WritableVmo;
use hyper_os::startup::{self, Startup};
use hyper_os::virtual_serial::Output;
use hyper_os::vm;
use hyper_os::{device, virtual_serial};
use hyper_vm_image::guest_fdt::io::IoDevices;
use hyper_vm_image::{Payload, ReadAt, guest_fdt, linux};
use std::fs::File;
use std::io;
use std::os::hyper::fs::FileExt;

type Result<T> = std::result::Result<T, String>;
pub const RAM_BASE: u64 = 0x4000_0000;
pub const RAM_BYTES: u64 = 64 * 1024 * 1024;
pub const PHYSICAL_MMIO: u64 = 0x0b00_0000;
fn show(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}

pub struct InstalledGuest {
    pub machine: hyper_os::OwnedHandle<VirtualMachineObject>,
    pub cpus: Vec<hyper_os::OwnedHandle<VirtualCpuObject>>,
    pub output: Output,
}

pub struct Image {
    pub memory: WritableVmo,
    pub plan: linux::BootPlan,
    pub arguments: String,
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
    pub fn load(path: &str) -> Result<Self> {
        let file = File::open(path).map_err(|error| format!("open {path}: {error}"))?;
        let length = file
            .metadata()
            .map_err(|error| format!("stat {path}: {error}"))?
            .len();
        let source = Source { file, length };
        let image = hyper_vm_image::parse(&source).map_err(show)?;
        let plan = linux::validate_reference(&source, image).map_err(show)?;
        if plan.memory_base() != RAM_BASE
            || plan.memory_size() != RAM_BYTES
            || plan.architecture() != hyper_vm_image::Architecture::Aarch64
        {
            return Err("I/O deployment requires 64 MiB AArch64 reference images".into());
        }
        println!("HypeR I/O loader: validated {path}, allocating guest RAM");
        let memory = WritableVmo::create_contiguous(RAM_BYTES).map_err(show)?;
        println!("HypeR I/O loader: copying {path} kernel");
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
    pub fn device_tree(&self, gic_version: u32, devices: IoDevices<'_>) -> Result<()> {
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

pub fn install(
    startup: &Startup<'_>,
    image: &Image,
    own: &hyper_os::OwnedHandle<hyper_os::handle::GuestMemoryObject>,
    shared: Option<&hyper_os::OwnedHandle<hyper_os::handle::GuestMemoryObject>>,
    physical: Option<&hyper_os::OwnedHandle<hyper_os::handle::PhysicalDeviceObject>>,
    serial_address: u64,
) -> Result<InstalledGuest> {
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
    Ok(InstalledGuest {
        machine,
        cpus,
        output,
    })
}
