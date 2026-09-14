// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::virtio_scsi::Queue;

fn reset() -> Request {
    Request {
        binding: 1,
        epoch: 7,
        transaction: 19,
        command: Command::Device(BackendOperation::Reset),
    }
}

fn reply(request: Request, status: u32) -> Result<Vec<u8>, Error> {
    let mut record = [0; MAX_RECORD];
    request.encode(&mut record)?;
    let length: u32 = if request.command == Command::Hello {
        64
    } else {
        48
    };
    record[8..12].copy_from_slice(&length.to_le_bytes());
    record[12..16].copy_from_slice(&1u32.to_le_bytes());
    record[40..44].copy_from_slice(&status.to_le_bytes());
    Ok(record[..length as usize].to_vec())
}

#[test]
fn stale_reply_cannot_authorize_quiescence() -> Result<(), Error> {
    let request = reset();
    let record = reply(request, 0)?;
    assert_eq!(Reply::decode(&record, request)?.status, Status::Success);
    for offset in [6, 16, 24, 32] {
        let mut stale = record.clone();
        stale[offset] ^= 1;
        assert_eq!(Reply::decode(&stale, request), Err(Error::MismatchedReply));
    }
    for length in 0..record.len() {
        assert_eq!(
            Reply::decode(&record[..length], request),
            Err(Error::InvalidRecord)
        );
    }
    assert_eq!(
        Reply::decode(&reply(request, 5)?, request)?.status,
        Status::QuiescenceFailed
    );
    Ok(())
}

#[test]
fn activation_wire_has_fixed_linux_layout() -> Result<(), Error> {
    let queues = [Queue {
        size: 128,
        descriptor: 0x4010_0000,
        available: 0x4010_1000,
        used: 0x4010_2000,
        ready: true,
    }; QUEUES];
    let request = Request {
        command: Command::Device(BackendOperation::Activate {
            features: VERSION_1,
            queues,
        }),
        ..reset()
    };
    let mut bytes = [0xff; MAX_RECORD];
    assert_eq!(request.encode(&mut bytes)?, 144);
    assert_eq!(u64_at(&bytes, 40)?, VERSION_1);
    for offset in [48, 80, 112] {
        assert_eq!(u32_at(&bytes, offset)?, 128);
        assert_eq!(u32_at(&bytes, offset + 4)?, 0);
        assert_eq!(u64_at(&bytes, offset + 8)?, 0x4010_0000);
        assert_eq!(u64_at(&bytes, offset + 16)?, 0x4010_1000);
        assert_eq!(u64_at(&bytes, offset + 24)?, 0x4010_2000);
    }
    assert_eq!(&bytes[12..16], &[0; 4]);
    Ok(())
}

#[test]
fn hello_rejects_incompatible_queue_contract() -> Result<(), Error> {
    let request = Request {
        command: Command::Hello,
        ..reset()
    };
    let mut bytes = reply(request, 0)?;
    bytes[48..56].copy_from_slice(&VERSION_1.to_le_bytes());
    bytes[56..60].copy_from_slice(&(QUEUES as u32).to_le_bytes());
    bytes[60..64].copy_from_slice(&QUEUE_MAX.to_le_bytes());
    assert_eq!(Reply::decode(&bytes, request)?.features, Some(VERSION_1));
    bytes[56] = 4;
    assert_eq!(
        Reply::decode(&bytes, request),
        Err(Error::UnsupportedBackend)
    );
    Ok(())
}
