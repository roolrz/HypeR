// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Bound parser allocation work with a relocating allocator, as in the kernel.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use hyper::exec::elf::Image;

thread_local! {
    static ALLOCATIONS: Cell<Option<(usize, usize)>> = const { Cell::new(None) };
}

struct CountingAllocator;

// SAFETY: All allocation and deallocation delegate unchanged to System. The
// default realloc allocates and copies, matching the kernel allocator contract.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = ALLOCATIONS.try_with(|counts| {
            if let Some((calls, bytes)) = counts.get() {
                counts.set(Some((calls + 1, bytes + layout.size())));
            }
        });
        // SAFETY: The caller supplies the allocator's valid requested layout.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: The caller returns the exact System allocation and layout.
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn word(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn dense_relr_image(groups: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; 0x3000];
    bytes[..9].copy_from_slice(&[0x7f, b'E', b'L', b'F', 2, 1, 1, 63, 0]);
    for (offset, value) in [(16, 3u16), (18, 183), (52, 64), (54, 56), (56, 3)] {
        bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }
    bytes[20] = 1;
    word(&mut bytes, 32, 64);
    for (index, (kind, flags, offset, address, file_size, memory_size, alignment)) in [
        (1u32, 5u32, 0x1000, 0, 4, 0x1000, 0x1000),
        (1, 6, 0x2000, 0x2000, 0x1000, 0x10000, 0x1000),
        (2, 6, 0x2000, 0x2000, 64, 64, 8),
    ]
    .into_iter()
    .enumerate()
    {
        let header = 64 + index * 56;
        bytes[header..header + 4].copy_from_slice(&kind.to_le_bytes());
        bytes[header + 4..header + 8].copy_from_slice(&flags.to_le_bytes());
        for (field, value) in [
            (8, offset),
            (16, address),
            (32, file_size),
            (40, memory_size),
            (48, alignment),
        ] {
            word(&mut bytes, header + field, value);
        }
    }
    for (index, (tag, value)) in [(36, 0x2100), (35, (groups as u64 + 1) * 8), (37, 8), (0, 0)]
        .into_iter()
        .enumerate()
    {
        word(&mut bytes, 0x2000 + index * 16, tag);
        word(&mut bytes, 0x2008 + index * 16, value);
    }
    word(&mut bytes, 0x2100, 0x4000);
    for index in 1..=groups {
        word(&mut bytes, 0x2100 + index * 8, u64::MAX);
    }
    bytes
}

#[test]
fn relr_expansion_has_linear_allocation_volume() {
    for groups in [16, 32, 64] {
        let bytes = dense_relr_image(groups);
        ALLOCATIONS.set(Some((0, 0)));
        let result = Image::parse(&bytes);
        let counts = ALLOCATIONS.replace(None);
        let image = match result {
            Ok(image) => image,
            Err(error) => panic!("valid dense RELR image rejected: {error:?}"),
        };
        let Some((calls, allocated)) = counts else {
            panic!("missing allocation counters")
        };
        let relocations = groups * 63 + 1;
        assert_eq!(image.relocations().len(), relocations);
        assert!(
            calls < 32,
            "{relocations} relocations required {calls} allocations"
        );
        assert!(
            allocated < relocations * 128,
            "{relocations} relocations allocated {allocated} bytes"
        );
    }
}
