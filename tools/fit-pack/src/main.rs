// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use std::env;
use std::fs;
use std::io;
use std::path::Path;

const FDT_MAGIC: u32 = 0xd00d_feed;
const FDT_BEGIN_NODE: u32 = 1;
const FDT_END_NODE: u32 = 2;
const FDT_PROP: u32 = 3;
const FDT_END: u32 = 9;
const HEADER_SIZE: usize = 40;

fn main() {
    if let Err(error) = run() {
        eprintln!("hyper-fit-pack: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments: Vec<String> = env::args().collect();
    if arguments.len() != 7 {
        return Err(String::from(
            "usage: hyper-fit-pack OUTPUT ARCH KERNEL LOAD ENTRY INITRAMFS",
        ));
    }
    let output = argument(&arguments, 1)?;
    let architecture = argument(&arguments, 2)?;
    let kernel = read_file(argument(&arguments, 3)?)?;
    let load = parse_u64(argument(&arguments, 4)?)?;
    let entry = parse_u64(argument(&arguments, 5)?)?;
    let initramfs = read_file(argument(&arguments, 6)?)?;
    let bytes = build_image(ImageInput {
        architecture,
        kernel: &kernel,
        load,
        entry,
        initramfs: &initramfs,
    })?;
    if fs::read(output).ok().as_deref() == Some(bytes.as_slice()) {
        return Ok(());
    }
    fs::write(output, bytes).map_err(io_error)
}

struct ImageInput<'a> {
    architecture: &'a str,
    kernel: &'a [u8],
    load: u64,
    entry: u64,
    initramfs: &'a [u8],
}

fn target(architecture: &str) -> Result<hyper_vm_image::Architecture, String> {
    match architecture {
        "arm64" => Ok(hyper_vm_image::Architecture::Aarch64),
        "riscv" => Ok(hyper_vm_image::Architecture::Riscv64),
        _ => Err(String::from("supported architectures are arm64 and riscv")),
    }
}

fn build_image(input: ImageInput<'_>) -> Result<Vec<u8>, String> {
    let ImageInput {
        architecture,
        kernel,
        load,
        entry,
        initramfs,
    } = input;
    let image_architecture = target(architecture)?;
    if kernel.is_empty() || initramfs.is_empty() {
        return Err(String::from("payloads must be nonzero"));
    }

    let mut fit = Builder::new();
    fit.begin_node("");
    fit.property_u32("#address-cells", 2)?;
    fit.property_string("description", "HypeR guest image")?;
    fit.begin_node("images");
    fit.begin_node("kernel@1");
    fit.property_string("description", "Linux kernel")?;
    fit.property("data", kernel)?;
    fit.property_string("type", "kernel")?;
    fit.property_string("arch", architecture)?;
    fit.property_string("os", "linux")?;
    fit.property_string("compression", "none")?;
    fit.property_u64("load", load)?;
    fit.property_u64("entry", entry)?;
    fit.end_node();
    fit.begin_node("ramdisk@1");
    fit.property_string("description", "Linux initramfs")?;
    fit.property("data", initramfs)?;
    fit.property_string("type", "ramdisk")?;
    fit.property_string("arch", architecture)?;
    fit.property_string("os", "linux")?;
    fit.property_string("compression", "gzip")?;
    fit.end_node();
    fit.end_node();
    fit.begin_node("configurations");
    fit.property_string("default", "conf@1")?;
    fit.begin_node("conf@1");
    fit.property_string("compatible", hyper_vm_image::GUEST_IMAGE_COMPATIBLE)?;
    fit.property_string("kernel", "kernel@1")?;
    fit.property_string("ramdisk", "ramdisk@1")?;
    fit.end_node();
    fit.end_node();
    fit.end_node();
    let bytes = fit.finish()?;
    validate(
        &bytes,
        ValidationExpectations {
            architecture: image_architecture,
            kernel_load: load,
            kernel_entry: entry,
            kernel_length: kernel.len(),
            initramfs_length: initramfs.len(),
        },
    )?;
    Ok(bytes)
}

struct ValidationExpectations {
    architecture: hyper_vm_image::Architecture,
    kernel_load: u64,
    kernel_entry: u64,
    kernel_length: usize,
    initramfs_length: usize,
}

fn validate(bytes: &[u8], expected: ValidationExpectations) -> Result<(), String> {
    let image = hyper_vm_image::parse(&MemorySource(bytes))
        .map_err(|error| format!("generated FIT failed validation: {error:?}"))?;
    match image.architecture {
        hyper_vm_image::Architecture::Aarch64 => {
            hyper_vm_image::aarch64_linux::validate(&MemorySource(bytes), image.kernel)
                .map_err(|error| format!("invalid Linux kernel: {error:?}"))?;
        }
        hyper_vm_image::Architecture::Riscv64 => {
            hyper_vm_image::riscv64_linux::validate(&MemorySource(bytes), image.kernel)
                .map_err(|error| format!("invalid Linux kernel: {error:?}"))?;
        }
        _ => return Err("unsupported Linux architecture".into()),
    }
    let expected_kernel_length = u64::try_from(expected.kernel_length)
        .map_err(|_| String::from("kernel length exceeds u64"))?;
    let expected_initramfs_length = u64::try_from(expected.initramfs_length)
        .map_err(|_| String::from("initramfs length exceeds u64"))?;
    let initramfs_matches = image.initramfs.is_some_and(|payload| {
        payload.length == expected_initramfs_length
            && payload.compression == hyper_vm_image::Compression::Gzip
    });
    if image.architecture != expected.architecture
        || image.kernel.load_address != expected.kernel_load
        || image.kernel.entry_address != expected.kernel_entry
        || image.kernel.length != expected_kernel_length
        || !initramfs_matches
    {
        return Err(String::from("generated FIT metadata mismatch"));
    }
    Ok(())
}

struct MemorySource<'bytes>(&'bytes [u8]);

