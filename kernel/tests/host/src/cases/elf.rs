// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native ELF load-plan validation and relocation decoding.

use hyper::exec::elf::{Error, Image, ImageKind, Machine, Relocation};

const ELF_HEADER_SIZE: usize = 64;
const PROGRAM_HEADER_SIZE: usize = 56;

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn write_i64(bytes: &mut [u8], offset: usize, value: i64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn initialize_header(bytes: &mut [u8], kind: u16, entry: u64, program_count: u16) {
    bytes[..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    bytes[7] = hyper::abi::native::HYPER_NATIVE_ELF_OSABI as u8;
    bytes[8] = hyper::abi::native::HYPER_NATIVE_ELF_ABI_VERSION as u8;
    write_u16(bytes, 16, kind);
    write_u16(bytes, 18, 183);
    write_u32(bytes, 20, 1);
    write_u64(bytes, 24, entry);
    write_u64(bytes, 32, ELF_HEADER_SIZE as u64);
    write_u16(bytes, 52, ELF_HEADER_SIZE as u16);
    write_u16(bytes, 54, PROGRAM_HEADER_SIZE as u16);
    write_u16(bytes, 56, program_count);
}

#[test]
fn requires_hyper_native_elf_branding() {
    let mut image = executable_image();
    image[7] = 0;
    assert_eq!(
        Image::parse(&image).map(|_| ()),
        Err(Error::UnsupportedOperatingSystemAbi)
    );

    image[7] = hyper::abi::native::HYPER_NATIVE_ELF_OSABI as u8;
    image[8] = 1;
    assert_eq!(
        Image::parse(&image).map(|_| ()),
        Err(Error::UnsupportedAbiVersion)
    );
}

#[allow(clippy::too_many_arguments)]
fn write_program_header(
    bytes: &mut [u8],
    index: usize,
    kind: u32,
    flags: u32,
    file_offset: u64,
    virtual_address: u64,
    file_size: u64,
    memory_size: u64,
    alignment: u64,
) {
    let offset = ELF_HEADER_SIZE + index * PROGRAM_HEADER_SIZE;
    write_u32(bytes, offset, kind);
    write_u32(bytes, offset + 4, flags);
    write_u64(bytes, offset + 8, file_offset);
    write_u64(bytes, offset + 16, virtual_address);
    write_u64(bytes, offset + 32, file_size);
    write_u64(bytes, offset + 40, memory_size);
    write_u64(bytes, offset + 48, alignment);
}

fn executable_image() -> Vec<u8> {
    let mut bytes = vec![0u8; 0x2000];
    initialize_header(&mut bytes, 2, 0x20_0000, 1);
    write_program_header(&mut bytes, 0, 1, 5, 0x1000, 0x20_0000, 4, 0x100, 0x1000);
    bytes[0x1000..0x1004].copy_from_slice(&[0xc0, 0x03, 0x5f, 0xd6]);
    bytes
}

#[test]
fn plans_a_static_executable_without_weakening_permissions() {
    let bytes = executable_image();
    let image = crate::require_ok(Image::parse(&bytes));

    assert_eq!(image.kind(), ImageKind::Executable);
    assert_eq!(image.machine(), Machine::Aarch64);
    assert_eq!(image.entry(), 0x20_0000);
    let segment = crate::require_some(image.segments().next());
    assert!(segment.permissions().readable());
    assert!(segment.permissions().executable());
    assert!(!segment.permissions().writable());
    assert_eq!(segment.data(), &[0xc0, 0x03, 0x5f, 0xd6]);
    assert_eq!(image.relocations().len(), 0);
}

#[test]
fn rejects_writable_code_and_non_executable_entry_points() {
    let mut writable_code = executable_image();
    write_u32(&mut writable_code, ELF_HEADER_SIZE + 4, 7);
    assert_eq!(
        Image::parse(&writable_code).map(|_| ()),
        Err(Error::WritableExecutableSegment)
    );

    let mut bad_entry = executable_image();
    write_u64(&mut bad_entry, 24, 0x20_1000);
    assert_eq!(
        Image::parse(&bad_entry).map(|_| ()),
        Err(Error::InvalidEntry)
    );
}

#[test]
fn rejects_page_overlapping_load_segments() {
    let mut bytes = vec![0u8; 0x3000];
    initialize_header(&mut bytes, 2, 0x20_0000, 2);
    write_program_header(&mut bytes, 0, 1, 5, 0x1000, 0x20_0000, 4, 0x1800, 0x1000);
    write_program_header(&mut bytes, 1, 1, 6, 0x2000, 0x20_1000, 4, 0x1000, 0x1000);
    assert_eq!(
        Image::parse(&bytes).map(|_| ()),
        Err(Error::OverlappingLoadSegments)
    );
}

#[test]
fn decodes_supported_position_independent_relocations() {
    let mut bytes = vec![0u8; 0x3000];
    initialize_header(&mut bytes, 3, 0, 3);
    write_program_header(&mut bytes, 0, 1, 5, 0x1000, 0, 4, 0x1000, 0x1000);
    write_program_header(&mut bytes, 1, 1, 6, 0x2000, 0x2000, 0x300, 0x1000, 0x1000);
    write_program_header(&mut bytes, 2, 2, 6, 0x2000, 0x2000, 112, 112, 8);

    let dynamic = [
        (7i64, 0x2100u64),
        (8, 24),
        (9, 24),
        (36, 0x2120),
        (35, 16),
        (37, 8),
        (0, 0),
    ];
    for (index, (tag, value)) in dynamic.into_iter().enumerate() {
        let offset = 0x2000 + index * 16;
        write_i64(&mut bytes, offset, tag);
        write_u64(&mut bytes, offset + 8, value);
    }
    write_u64(&mut bytes, 0x2100, 0x2200);
    write_u64(&mut bytes, 0x2108, 1027);
    write_i64(&mut bytes, 0x2110, -8);
    write_u64(&mut bytes, 0x2120, 0x2208);
    write_u64(&mut bytes, 0x2128, 3);

    let image = crate::require_ok(Image::parse(&bytes));
    assert_eq!(image.kind(), ImageKind::PositionIndependent);
    let relocations: Vec<_> = image.relocations().collect();
    assert_eq!(
        relocations,
        vec![
            Relocation::Relative {
                target: 0x2200,
                addend: -8,
            },
            Relocation::RelativeInPlace { target: 0x2208 },
            Relocation::RelativeInPlace { target: 0x2210 },
        ]
    );

    write_u64(&mut bytes, 0x2100, 0);
    assert_eq!(
        Image::parse(&bytes).map(|_| ()),
        Err(Error::InvalidRelocation)
    );
}
