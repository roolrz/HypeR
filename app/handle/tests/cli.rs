// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn process_selectors_accept_full_width_ids_and_exact_names()
-> Result<(), Box<dyn std::error::Error>> {
    for spelling in [
        "18446744073709551615",
        "0xffffffffffffffff",
        "0XFFFFFFFFFFFFFFFF",
    ] {
        let args = Handle::try_parse_from(["handle", spelling])?;
        assert_eq!(
            args.process_selector(),
            Some(&ProcessSelector::Koid(NonZeroU64::MAX))
        );
    }
    let args = Handle::try_parse_from(["handle", "-p", "io-runtime"])?;
    assert_eq!(
        args.process_selector(),
        Some(&ProcessSelector::Name("io-runtime".into()))
    );
    for value in [
        "0",
        "0x0",
        "-1",
        "+1",
        "0x+1",
        "0x",
        "0xgg",
        "18446744073709551616",
        "0x10000000000000000",
    ] {
        assert!(
            Handle::try_parse_from(["handle", "--object", value]).is_err(),
            "{value}"
        );
    }
    Ok(())
}

#[test]
fn valid_modes_and_conflicting_selections() {
    for args in [
        vec!["handle"],
        vec!["handle", "--objects"],
        vec!["handle", "shell", "--handle", "0x1000001"],
        vec!["handle", "--all", "--object", "0x100000001"],
        vec!["handle", "--object", "42", "-v"],
        vec!["handle", "--list-kinds"],
        vec!["handle", "--list-rights", "--no-headers"],
    ] {
        assert!(Handle::try_parse_from(&args).is_ok(), "{args:?}");
    }
    for args in [
        vec!["handle", "0"],
        vec!["handle", "1", "2"],
        vec!["handle", "--objects", "1"],
        vec!["handle", "shell", "-p", "shell"],
        vec!["handle", "--all", "shell"],
        vec!["handle", "--handle", "1"],
        vec!["handle", "--all", "--handle", "1"],
        vec!["handle", "--right", "read"],
        vec!["handle", "--objects", "--right", "read"],
        vec!["handle", "--list-kinds", "--kind", "process"],
        vec!["handle", "--list-kinds", "--list-rights"],
        vec!["handle", "--list-rights", "--object", "1"],
        vec!["handle", "--no-headers", "-v"],
    ] {
        assert!(Handle::try_parse_from(&args).is_err(), "{args:?}");
    }
}

#[test]
fn filter_vocabulary_is_validated() -> Result<(), Box<dyn std::error::Error>> {
    for kind in ObjectKind::KNOWN {
        let args = Handle::try_parse_from(["handle", "--kind", kind.name()])?;
        assert_eq!(args.kind, [kind.as_raw()]);
    }
    for right in Rights::known_names() {
        assert!(Handle::try_parse_from(["handle", "--all", "--right", right]).is_ok());
    }
    assert!(Handle::try_parse_from(["handle", "--kind", "physcial-device"]).is_err());
    assert!(Handle::try_parse_from(["handle", "--all", "--right", "wriet"]).is_err());
    let args = Handle::try_parse_from(["handle", "--kind", "0xffffffff"])?;
    assert_eq!(args.kind, [u32::MAX]);
    assert!(Handle::try_parse_from(["handle", "--kind", "0x100000000"]).is_err());
    Ok(())
}