impl hyper_vm_image::ReadAt for MemorySource<'_> {
    type Error = ();

    fn length(&self) -> Result<u64, Self::Error> {
        u64::try_from(self.0.len()).map_err(|_| ())
    }

    fn read_exact_at(&self, offset: u64, output: &mut [u8]) -> Result<(), Self::Error> {
        let start = usize::try_from(offset).map_err(|_| ())?;
        let end = start.checked_add(output.len()).ok_or(())?;
        output.copy_from_slice(self.0.get(start..end).ok_or(())?);
        Ok(())
    }
}

fn argument(arguments: &[String], index: usize) -> Result<&str, String> {
    arguments
        .get(index)
        .map(String::as_str)
        .ok_or_else(|| String::from("missing argument"))
}

fn parse_u64(value: &str) -> Result<u64, String> {
    value
        .strip_prefix("0x")
        .map_or_else(|| value.parse::<u64>(), |hex| u64::from_str_radix(hex, 16))
        .map_err(|_| format!("invalid integer: {value}"))
}

fn read_file(path: &str) -> Result<Vec<u8>, String> {
    fs::read(Path::new(path)).map_err(io_error)
}

fn io_error(error: io::Error) -> String {
    error.to_string()
}

struct Builder {
    structure: Vec<u8>,
    strings: Vec<u8>,
}

impl Builder {
    const fn new() -> Self {
        Self {
            structure: Vec::new(),
            strings: Vec::new(),
        }
    }

    fn begin_node(&mut self, name: &str) {
        self.push_u32(FDT_BEGIN_NODE);
        self.structure.extend_from_slice(name.as_bytes());
        self.structure.push(0);
        self.pad();
    }

    fn end_node(&mut self) {
        self.push_u32(FDT_END_NODE);
    }

    fn property(&mut self, name: &str, value: &[u8]) -> Result<(), String> {
        let name_offset = self.name_offset(name)?;
        self.push_u32(FDT_PROP);
        self.push_u32(u32::try_from(value.len()).map_err(|_| String::from("property too large"))?);
        self.push_u32(name_offset);
        self.structure.extend_from_slice(value);
        self.pad();
        Ok(())
    }

