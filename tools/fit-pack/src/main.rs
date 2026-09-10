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
    if arguments.len() != 10 {
        return Err(String::from(
            "usage: hyper-fit-pack OUTPUT ARCH MEMORY VCPUS KERNEL LOAD ENTRY INITRAMFS BOOTARGS",
        ));
    }
    let output = argument(&arguments, 1)?;
    let architecture = argument(&arguments, 2)?;
    let memory_size = parse_u64(argument(&arguments, 3)?)?;
    let vcpu_count = parse_u32(argument(&arguments, 4)?)?;
    let kernel = read_file(argument(&arguments, 5)?)?;
    let load = parse_u64(argument(&arguments, 6)?)?;
    let entry = parse_u64(argument(&arguments, 7)?)?;
    let initramfs = read_file(argument(&arguments, 8)?)?;
    let boot_arguments = argument(&arguments, 9)?;
    let bytes = build_image(ImageInput {
        architecture,
        memory_size,
        vcpu_count,
        kernel: &kernel,
        load,
        entry,
        initramfs: &initramfs,
        boot_arguments,
    })?;
    if fs::read(output).ok().as_deref() == Some(bytes.as_slice()) {
        return Ok(());
    }
    fs::write(output, bytes).map_err(io_error)
}

struct ImageInput<'a> {
    architecture: &'a str,
    memory_size: u64,
    vcpu_count: u32,
    kernel: &'a [u8],
    load: u64,
    entry: u64,
    initramfs: &'a [u8],
    boot_arguments: &'a str,
}

fn target(
    architecture: &str,
) -> Result<
    (
        hyper_vm_image::Architecture,
        hyper_vm_image::PlatformProfile,
    ),
    String,
> {
    match architecture {
        "arm64" => Ok((
            hyper_vm_image::Architecture::Aarch64,
            hyper_vm_image::PlatformProfile::Aarch64Reference,
        )),
        "riscv" => Ok((
            hyper_vm_image::Architecture::Riscv64,
            hyper_vm_image::PlatformProfile::Riscv64Reference,
        )),
        _ => Err(String::from(
            "supported reference architectures are arm64 and riscv",
        )),
    }
}

fn build_image(input: ImageInput<'_>) -> Result<Vec<u8>, String> {
    let ImageInput {
        architecture,
        memory_size,
        vcpu_count,
        kernel,
        load,
        entry,
        initramfs,
        boot_arguments,
    } = input;
    let (image_architecture, platform_profile) = target(architecture)?;
    if kernel.is_empty() || initramfs.is_empty() || memory_size == 0 || vcpu_count == 0 {
        return Err(String::from(
            "payload and machine dimensions must be nonzero",
        ));
    }
    let ramdisk_load = hyper_vm_image::linux::plan_initramfs_load(
        image_architecture,
        platform_profile,
        memory_size,
        initramfs.len() as u64,
    )
    .map_err(|error| format!("invalid initramfs placement: {error:?}"))?;

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
    fit.property_u64("load", ramdisk_load)?;
    fit.end_node();
    fit.end_node();
    fit.begin_node("configurations");
    fit.property_string("default", "conf@1")?;
    fit.begin_node("conf@1");
    fit.property_string("compatible", hyper_vm_image::GUEST_IMAGE_COMPATIBLE)?;
    fit.property_string("kernel", "kernel@1")?;
    fit.property_string("ramdisk", "ramdisk@1")?;
    fit.property_string("bootargs", boot_arguments)?;
    fit.property_u64("hyper,memory-size", memory_size)?;
    fit.property_u32("hyper,vcpu-count", vcpu_count)?;
    fit.property_string(
        "hyper,platform-profile",
        hyper_vm_image::linux::profile_name(platform_profile)
            .ok_or("unsupported platform profile")?,
    )?;
    fit.end_node();
    fit.end_node();
    fit.end_node();
    let bytes = fit.finish()?;
    validate(
        &bytes,
        ValidationExpectations {
            architecture: image_architecture,
            platform_profile,
            memory_size,
            vcpu_count,
            kernel_load: load,
            kernel_entry: entry,
            kernel_length: kernel.len(),
            initramfs_load: ramdisk_load,
            initramfs_length: initramfs.len(),
            boot_arguments,
        },
    )?;
    Ok(bytes)
}

