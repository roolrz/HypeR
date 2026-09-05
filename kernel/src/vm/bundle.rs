// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Versioned VM bundle discovery inside the firmware-provided boot ramdisk.

use core::str;

use crate::archive::cpio::{Archive, EntryKind};

const BOOT_CONFIG_PATH: &str = "hypervisor/boot.conf";
const BOOT_FORMAT: &str = "hyper-boot-v1";
const VM_FORMAT: &str = "hyper-vm-v1";
const MAX_VM_NAME_LENGTH: usize = 64;
const VM_BUNDLE_PREFIX: &str = "hypervisor/vms/";
const VM_BUNDLE_SUFFIX: &str = ".cpio";
const VM_BUNDLE_PATH_CAPACITY: usize =
    VM_BUNDLE_PREFIX.len() + MAX_VM_NAME_LENGTH + VM_BUNDLE_SUFFIX.len();
const MAX_COMMAND_LINE_LENGTH: usize = 2048;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    BootArchive(crate::archive::cpio::Error),
    BundleArchive(crate::archive::cpio::Error),
    DuplicateProperty,
    InvalidCommandLine,
    InvalidProperty,
    InvalidMemorySize,
    InvalidPath,
    InvalidText,
    InvalidVcpuCount,
    InvalidVmName,
    MissingBootConfiguration,
    MissingInitramfs,
    MissingKernel,
    MissingManifest,
    MissingProperty,
    MissingVmBundle,
    NotRegularFile,
    UnsupportedFormat,
    UnknownProperty,
}

#[derive(Clone, Copy)]
pub struct VmBundle<'a> {
    name: &'a str,
    guest_type: &'a str,
    architecture: &'a str,
    memory_size: u64,
    vcpu_count: u32,
    command_line: &'a str,
    kernel: &'a [u8],
    initramfs: Option<&'a [u8]>,
}

impl<'a> VmBundle<'a> {
    pub const fn name(&self) -> &'a str {
        self.name
    }

    pub const fn memory_size(&self) -> u64 {
        self.memory_size
    }

    pub const fn guest_type(&self) -> &'a str {
        self.guest_type
    }

    pub const fn architecture(&self) -> &'a str {
        self.architecture
    }

    pub const fn vcpu_count(&self) -> u32 {
        self.vcpu_count
    }

    pub const fn command_line(&self) -> &'a str {
        self.command_line
    }

    pub const fn kernel(&self) -> &'a [u8] {
        self.kernel
    }

    pub const fn initramfs(&self) -> Option<&'a [u8]> {
        self.initramfs
    }
}

pub fn select_default(ramdisk: &[u8]) -> Result<VmBundle<'_>, Error> {
    let outer = Archive::new(ramdisk).map_err(Error::BootArchive)?;
    let boot_entry = outer
        .find_unique(BOOT_CONFIG_PATH)
        .map_err(Error::BootArchive)?
        .ok_or(Error::MissingBootConfiguration)?;
    require_file(boot_entry.kind())?;
    let boot = parse_boot_configuration(text(boot_entry.data())?)?;
    let mut bundle_path_storage = [0u8; VM_BUNDLE_PATH_CAPACITY];
    let bundle_path = bundle_path(boot.default_vm, &mut bundle_path_storage)?;
    let bundle_entry = outer
        .find_unique(bundle_path)
        .map_err(Error::BootArchive)?
        .ok_or(Error::MissingVmBundle)?;
    require_file(bundle_entry.kind())?;

    let bundle = Archive::new(bundle_entry.data()).map_err(Error::BundleArchive)?;
    let manifest_entry = bundle
        .find_unique("manifest")
        .map_err(Error::BundleArchive)?
        .ok_or(Error::MissingManifest)?;
    require_file(manifest_entry.kind())?;
    let manifest = parse_manifest(text(manifest_entry.data())?)?;

    let kernel_entry = bundle
        .find_unique(manifest.kernel_path)
        .map_err(Error::BundleArchive)?
        .ok_or(Error::MissingKernel)?;
    require_file(kernel_entry.kind())?;
    let initramfs = match manifest.initramfs_path {
        Some(path) => {
            let entry = bundle
                .find_unique(path)
                .map_err(Error::BundleArchive)?
                .ok_or(Error::MissingInitramfs)?;
            require_file(entry.kind())?;
            Some(entry.data())
        }
        None => None,
    };

    Ok(VmBundle {
        name: boot.default_vm,
        guest_type: manifest.guest_type,
        architecture: manifest.architecture,
        memory_size: manifest.memory_size,
        vcpu_count: manifest.vcpu_count,
        command_line: manifest.command_line,
        kernel: kernel_entry.data(),
        initramfs,
    })
}

fn bundle_path<'a>(name: &str, storage: &'a mut [u8]) -> Result<&'a str, Error> {
    let prefix_end = VM_BUNDLE_PREFIX.len();
    let name_end = prefix_end
        .checked_add(name.len())
        .ok_or(Error::InvalidPath)?;
    let path_end = name_end
        .checked_add(VM_BUNDLE_SUFFIX.len())
        .ok_or(Error::InvalidPath)?;
    if path_end > storage.len() {
        return Err(Error::InvalidPath);
    }
    storage[..prefix_end].copy_from_slice(VM_BUNDLE_PREFIX.as_bytes());
    storage[prefix_end..name_end].copy_from_slice(name.as_bytes());
    storage[name_end..path_end].copy_from_slice(VM_BUNDLE_SUFFIX.as_bytes());
    str::from_utf8(&storage[..path_end]).map_err(|_| Error::InvalidPath)
}

