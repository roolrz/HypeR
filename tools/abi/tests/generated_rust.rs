// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

#[allow(dead_code)]
#[path = "../../../abi/native/generated.rs"]
mod generated;

#[test]
fn generated_rust_layouts_are_compiler_checked() {
    assert_eq!(core::mem::size_of::<generated::HyperNativeHandleInfo>(), 16);
    assert_eq!(
        core::mem::align_of::<generated::HyperNativeObjectBasicInfo>(),
        8
    );
    assert_eq!(generated::HYPER_NATIVE_SYS_HANDLE_CLOSE, 1);
    assert_eq!(generated::HYPER_NATIVE_RIGHTS_MASK, 0xf_ffff);
    assert_eq!(generated::HYPER_NATIVE_SIGNAL_EVENT_SIGNALED, 1);
    assert_eq!(generated::HYPER_NATIVE_DEADLINE_INFINITE, u64::MAX);
}
