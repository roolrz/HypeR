// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native initial-stack layout and rejection tests.

use hyper::exec::startup::{AuxiliaryValues, Error, Layout, StartupHandle};

fn word(bytes: &[u8], offset: usize) -> u64 {
    let mut encoded = [0_u8; 8];
    encoded.copy_from_slice(&bytes[offset..offset + 8]);
    u64::from_le_bytes(encoded)
}

#[test]
fn encodes_system_v_vectors_and_tagged_handles() {
    let arguments = ["/init"];
    let environment = ["MODE=test"];
    let layout = crate::require_ok(Layout::try_new(0x1_0000, &arguments, &environment, 1));
    let auxiliary = AuxiliaryValues {
        program_header: 0x20_0040,
        program_header_entry_size: 56,
        program_header_count: 9,
        interpreter_base: 0x1000_0000,
        program_entry: 0x20_1000,
    };
    let stack = crate::require_ok(layout.encode(
        auxiliary,
        &arguments,
        &environment,
        &[StartupHandle {
            purpose: crate::require_ok(u32::try_from(
                hyper::abi::native::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_CONSOLE,
            )),
            handle: 0x1234_5678,
        }],
    ));
    assert_eq!(stack.base(), layout.stack_pointer());
    assert_eq!(stack.base() & 0xf, 0);
    let bytes = stack.bytes();
    assert_eq!(word(bytes, 0), 1);
    let argv0 = word(bytes, 8);
    assert_eq!(word(bytes, 16), 0);
    let env0 = word(bytes, 24);
    assert_eq!(word(bytes, 32), 0);
    assert_eq!(word(bytes, 40), 3);
    assert_eq!(word(bytes, 48), auxiliary.program_header);
    assert_eq!(word(bytes, 56), 4);
    assert_eq!(word(bytes, 64), auxiliary.program_header_entry_size);
    assert_eq!(word(bytes, 72), 5);
    assert_eq!(word(bytes, 80), auxiliary.program_header_count);
    assert_eq!(word(bytes, 88), 6);
    assert_eq!(word(bytes, 96), hyper::mm::PAGE_SIZE);
    assert_eq!(word(bytes, 104), 7);
    assert_eq!(word(bytes, 112), auxiliary.interpreter_base);
    assert_eq!(word(bytes, 120), 9);
    assert_eq!(word(bytes, 128), auxiliary.program_entry);
    assert_eq!(
        word(bytes, 136),
        hyper::abi::native::HYPER_NATIVE_AUXV_STARTUP_HANDLES
    );
    let records = word(bytes, 144);
    assert_eq!(
        word(bytes, 152),
        hyper::abi::native::HYPER_NATIVE_AUXV_STARTUP_HANDLE_COUNT
    );
    assert_eq!(word(bytes, 160), 1);
    assert_eq!(word(bytes, 168), 0);
    assert_eq!(word(bytes, 176), 0);

    let argv_offset = crate::require_ok(usize::try_from(argv0 - stack.base()));
    let env_offset = crate::require_ok(usize::try_from(env0 - stack.base()));
    let record_offset = crate::require_ok(usize::try_from(records - stack.base()));
    assert_eq!(&bytes[argv_offset..argv_offset + 6], b"/init\0");
    assert_eq!(&bytes[env_offset..env_offset + 10], b"MODE=test\0");
    assert_eq!(
        u32::from_le_bytes(crate::require_ok(
            bytes[record_offset..record_offset + 4].try_into(),
        )),
        crate::require_ok(u32::try_from(
            hyper::abi::native::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_CONSOLE,
        ))
    );
    assert_eq!(word(bytes, record_offset + 8), 0x1234_5678);
}

#[test]
fn rejects_embedded_nul_and_layout_mismatch() {
    assert_eq!(
        Layout::try_new(0x1_0000, &["bad\0argument"], &[], 0),
        Err(Error::EmbeddedNul)
    );
    let layout = crate::require_ok(Layout::try_new(0x1_0000, &["/init"], &[], 0));
    assert!(matches!(
        layout.encode(
            AuxiliaryValues::minimal(0x20_1000),
            &["/a-much-longer-different-startup-image-name"],
            &[],
            &[],
        ),
        Err(Error::LayoutMismatch)
    ));
}

#[test]
fn supports_optional_and_console_startup_handle_sets() {
    for handle_count in [5_usize, 6] {
        let layout = crate::require_ok(Layout::try_new(0x2_0000, &["/init"], &[], handle_count));
        let handles: Vec<_> = (0..handle_count)
            .map(|index| StartupHandle {
                purpose: crate::require_ok(u32::try_from(index + 1)),
                handle: crate::require_ok(u64::try_from(index + 0x100)),
            })
            .collect();
        let stack = crate::require_ok(layout.encode(
            AuxiliaryValues::minimal(0x20_1000),
            &["/init"],
            &[],
            &handles,
        ));
        assert_eq!(
            word(stack.bytes(), 152),
            crate::require_ok(u64::try_from(handle_count))
        );
        assert_eq!(stack.base() & 0xf, 0);
    }
}

#[test]
fn rejects_oversized_and_underflowing_layouts() {
    let oversized = "x".repeat(64 * 1024);
    assert_eq!(
        Layout::try_new(0x2_0000, &[oversized.as_str()], &[], 0),
        Err(Error::TooLarge)
    );
    assert_eq!(
        Layout::try_new(32, &["/init"], &[], 0),
        Err(Error::AddressOverflow)
    );
}
