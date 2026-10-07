// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;
use clap::Parser;
use hyper_os::handle::{GuestMappingObject, Koid, TypedObject};
use hyper_os::inspect::ObjectReferenceCounts;

#[test]
fn detailed_objects_explain_lifetime_and_supported_rights() -> Result<(), Box<dyn std::error::Error>>
{
    let args = Handle::try_parse_from(["handle", "-v"])?;
    let item = ObjectObservation {
        koid: Koid::from_raw(0x200000123)?,
        object_kind: GuestMappingObject::KIND,
        handles: ObjectHandleState::Retired,
        supported_rights: Rights::INSPECT.union(Rights::REVOKE),
        references: ObjectReferenceCounts {
            strong: 7,
            kernel_service: 0,
            vm_device_binding: 1,
            scheduler: 0,
            operation: 0,
            user_authority: 0,
            publication: 2,
            diagnostic: 1,
            retirement: 3,
        },
    };
    let mut out = Vec::new();
    object(&args, &item, &mut out)?;
    let text = String::from_utf8(out)?;
    assert!(text.starts_with("0x0000000200000123 guest-mapping"));
    assert!(text.contains("retired"));
    assert!(text.contains("supported-rights: inspect|revoke"));
    assert!(text.contains("vm-device-binding=1"));
    assert!(text.contains("retirement=3"));
    assert!(!text.contains("unrecognized"));
    Ok(())
}

#[test]
fn handle_output_distinguishes_process_identity_and_escapes_names()
-> Result<(), Box<dyn std::error::Error>> {
    let args = Handle::try_parse_from(["handle", "--all", "-v"])?;
    let item = HandleObservation {
        process_koid: Koid::from_raw(0x100000321)?,
        handle: 0x1000001,
        object_koid: Koid::from_raw(0x200000123)?,
        object_kind: GuestMappingObject::KIND,
        rights: Rights::SET_ATTRIBUTES.union(Rights::LOCK_FILE),
        flags: 0x12,
    };
    let mut out = Vec::new();
    handle(&args, &item, "test\n\x1b[31m", &mut out)?;
    let text = String::from_utf8(out)?;
    assert!(text.starts_with("0x0000000100000321 0x0000000001000001 0x0000000200000123"));
    assert!(text.contains("set-attributes|lock-file"));
    assert!(text.contains("flags: 0x00000012"));
    assert!(!text.contains('\x1b'));
    assert!(text.contains("test\\n"));
    assert_eq!(format!("{}", RightsList(Rights::NONE)), "none");
    Ok(())
}