struct BootConfiguration<'a> {
    default_vm: &'a str,
}

fn parse_boot_configuration(text: &str) -> Result<BootConfiguration<'_>, Error> {
    let mut format = None;
    let mut default_vm = None;
    for property in properties(text)? {
        let (name, value) = property?;
        match name {
            "format" => set_once(&mut format, value)?,
            "default" => set_once(&mut default_vm, value)?,
            _ => return Err(Error::UnknownProperty),
        }
    }
    if format.ok_or(Error::MissingProperty)? != BOOT_FORMAT {
        return Err(Error::UnsupportedFormat);
    }
    let default_vm = default_vm.ok_or(Error::MissingProperty)?;
    validate_vm_name(default_vm)?;
    Ok(BootConfiguration { default_vm })
}

struct Manifest<'a> {
    guest_type: &'a str,
    architecture: &'a str,
    memory_size: u64,
    vcpu_count: u32,
    command_line: &'a str,
    kernel_path: &'a str,
    initramfs_path: Option<&'a str>,
}

fn parse_manifest(text: &str) -> Result<Manifest<'_>, Error> {
    let mut format = None;
    let mut guest_type = None;
    let mut architecture = None;
    let mut memory = None;
    let mut vcpus = None;
    let mut command_line = None;
    let mut kernel = None;
    let mut initramfs = None;
    for property in properties(text)? {
        let (name, value) = property?;
        match name {
            "format" => set_once(&mut format, value)?,
            "type" => set_once(&mut guest_type, value)?,
            "architecture" => set_once(&mut architecture, value)?,
            "memory" => set_once(&mut memory, value)?,
            "vcpus" => set_once(&mut vcpus, value)?,
            "command_line" => set_once(&mut command_line, value)?,
            "kernel" => set_once(&mut kernel, value)?,
            "initramfs" => set_once(&mut initramfs, value)?,
            _ => return Err(Error::UnknownProperty),
        }
    }
    if format.ok_or(Error::MissingProperty)? != VM_FORMAT {
        return Err(Error::UnsupportedFormat);
    }
    let guest_type = guest_type.ok_or(Error::MissingProperty)?;
    let architecture = architecture.ok_or(Error::MissingProperty)?;
    if guest_type.is_empty() || architecture.is_empty() {
        return Err(Error::InvalidProperty);
    }
    let memory_size = parse_u64(memory.ok_or(Error::MissingProperty)?)?;
    if memory_size < 64 * 1024 * 1024
        || !memory_size.is_power_of_two()
        || memory_size & (crate::mm::PAGE_SIZE - 1) != 0
    {
        return Err(Error::InvalidMemorySize);
    }
    let vcpu_count = parse_u32(vcpus.ok_or(Error::MissingProperty)?)?;
    if vcpu_count == 0 {
        return Err(Error::InvalidVcpuCount);
    }
    let command_line = command_line.ok_or(Error::MissingProperty)?;
    if command_line.len() > MAX_COMMAND_LINE_LENGTH {
        return Err(Error::InvalidCommandLine);
    }
    let kernel_path = kernel.ok_or(Error::MissingProperty)?;
    validate_path(kernel_path)?;
    let initramfs_path = match initramfs {
        Some("") | None => None,
        Some(path) => {
            validate_path(path)?;
            Some(path)
        }
    };
    Ok(Manifest {
        guest_type,
        architecture,
        memory_size,
        vcpu_count,
        command_line,
        kernel_path,
        initramfs_path,
    })
}

fn properties(text: &str) -> Result<impl Iterator<Item = Result<(&str, &str), Error>>, Error> {
    if text.as_bytes().contains(&0) {
        return Err(Error::InvalidText);
    }
    Ok(text.lines().filter_map(|line| {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            None
        } else {
            Some(line.split_once('=').ok_or(Error::InvalidProperty))
        }
    }))
}

fn set_once<'a>(slot: &mut Option<&'a str>, value: &'a str) -> Result<(), Error> {
    if slot.replace(value).is_some() {
        Err(Error::DuplicateProperty)
    } else {
        Ok(())
    }
}

fn validate_vm_name(name: &str) -> Result<(), Error> {
    if name.is_empty()
        || name.len() > MAX_VM_NAME_LENGTH
        || matches!(name, "." | "..")
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        Err(Error::InvalidVmName)
    } else {
        Ok(())
    }
}

fn validate_path(path: &str) -> Result<(), Error> {
    if path.is_empty()
        || path.starts_with('/')
        || path.ends_with('/')
        || path
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
    {
        Err(Error::InvalidPath)
    } else {
        Ok(())
    }
}

fn parse_u64(value: &str) -> Result<u64, Error> {
    value.parse().map_err(|_| Error::InvalidMemorySize)
}

fn parse_u32(value: &str) -> Result<u32, Error> {
    value.parse().map_err(|_| Error::InvalidVcpuCount)
}

fn require_file(kind: EntryKind) -> Result<(), Error> {
    if kind == EntryKind::File {
        Ok(())
    } else {
        Err(Error::NotRegularFile)
    }
}

fn text(bytes: &[u8]) -> Result<&str, Error> {
    str::from_utf8(bytes).map_err(|_| Error::InvalidText)
}