    fn property_u32(&mut self, name: &str, value: u32) -> Result<(), String> {
        self.property(name, &value.to_be_bytes())
    }

    fn property_u64(&mut self, name: &str, value: u64) -> Result<(), String> {
        self.property(name, &value.to_be_bytes())
    }

    fn property_string(&mut self, name: &str, value: &str) -> Result<(), String> {
        let mut encoded = Vec::with_capacity(value.len() + 1);
        encoded.extend_from_slice(value.as_bytes());
        encoded.push(0);
        self.property(name, &encoded)
    }

    fn name_offset(&mut self, name: &str) -> Result<u32, String> {
        let mut offset = 0usize;
        while offset < self.strings.len() {
            let tail = self
                .strings
                .get(offset..)
                .ok_or_else(|| String::from("invalid string table"))?;
            let length = tail
                .iter()
                .position(|byte| *byte == 0)
                .ok_or_else(|| String::from("invalid string table"))?;
            if tail.get(..length) == Some(name.as_bytes()) {
                return u32::try_from(offset).map_err(|_| String::from("string table too large"));
            }
            offset += length + 1;
        }
        let result = u32::try_from(self.strings.len())
            .map_err(|_| String::from("string table too large"))?;
        self.strings.extend_from_slice(name.as_bytes());
        self.strings.push(0);
        Ok(result)
    }

    fn push_u32(&mut self, value: u32) {
        self.structure.extend_from_slice(&value.to_be_bytes());
    }

    fn pad(&mut self) {
        while !self.structure.len().is_multiple_of(4) {
            self.structure.push(0);
        }
    }