struct ValidationExpectations<'arguments> {
    architecture: hyper_vm_image::Architecture,
    platform_profile: hyper_vm_image::PlatformProfile,
    memory_size: u64,
    vcpu_count: u32,
    kernel_load: u64,
    kernel_entry: u64,
    kernel_length: usize,
    initramfs_load: u64,
    initramfs_length: usize,
    boot_arguments: &'arguments str,
}

fn validate(bytes: &[u8], expected: ValidationExpectations<'_>) -> Result<(), String> {
    let image = hyper_vm_image::parse(&MemorySource(bytes))
        .map_err(|error| format!("generated FIT failed validation: {error:?}"))?;
    hyper_vm_image::linux::validate_reference(&MemorySource(bytes), image)
        .map_err(|error| format!("generated image violates platform layout: {error:?}"))?;
    let expected_kernel_length = u64::try_from(expected.kernel_length)
        .map_err(|_| String::from("kernel length exceeds u64"))?;
    let expected_initramfs_length = u64::try_from(expected.initramfs_length)
        .map_err(|_| String::from("initramfs length exceeds u64"))?;
    let initramfs_matches = image.initramfs.is_some_and(|payload| {
        payload.load_address == expected.initramfs_load
            && payload.entry_address == expected.initramfs_load
            && payload.length == expected_initramfs_length
            && payload.compression == hyper_vm_image::Compression::Gzip
    });
    if image.architecture != expected.architecture
        || image.memory_size != expected.memory_size
        || image.vcpu_count != expected.vcpu_count
        || image.kernel.load_address != expected.kernel_load
        || image.kernel.entry_address != expected.kernel_entry
        || image.kernel.length != expected_kernel_length
        || !initramfs_matches
        || image.boot_arguments.as_bytes() != expected.boot_arguments.as_bytes()
        || image.platform_profile != expected.platform_profile
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

fn parse_u32(value: &str) -> Result<u32, String> {
    parse_u64(value)
        .and_then(|value| u32::try_from(value).map_err(|_| String::from("integer exceeds u32")))
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

    #[test]
    fn production_packer_validates_both_reference_platforms() -> Result<(), String> {
        for (name, architecture, profile, base) in [
            (
                "arm64",
                hyper_vm_image::Architecture::Aarch64,
                hyper_vm_image::PlatformProfile::Aarch64Reference,
                0x4000_0000u64,
            ),
            (
                "riscv",
                hyper_vm_image::Architecture::Riscv64,
                hyper_vm_image::PlatformProfile::Riscv64Reference,
                0x8000_0000u64,
            ),
        ] {
            let mut kernel = linux_image();
            if name == "riscv" {
                kernel[32..36].copy_from_slice(&2u32.to_le_bytes());
                kernel[48..56].copy_from_slice(&0x0000_0056_4353_4952u64.to_le_bytes());
                kernel[56..60].copy_from_slice(&0x0543_5352u32.to_le_bytes());
            }
            let bytes = build_image(ImageInput {
                architecture: name,
                memory_size: 128 * 1024 * 1024,
                vcpu_count: 1,
                kernel: &kernel,
                load: base + 0x20_0000,
                entry: base + 0x20_0000,
                initramfs: &[0; 16],
                boot_arguments: "console=test",
            })?;
            let image = hyper_vm_image::parse(&MemorySource(&bytes))
                .map_err(|error| format!("{error:?}"))?;
            assert_eq!(image.architecture, architecture);
            assert_eq!(image.platform_profile, profile);
            let plan = hyper_vm_image::linux::validate_reference(&MemorySource(&bytes), image)
                .map_err(|error| format!("{error:?}"))?;
            assert_eq!(plan.memory_base(), base);
            assert_eq!(plan.kernel_entry(), base + 0x20_0000);
            let mut expected = complete_expectations("console=test", base + 0x07ff_f000);
            expected.architecture = architecture;
            expected.platform_profile = profile;
            expected.kernel_load = base + 0x20_0000;
            expected.kernel_entry = expected.kernel_load;
            validate(&bytes, expected)?;
        }
        Ok(())
    }

    #[test]
    fn production_packer_rejects_wrong_header_architecture_and_entry() {
        let kernel = linux_image();
        assert!(
            build_image(ImageInput {
                architecture: "riscv",
                memory_size: 128 * 1024 * 1024,
                vcpu_count: 1,
                kernel: &kernel,
                load: 0x8020_0000,
                entry: 0x8020_0000,
                initramfs: &[0; 16],
                boot_arguments: "",
            })
            .is_err()
        );
        assert!(
            build_image(ImageInput {
                architecture: "arm64",
                memory_size: 128 * 1024 * 1024,
                vcpu_count: 1,
                kernel: &kernel,
                load: 0x4020_0000,
                entry: 0x4020_0004,
                initramfs: &[0; 16],
                boot_arguments: "",
            })
            .is_err()
        );
        assert!(target("x86_64").is_err());
    }

    #[test]
    fn generated_fit_round_trips() -> Result<(), String> {
        let bytes = minimal_fit(Placement::DefaultBeforeConfiguration)?;
        let image = hyper_vm_image::parse(&MemorySource(&bytes))
            .map_err(|error| format!("parse failed: {error:?}"))?;
        assert_eq!(image.architecture, hyper_vm_image::Architecture::Aarch64);
        assert_eq!(image.memory_size, 128 * 1024 * 1024);
        assert_eq!(image.vcpu_count, 1);
        assert_eq!(image.kernel.length, 64);
        assert_eq!(image.boot_arguments.as_str(), "console=ttyAMA0");
        hyper_vm_image::aarch64_linux::validate_reference(&MemorySource(&bytes), image)
            .map_err(|error| format!("reference layout failed: {error:?}"))?;
        Ok(())
    }

    #[test]
    fn deterministic_validation_rejects_metadata_drift() -> Result<(), String> {
        let bytes = complete_fit()?;
        assert!(
            validate(
                &bytes,
                complete_expectations("console=ttyAMA0", 0x47ff_f000)
            )
            .is_ok()
        );
        assert!(validate(&bytes, complete_expectations("console=other", 0x47ff_f000)).is_err());
        assert!(
            validate(
                &bytes,
                complete_expectations("console=ttyAMA0", 0x47ff_e000)
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn rejects_a_32_bit_v1_load_address() -> Result<(), String> {
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
        fit.property_u64("hyper,memory-size", 128 * 1024 * 1024)?;
        fit.property_u32("hyper,vcpu-count", 1)?;
        fit.property_string(
            "hyper,platform-profile",
            hyper_vm_image::AARCH64_REFERENCE_PROFILE,
        )?;
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
    fn rejects_embedded_nul_in_boot_arguments() -> Result<(), String> {
        let bytes = fit_with_boot_arguments(b"console\0debug\0")?;
        assert!(matches!(
            hyper_vm_image::parse(&MemorySource(&bytes)),
            Err(hyper_vm_image::Error::InvalidString)
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
            b"console=ttyAMA0\0",
            Some("hyper,guest-image-v2"),
            Some(hyper_vm_image::AARCH64_REFERENCE_PROFILE),
        )?;
        assert!(matches!(
            hyper_vm_image::parse(&MemorySource(&bytes)),
            Err(hyper_vm_image::Error::UnsupportedImage)
        ));
        Ok(())
    }

    #[test]
    fn rejects_an_unknown_platform_profile() -> Result<(), String> {
        let bytes = fit_with_contract(
            Placement::DefaultBeforeConfiguration,
            b"console=ttyAMA0\0",
            Some(hyper_vm_image::GUEST_IMAGE_COMPATIBLE),
            Some("aarch64-other"),
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
            fit.property_u64("hyper,memory-size", 128 * 1024 * 1024)?;
            fit.property_u32("hyper,vcpu-count", 1)?;
            fit.property_string(
                "hyper,platform-profile",
                hyper_vm_image::AARCH64_REFERENCE_PROFILE,
            )?;
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
    fn rejects_a_missing_guest_contract_or_platform_profile() -> Result<(), String> {
        for (compatible, platform_profile) in [
            (None, Some(hyper_vm_image::AARCH64_REFERENCE_PROFILE)),
            (Some(hyper_vm_image::GUEST_IMAGE_COMPATIBLE), None),
        ] {
            let bytes = fit_with_contract(
                Placement::DefaultBeforeConfiguration,
                b"console=ttyAMA0\0",
                compatible,
                platform_profile,
            )?;
            assert!(hyper_vm_image::parse(&MemorySource(&bytes)).is_err());
        }
        Ok(())
    }

    enum Placement {
        DefaultBeforeConfiguration,
        DefaultAfterConfiguration,
    }

    fn minimal_fit(placement: Placement) -> Result<Vec<u8>, String> {
        fit_with_configuration(placement, b"console=ttyAMA0\0")
    }

    fn complete_fit() -> Result<Vec<u8>, String> {
        let mut fit = Builder::new();
        fit.begin_node("");
        fit.property_u32("#address-cells", 2)?;
        fit.begin_node("images");
        fit.begin_node("kernel@1");
        fit.property("data", &linux_image())?;
        fit.property_string("type", "kernel")?;
        fit.property_string("arch", "arm64")?;
        fit.property_string("os", "linux")?;
        fit.property_string("compression", "none")?;
        fit.property_u64("load", 0x4020_0000)?;
        fit.property_u64("entry", 0x4020_0000)?;
        fit.end_node();
        fit.begin_node("ramdisk@1");
        fit.property("data", &[0; 16])?;
        fit.property_string("type", "ramdisk")?;
        fit.property_string("arch", "arm64")?;
        fit.property_string("os", "linux")?;
        fit.property_string("compression", "gzip")?;
        fit.property_u64("load", 0x47ff_f000)?;
        fit.end_node();
        fit.end_node();
        fit.begin_node("configurations");
        fit.property_string("default", "conf@1")?;
        fit.begin_node("conf@1");
        fit.property_string("compatible", hyper_vm_image::GUEST_IMAGE_COMPATIBLE)?;
        fit.property_string("kernel", "kernel@1")?;
        fit.property_string("ramdisk", "ramdisk@1")?;
        fit.property_string("bootargs", "console=ttyAMA0")?;
        fit.property_u64("hyper,memory-size", 128 * 1024 * 1024)?;
        fit.property_u32("hyper,vcpu-count", 1)?;
        fit.property_string(
            "hyper,platform-profile",
            hyper_vm_image::AARCH64_REFERENCE_PROFILE,
        )?;
        fit.end_node();
        fit.end_node();
        fit.end_node();
        fit.finish()
    }

    fn complete_expectations(
        boot_arguments: &str,
        initramfs_load: u64,
    ) -> ValidationExpectations<'_> {
        ValidationExpectations {
            architecture: hyper_vm_image::Architecture::Aarch64,
            platform_profile: hyper_vm_image::PlatformProfile::Aarch64Reference,
            memory_size: 128 * 1024 * 1024,
            vcpu_count: 1,
            kernel_load: 0x4020_0000,
            kernel_entry: 0x4020_0000,
            kernel_length: 64,
            initramfs_load,
            initramfs_length: 16,
            boot_arguments,
        }
    }

    fn fit_with_boot_arguments(arguments: &[u8]) -> Result<Vec<u8>, String> {
        fit_with_configuration(Placement::DefaultBeforeConfiguration, arguments)
    }

    fn fit_with_configuration(
        placement: Placement,
        boot_arguments: &[u8],
    ) -> Result<Vec<u8>, String> {
        fit_with_contract(
            placement,
            boot_arguments,
            Some(hyper_vm_image::GUEST_IMAGE_COMPATIBLE),
            Some(hyper_vm_image::AARCH64_REFERENCE_PROFILE),
        )
    }

    fn fit_with_contract(
        placement: Placement,
        boot_arguments: &[u8],
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
        fit.property("bootargs", boot_arguments)?;
        fit.property_u64("hyper,memory-size", 128 * 1024 * 1024)?;
        fit.property_u32("hyper,vcpu-count", 1)?;
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
