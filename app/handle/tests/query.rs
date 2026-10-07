// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;
use clap::Parser;
use hyper_os::handle::{FileObject, ProcessObject, Rights, TypedObject};

#[test]
fn exact_name_resolution_rejects_missing_and_ambiguous_names()
-> Result<(), Box<dyn std::error::Error>> {
    let first = Koid::from_raw(0x100000001)?;
    let second = Koid::from_raw(0x200000001)?;
    assert_eq!(
        named_process(
            "shell",
            [(first, "shell"), (second, "shell-child")].into_iter()
        )?,
        first
    );
    assert!(named_process("shell", [(first, "shell-child")].into_iter()).is_err());
    let duplicate = named_process("shell", [(first, "shell"), (second, "shell")].into_iter());
    let Err(error) = duplicate else {
        return Err("ambiguous name selected a process".into());
    };
    assert!(error.to_string().contains("0x0000000100000001"));
    assert!(error.to_string().contains("0x0000000200000001"));
    Ok(())
}

#[test]
fn handle_filters_preserve_generation_and_require_all_rights()
-> Result<(), Box<dyn std::error::Error>> {
    let item = HandleObservation {
        process_koid: Koid::from_raw(0x100000001)?,
        handle: 0x1000001,
        object_koid: Koid::from_raw(0x100000002)?,
        rights: Rights::READ.union(Rights::INSPECT),
        object_kind: FileObject::KIND,
        flags: 0,
    };
    let args = Handle::try_parse_from([
        "handle",
        "shell",
        "--object",
        "0x100000002",
        "--handle",
        "0x1000001",
        "--kind",
        "file",
        "--kind",
        "process",
        "--right",
        "read",
        "--right",
        "inspect",
    ])?;
    assert!(matches_handle(&args, &item));
    assert!(matches_handle(
        &args,
        &HandleObservation {
            object_kind: ProcessObject::KIND,
            ..item
        }
    ));
    assert!(!matches_handle(
        &args,
        &HandleObservation {
            rights: Rights::READ,
            ..item
        }
    ));
    assert!(!matches_handle(
        &args,
        &HandleObservation {
            object_koid: Koid::from_raw(0x200000002)?,
            ..item
        }
    ));
    assert!(!matches_handle(
        &args,
        &HandleObservation {
            handle: 0x2000001,
            ..item
        }
    ));
    let write = Handle::try_parse_from(["handle", "--all", "--right", "write"])?;
    assert!(!matches_handle(&write, &item));
    Ok(())
}

#[test]
fn only_not_found_can_be_skipped_during_cross_process_scan()
-> Result<(), Box<dyn std::error::Error>> {
    assert!(process_unavailable(&io::Error::other(Error::Status(
        Status::NOT_FOUND
    ))));
    assert!(!process_unavailable(&io::Error::other(
        Error::InvalidResponse
    )));
    let pipe = io::Error::from(io::ErrorKind::BrokenPipe);
    assert!(!process_unavailable(&pipe));
    assert_eq!(
        context(Koid::from_raw(1)?, pipe).kind(),
        io::ErrorKind::BrokenPipe
    );
    let args = Handle::try_parse_from(["handle", "--no-headers"])?;
    let mut out = Vec::new();
    empty(&args, 0, "objects", &mut out)?;
    assert!(out.is_empty());
    Ok(())
}
