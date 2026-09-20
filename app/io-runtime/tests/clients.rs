// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0
use super::*;

#[test]
fn exact_board_identity_controls_authorization() {
    assert_eq!(
        parse("hyper.clients.v1\n0 config\n1 alpine\n").unwrap(),
        vec![Client {
            id: 1,
            volume: "alpine".into()
        }]
    );
    for invalid in [
        "hyper.clients.v1\n1 alpine\n",
        "hyper.clients.v1\n0 other\n",
        "hyper.clients.v1\n0 config\n1 vm\n1 other\n",
        "hyper.clients.v1\n0 config\n1 vm\n2 vm\n",
        "hyper.clients.v1\n0 config\n1 ../config\n",
        "hyper.clients.v1\n0 config\n1 vm extra\n",
    ] {
        assert!(parse(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn capability_records_are_canonical_and_checked() {
    let mut request = hyper_service::io::encode_connect(7, "vm-seven").unwrap();
    assert_eq!(
        hyper_service::io::decode_connect(&request),
        Some((7, "vm-seven"))
    );
    request[13] = 1;
    assert!(hyper_service::io::decode_connect(&request).is_none());
    assert!(hyper_service::io::encode_memory(u64::MAX - 4095, 4096).is_none());
    assert!(hyper_service::io::encode_memory(0x4000_0000, 0).is_none());
    let memory = hyper_service::io::encode_memory(0x4000_0000, 128 * 1024 * 1024).unwrap();
    assert_eq!(
        hyper_service::io::decode_memory(&memory),
        Some((0x4000_0000, 128 * 1024 * 1024))
    );
}

#[test]
fn dynamic_dma_translation_excludes_both_static_aliases() {
    let static_ranges = [
        DmaRange {
            dma_base: 0x8000,
            cpu_base: 0x4400_0000,
            size: 0x2000,
        },
        DmaRange {
            dma_base: 0x2000,
            cpu_base: 0x4000_0000,
            size: 0x4000,
        },
    ];
    let dynamic = dynamic_dma_ranges(&static_ranges).unwrap();
    for range in &dynamic {
        assert_eq!(
            range.cpu_base - range.dma_base,
            hyper_os::vm::DYNAMIC_ALIAS_OFFSET
        );
        for excluded in &static_ranges {
            assert!(
                range.dma_base + range.size <= excluded.dma_base
                    || range.dma_base >= excluded.dma_base + excluded.size
            );
        }
    }
    assert_eq!(
        dynamic.iter().map(|range| range.size).sum::<u64>() + 0x6000,
        hyper_os::vm::DYNAMIC_PHYSICAL_LIMIT
    );
    assert!(dynamic_dma_ranges(&[static_ranges[0], static_ranges[0]]).is_err());
}

#[test]
fn runtime_cannot_advance_the_backend_epoch_or_change_mapping_identity() {
    use hyper_vm_support::io_protocol::{Command, Request};
    let request = Request {
        binding: 2,
        epoch: 3,
        transaction: 99,
        command: Command::Hello,
    };
    assert!(authorize_request(request, 2, 3));
    assert!(!authorize_request(
        Request {
            epoch: u32::MAX,
            ..request
        },
        2,
        3
    ));
    assert!(!authorize_request(
        Request {
            binding: 4,
            ..request
        },
        2,
        3
    ));
    assert!(!authorize_request(
        Request {
            command: Command::Release,
            ..request
        },
        2,
        3
    ));
    assert!(!authorize_request(
        Request {
            command: Command::Prepare {
                alias: 0,
                guest_base: 0x4000_0000,
                length: 4096,
                mapping_token: 1
            },
            ..request
        },
        2,
        3
    ));
}
