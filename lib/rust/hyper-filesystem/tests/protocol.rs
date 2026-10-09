// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use hyper_filesystem::{
    driver::{Format, identify},
    protocol::*,
};

fn request() -> Request<'static> {
    Request {
        sequence: 17,
        operation: Operation::Rename,
        offset: 0,
        length: 0,
        path: "dir/file",
        target: "renamed",
        accessed: None,
        modified: None,
    }
}
#[test]
fn request_validates_exact_frame_paths_and_sequence() -> Result<(), &'static str> {
    let mut bytes = [0; REQUEST_BYTES];
    let length = request().encode(&mut bytes).ok_or("valid request")?;
    let decoded = Request::decode(&bytes[..length]).ok_or("valid frame")?;
    assert_eq!(decoded.path, "dir/file");
    assert!(Request::decode(&bytes[..length + 1]).is_none());
    for path in ["../escape", "/root", "a//b", "a/./b", "nul\0"] {
        assert!(Request { path, ..request() }.encode(&mut bytes).is_none());
    }
    assert!(
        Request {
            sequence: 0,
            ..request()
        }
        .encode(&mut bytes)
        .is_none()
    );
    Ok(())
}
#[test]
fn response_rejects_mismatched_transaction_and_malformed_metadata() {
    let mut bytes = [0; RESPONSE_BYTES];
    let mut response = Response {
        sequence: 17,
        operation: Operation::Stat,
        result: Ok(0),
        entry: Entry::empty(),
    };
    assert!(response.encode(&mut bytes).is_some());
    assert!(Response::decode(&bytes, 17, Operation::Stat).is_some());
    assert!(Response::decode(&bytes, 18, Operation::Stat).is_none());
    assert!(Response::decode(&bytes, 17, Operation::Read).is_none());
    assert!(Response::decode(&bytes[..RESPONSE_BYTES - 1], 17, Operation::Stat).is_none());
    bytes[56..64].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(Response::decode(&bytes, 17, Operation::Stat).is_none());
    response.entry.name_len = usize::MAX;
    assert!(response.encode(&mut bytes).is_none());
    response.entry = Entry::empty();
    response.entry.kind = 999;
    assert!(response.encode(&mut bytes).is_none());
    response.entry = Entry::empty();
    response.result = Ok(1);
    assert!(response.encode(&mut bytes).is_none());
}
#[test]
fn format_detection_uses_geometry_not_informational_label() {
    let mut boot = [0; 512];
    boot[11..13].copy_from_slice(&512u16.to_le_bytes());
    boot[13] = 1;
    boot[14..16].copy_from_slice(&32u16.to_le_bytes());
    boot[16] = 2;
    boot[32..36].copy_from_slice(&70000u32.to_le_bytes());
    boot[36..40].copy_from_slice(&600u32.to_le_bytes());
    boot[510..512].copy_from_slice(&[0x55, 0xaa]);
    assert_eq!(identify(&boot), Some(Format::Fat));
    boot[82..90].copy_from_slice(b"UNKNOWN ");
    assert_eq!(identify(&boot), Some(Format::Fat));
    boot[13] = 3;
    assert_eq!(identify(&boot), None);
    let mut partition = [0; 512];
    partition[510..512].copy_from_slice(&[0x55, 0xaa]);
    assert_eq!(identify(&partition), None);
}
