// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

#[test]
fn prepared_builder_transfer_keeps_supervisor_retention_authority() -> hyper_os::Result<()> {
    use hyper_os::handle::{ProcessBuilderObject, Rights, TypedObject};
    let descriptors = hyper_component::installation::descriptors(
        &hyper_component::installation::Request::Prepared {
            path: "/custom-supervisor".into(),
        },
    )?;
    assert_eq!(descriptors[0].0, ProcessBuilderObject::KIND.as_raw());
    assert!(descriptors[0].1.contains(Rights::RETAIN_PROCESS));
    assert!(!descriptors[0].1.contains(Rights::DUPLICATE));
    Ok(())
}
