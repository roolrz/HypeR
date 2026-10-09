// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::fixtures::{elf, put16, put64};
use std::io::Cursor;

#[test]
fn reads_program_headers_without_sections_or_whole_string_table() -> io::Result<()> {
    let path = "/lib/aarch64-hyper-hyper/ld-hyper-aarch64.so";
    let bytes = elf(&["libone.so", "libtwo.so"], None, Some(path), 183);
    let image = read(Cursor::new(bytes))?;
    assert_eq!(image.interpreter.as_deref(), Some(path));
    assert_eq!(image.needed, ["libone.so", "libtwo.so"]);
    assert!(!image.static_executable);
    let image = read(Cursor::new(elf(&[], None, None, 183)))?;
    assert!(image.static_executable);
    let image = read(Cursor::new(elf(&[], Some("libself.so"), None, 243)))?;
    assert!(!image.static_executable);
    assert_eq!(image.soname.as_deref(), Some("libself.so"));
    assert_eq!(image.architecture, Architecture::Riscv64);
    Ok(())
}

#[test]
fn rejects_truncated_overflowing_and_unmapped_metadata() {
    let original = elf(&["libone.so"], None, None, 183);
    for length in [0, 4, 63, 119, 4095] {
        assert!(read(Cursor::new(&original[..length])).is_err());
    }
    for (offset, value) in [
        (32, u64::MAX),      // Program-header table overflow.
        (64 + 16, u64::MAX), // LOAD address overflow.
        (120 + 8, u64::MAX), // Dynamic file offset overflow.
        (120 + 16, 0),       // Dynamic address outside LOAD.
        (120 + 32, MAX_DYNAMIC_BYTES + 16),
        (120 + 32, 17),
        (520, 9999),     // DT_NEEDED exceeds the string table.
        (536, 0x401000), // DT_STRTAB starts in zero-filled memory.
        (552, u64::MAX), // DT_STRSZ exceeds the segment.
    ] {
        let mut bytes = original.clone();
        put64(&mut bytes, offset, value);
        assert!(
            read(Cursor::new(bytes)).is_err(),
            "accepted invalid field at {offset}"
        );
    }
}

#[test]
fn rejects_ambiguous_segments_and_duplicate_or_unterminated_dynamic_metadata() {
    let original = elf(&["libone.so"], None, None, 183);
    let mut bytes = original.clone();
    put16(&mut bytes, 56, 3);
    bytes.copy_within(64..120, 176);
    assert!(read(Cursor::new(bytes)).is_err());
    let mut bytes = original.clone();
    put16(&mut bytes, 56, 3);
    bytes.copy_within(120..176, 176);
    assert!(read(Cursor::new(bytes)).is_err());
    let mut bytes = original.clone();
    put64(&mut bytes, 544, 5); // Second DT_STRTAB.
    assert!(read(Cursor::new(bytes)).is_err());
    let mut bytes = original;
    put64(&mut bytes, 560, 99); // Remove DT_NULL.
    assert!(read(Cursor::new(bytes)).is_err());
}

#[test]
fn bounds_names_and_dependency_counts_and_rejects_directory_escape() {
    for name in ["", ".", "..", "../libevil.so", "/libevil.so"] {
        assert!(read(Cursor::new(elf(&[name], None, None, 183))).is_err());
    }
    let overlong = "x".repeat(128);
    assert!(read(Cursor::new(elf(&[&overlong], None, None, 183))).is_err());
    assert!(read(Cursor::new(elf(&["libx.so"; 17], None, None, 183))).is_err());
    let mut bytes = elf(&["libx.so"], None, None, 183);
    bytes[2056] = b'x'; // Replace the final NUL, without extending DT_STRSZ.
    assert!(read(Cursor::new(bytes)).is_err());
}

#[test]
fn rejects_invalid_header_and_interpreter_encodings() {
    let original = elf(&[], None, Some("/loader.so"), 183);
    for (offset, value) in [
        (0, 0),
        (4, 1),
        (5, 2),
        (6, 2),
        (16, 1),
        (20, 2),
        (52, 0),
        (54, 0),
        (56, 0),
    ] {
        let mut bytes = original.clone();
        bytes[offset] = value;
        assert!(read(Cursor::new(bytes)).is_err());
    }
    assert!(read(Cursor::new(elf(&[], None, Some("relative.so"), 183))).is_err());
    let mut bytes = original;
    bytes[3073] = 0;
    assert!(read(Cursor::new(bytes)).is_err());
}

#[test]
fn metadata_io_is_bounded_even_with_large_trailing_debug_data() -> io::Result<()> {
    struct Meter {
        cursor: Cursor<Vec<u8>>,
        read: usize,
    }
    impl Read for Meter {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            let count = self.cursor.read(output)?;
            self.read += count;
            Ok(count)
        }
    }
    impl Seek for Meter {
        fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
            self.cursor.seek(position)
        }
    }
    let mut bytes = elf(&["libone.so"], None, None, 183);
    bytes.resize(1024 * 1024, 0);
    let mut input = Meter {
        cursor: Cursor::new(bytes),
        read: 0,
    };
    read(&mut input)?;
    assert!(
        input.read < 1024,
        "read {} bytes for a tiny dependency table",
        input.read
    );
    Ok(())
}