    fn finish(mut self) -> Result<Vec<u8>, String> {
        self.push_u32(FDT_END);
        let structure_offset = HEADER_SIZE + 16;
        let strings_offset = structure_offset
            .checked_add(self.structure.len())
            .ok_or_else(|| String::from("FIT size overflow"))?;
        let total_size = strings_offset
            .checked_add(self.strings.len())
            .ok_or_else(|| String::from("FIT size overflow"))?;
        let mut output = Vec::with_capacity(total_size);
        for value in [
            FDT_MAGIC,
            u32::try_from(total_size).map_err(|_| String::from("FIT exceeds u32"))?,
            u32::try_from(structure_offset).map_err(|_| String::from("FIT exceeds u32"))?,
            u32::try_from(strings_offset).map_err(|_| String::from("FIT exceeds u32"))?,
            HEADER_SIZE as u32,
            17,
            16,
            0,
            u32::try_from(self.strings.len()).map_err(|_| String::from("FIT exceeds u32"))?,
            u32::try_from(self.structure.len()).map_err(|_| String::from("FIT exceeds u32"))?,
        ] {
            output.extend_from_slice(&value.to_be_bytes());
        }
        output.extend_from_slice(&[0; 16]);
        output.extend_from_slice(&self.structure);
        output.extend_from_slice(&self.strings);
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn configuration(
        memory_size: u64,
        vcpu_count: u32,
    ) -> Result<hyper_vm_image::Configuration, String> {
        Ok(hyper_vm_image::Configuration {
            memory_size,
            vcpu_count,
            boot_arguments: hyper_vm_image::BootArguments::new("console=ttyAMA0")
                .map_err(|e| format!("{e:?}"))?,
        })
    }

    #[test]
    fn one_bundle_supports_different_machine_configurations() -> Result<(), String> {
        let bytes = build_image(ImageInput {
            architecture: "arm64",
            kernel: &linux_image(),
            load: 0x4020_0000,
            entry: 0x4020_0000,
            initramfs: &[0; 16],
        })?;
        let source = MemorySource(&bytes);
        let bundle = hyper_vm_image::parse(&source).map_err(|e| format!("{e:?}"))?;
        for key in [
            "hyper,memory-size",
            "hyper,vcpu-count",
            "bootargs",
            "hyper,platform-profile",
        ] {
            assert!(
                !bytes
                    .windows(key.len())
                    .any(|window| window == key.as_bytes())
            );
        }
        for memory in [64 * 1024 * 1024, 128 * 1024 * 1024, 256 * 1024 * 1024] {
            for cpus in [1, 2, 4, 8] {
                let image = bundle
                    .configure(configuration(memory, cpus)?)
                    .map_err(|e| format!("{e:?}"))?;
                let plan = hyper_vm_image::linux::validate_reference(&source, image)
                    .map_err(|e| format!("{e:?}"))?;
                assert_eq!(plan.memory_size(), memory);
                assert_eq!(plan.vcpu_count(), cpus);
                assert_eq!(
                    plan.initramfs().ok_or("missing initramfs")?.start(),
                    0x4000_0000 + memory - 4096
                );
            }
        }
        for cpus in [0, 9] {
            assert!(
                bundle
                    .configure(configuration(128 * 1024 * 1024, cpus)?)
                    .is_err()
            );
        }
        assert!(bundle.configure(configuration(1024, 1)?).is_err());
        let mut oversized = bundle;
        oversized
            .initramfs
            .as_mut()
            .ok_or("missing initramfs")?
            .length = 128 * 1024 * 1024;
        let image = oversized
            .configure(configuration(128 * 1024 * 1024, 1)?)
            .map_err(|e| format!("{e:?}"))?;
        assert!(hyper_vm_image::linux::validate_reference(&source, image).is_err());
        Ok(())
    }

    #[test]
    fn production_packer_checks_kernel_headers_without_machine_policy() -> Result<(), String> {
        for (arch, base) in [("arm64", 0x4000_0000), ("riscv", 0x8000_0000)] {
            let mut kernel = linux_image();
            if arch == "riscv" {
                kernel[32..36].copy_from_slice(&2u32.to_le_bytes());
                kernel[48..56].copy_from_slice(&0x0000_0056_4353_4952u64.to_le_bytes());
                kernel[56..60].copy_from_slice(&0x0543_5352u32.to_le_bytes());
            }
            let bytes = build_image(ImageInput {
                architecture: arch,
                kernel: &kernel,
                load: base + 0x20_0000,
                entry: base + 0x20_0000,
                initramfs: &[0; 16],
            })?;
            let image =
                hyper_vm_image::parse(&MemorySource(&bytes)).map_err(|e| format!("{e:?}"))?;
            assert_eq!(image.kernel.load_address, base + 0x20_0000);
            let configured = image
                .configure_for_host(configuration(128 * 1024 * 1024, 1)?, image.architecture)
                .map_err(|e| format!("{e:?}"))?;
            let plan = hyper_vm_image::linux::validate_reference(&MemorySource(&bytes), configured)
                .map_err(|e| format!("{e:?}"))?;
            assert_eq!(plan.architecture(), image.architecture);
            assert_eq!(plan.memory_base(), base);
            assert_eq!(
                plan.platform_profile(),
                if arch == "arm64" {
                    hyper_vm_image::PlatformProfile::Aarch64Reference
                } else {
                    hyper_vm_image::PlatformProfile::Riscv64Reference
                }
            );
            for host in [
                hyper_vm_image::Architecture::Aarch64,
                hyper_vm_image::Architecture::Riscv64,
                hyper_vm_image::Architecture::X86_64,
            ] {
                if host == image.architecture {
                    continue;
                }
                assert!(matches!(
                    image.configure_for_host(configuration(128 * 1024 * 1024, 1)?, host),
                    Err(hyper_vm_image::ConfigurationError::ArchitectureMismatch { .. })
                ));
            }
            if arch == "riscv" {
                assert!(matches!(
                    image.configure(configuration(128 * 1024 * 1024, 2)?),
                    Err(hyper_vm_image::ConfigurationError::InvalidVcpuCount)
                ));
            }

            assert!(
                build_image(ImageInput {
                    architecture: arch,
                    kernel: &kernel,
                    load: base + 0x20_0000,
                    entry: base + 0x20_0004,
                    initramfs: &[0; 16]
                })
                .is_err()
            );
        }
        assert!(
            build_image(ImageInput {
                architecture: "riscv",
                kernel: &linux_image(),
                load: 0x8020_0000,
                entry: 0x8020_0000,
                initramfs: &[0; 16]
            })
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn rejects_a_32_bit_kernel_load_address() -> Result<(), String> {
        let mut fit = Builder::new();
        fit.begin_node("");
        fit.begin_node("images");
        fit.begin_node("kernel@1");
        fit.property("data", &linux_image())?;
        fit.property_string("type", "kernel")?;
        fit.property_string("arch", "arm64")?;
        fit.property_string("os", "linux")?;
        fit.property_u32("load", 0x4020_0000)?;
        fit.end_node();
        fit.end_node();
        fit.begin_node("configurations");
        fit.property_string("default", "conf@1")?;
        fit.begin_node("conf@1");
        fit.property_string("compatible", hyper_vm_image::GUEST_IMAGE_COMPATIBLE)?;
        fit.property_string("kernel", "kernel@1")?;
        fit.end_node();
        fit.end_node();
        fit.end_node();
        let bytes = fit.finish()?;
        assert!(matches!(
            hyper_vm_image::parse(&MemorySource(&bytes)),
            Err(hyper_vm_image::Error::InvalidStructure)
        ));
        Ok(())
    }

    #[test]
    fn default_may_follow_its_configuration_node() -> Result<(), String> {
        let bytes = minimal_fit(Placement::DefaultAfterConfiguration)?;
        hyper_vm_image::parse(&MemorySource(&bytes))
            .map_err(|error| format!("parse failed: {error:?}"))?;
        Ok(())
    }

    #[test]
    fn rejects_an_obsolete_header_version() -> Result<(), String> {
        let mut bytes = minimal_fit(Placement::DefaultBeforeConfiguration)?;
        replace_u32(&mut bytes, 20, 16)?;
        assert!(matches!(
            hyper_vm_image::parse(&MemorySource(&bytes)),
            Err(hyper_vm_image::Error::InvalidHeader)
        ));
        Ok(())
    }

    #[test]
    fn rejects_overlapping_structure_and_string_blocks() -> Result<(), String> {
        let mut bytes = minimal_fit(Placement::DefaultBeforeConfiguration)?;
        let structure = read_u32(&bytes, 8)?;
        replace_u32(&mut bytes, 12, structure)?;
        assert!(matches!(
            hyper_vm_image::parse(&MemorySource(&bytes)),
            Err(hyper_vm_image::Error::InvalidHeader)
        ));
        Ok(())
    }

    #[test]
    fn rejects_a_property_outside_the_root_node() -> Result<(), String> {
        let mut fit = Builder::new();
        fit.begin_node("");
        fit.end_node();
        fit.property_string("stray", "value")?;
        let bytes = fit.finish()?;
        assert!(matches!(
            hyper_vm_image::parse(&MemorySource(&bytes)),
            Err(hyper_vm_image::Error::InvalidStructure)
        ));
        Ok(())
    }

    #[test]
    fn rejects_an_incompatible_guest_contract() -> Result<(), String> {
        let bytes = fit_with_contract(
            Placement::DefaultBeforeConfiguration,
            Some("hyper,guest-image-v1"),
            Some(hyper_vm_image::AARCH64_REFERENCE_PROFILE),
        )?;
        assert!(matches!(
            hyper_vm_image::parse(&MemorySource(&bytes)),
            Err(hyper_vm_image::Error::UnsupportedImage)
        ));
        Ok(())
    }

    #[test]
    fn rejects_children_beneath_selected_configuration_and_image_nodes() -> Result<(), String> {
        for nested_in_image in [false, true] {
            let mut fit = Builder::new();
            fit.begin_node("");
            fit.begin_node("images");
            fit.begin_node("kernel@1");
            fit.property("data", &linux_image())?;
            fit.property_string("type", "kernel")?;
            fit.property_string("arch", "arm64")?;
            fit.property_string("os", "linux")?;
            fit.property_u64("load", 0x4020_0000)?;
            if nested_in_image {
                fit.begin_node("metadata");
                fit.end_node();
                fit.property_u64("load", 0x4020_0000)?;
            }
            fit.end_node();
            fit.end_node();
            fit.begin_node("configurations");
            fit.property_string("default", "conf@1")?;
            fit.begin_node("conf@1");
            fit.property_string("compatible", hyper_vm_image::GUEST_IMAGE_COMPATIBLE)?;
            fit.property_string("kernel", "kernel@1")?;
            if !nested_in_image {
                fit.begin_node("metadata");
                fit.end_node();
                fit.property_string("kernel", "kernel@1")?;
            }
            fit.end_node();
            fit.end_node();
            fit.end_node();
            let bytes = fit.finish()?;
            assert!(matches!(
                hyper_vm_image::parse(&MemorySource(&bytes)),
                Err(hyper_vm_image::Error::InvalidStructure)
            ));
        }
        Ok(())
    }

    #[test]
    fn rejects_policy_in_bundle() -> Result<(), String> {
        let bytes = fit_with_contract(
            Placement::DefaultBeforeConfiguration,
            Some(hyper_vm_image::GUEST_IMAGE_COMPATIBLE),
            Some("aarch64-reference"),
        )?;
        assert!(matches!(
            hyper_vm_image::parse(&MemorySource(&bytes)),
            Err(hyper_vm_image::Error::UnsupportedImage)
        ));
        let bytes = fit_with_contract(Placement::DefaultBeforeConfiguration, None, None)?;
        assert!(hyper_vm_image::parse(&MemorySource(&bytes)).is_err());
        assert!(hyper_vm_image::BootArguments::new("console\0debug").is_err());
        assert!(hyper_vm_image::BootArguments::new(&"a".repeat(2049)).is_err());
        Ok(())
    }

    enum Placement {
        DefaultBeforeConfiguration,
        DefaultAfterConfiguration,
    }

    fn minimal_fit(placement: Placement) -> Result<Vec<u8>, String> {
        fit_with_contract(
            placement,
            Some(hyper_vm_image::GUEST_IMAGE_COMPATIBLE),
            None,
        )
    }

    fn fit_with_contract(
        placement: Placement,
        compatible: Option<&str>,
        platform_profile: Option<&str>,
    ) -> Result<Vec<u8>, String> {
        let mut fit = Builder::new();
        fit.begin_node("");
        fit.begin_node("images");
        fit.begin_node("kernel@1");
        fit.property("data", &linux_image())?;
        fit.property_string("type", "kernel")?;
        fit.property_string("arch", "arm64")?;
        fit.property_string("os", "linux")?;
        fit.property_u64("load", 0x4020_0000)?;
        fit.end_node();
        fit.end_node();
        fit.begin_node("configurations");
        if matches!(placement, Placement::DefaultBeforeConfiguration) {
            fit.property_string("default", "conf@1")?;
        }
        fit.begin_node("conf@1");
        if let Some(compatible) = compatible {
            fit.property_string("compatible", compatible)?;
        }
        fit.property_string("kernel", "kernel@1")?;

        if let Some(platform_profile) = platform_profile {
            fit.property_string("hyper,platform-profile", platform_profile)?;
        }
        fit.end_node();
        if matches!(placement, Placement::DefaultAfterConfiguration) {
            fit.property_string("default", "conf@1")?;
        }
        fit.end_node();
        fit.end_node();
        fit.finish()
    }

    fn linux_image() -> [u8; 64] {
        let mut image = [0u8; 64];
        image[8..16].copy_from_slice(&0x0020_0000_u64.to_le_bytes());
        image[16..24].copy_from_slice(&64_u64.to_le_bytes());
        image[56..60].copy_from_slice(&0x644d_5241_u32.to_le_bytes());
        image
    }

    fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
        let value = bytes
            .get(offset..offset + 4)
            .ok_or_else(|| String::from("header field missing"))?
            .try_into()
            .map_err(|_| String::from("header field malformed"))?;
        Ok(u32::from_be_bytes(value))
    }

    fn replace_u32(bytes: &mut [u8], offset: usize, value: u32) -> Result<(), String> {
        bytes
            .get_mut(offset..offset + 4)
            .ok_or_else(|| String::from("header field missing"))?
            .copy_from_slice(&value.to_be_bytes());
        Ok(())
    }
}
