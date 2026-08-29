// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

#[allow(dead_code)]
#[path = "../../../abi/native/experimental.rs"]
mod generated;

#[test]
fn generated_rust_layouts_are_compiler_checked() {
    assert_eq!(
        core::mem::size_of::<generated::HyperExperimentalHandleInfo>(),
        16
    );
    assert_eq!(
        core::mem::align_of::<generated::HyperExperimentalObjectBasicInfo>(),
        8
    );
    assert_eq!(generated::HYPER_EXPERIMENTAL_SYS_HANDLE_CLOSE, 1);
    assert_eq!(generated::HYPER_EXPERIMENTAL_RIGHTS_MASK, 0x7_ffff);
}
