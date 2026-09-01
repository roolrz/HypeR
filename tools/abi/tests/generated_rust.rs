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
    assert_eq!(
        core::mem::size_of::<generated::HyperNativeChannelDisposition>(),
        24
    );
    assert_eq!(
        core::mem::align_of::<generated::HyperNativeChannelDisposition>(),
        8
    );
    assert_eq!(generated::HYPER_NATIVE_OBJECT_CHANNEL, 2);
    assert_eq!(generated::HYPER_NATIVE_SYS_CHANNEL_CREATE, 12);
    assert_eq!(generated::HYPER_NATIVE_SYS_CHANNEL_WRITE, 13);
    assert_eq!(generated::HYPER_NATIVE_SYS_CHANNEL_READ, 14);
    assert_eq!(generated::HYPER_NATIVE_SIGNAL_CHANNEL_READABLE, 1);
    assert_eq!(generated::HYPER_NATIVE_SIGNAL_CHANNEL_WRITABLE, 2);
    assert_eq!(generated::HYPER_NATIVE_SIGNAL_CHANNEL_PEER_CLOSED, 4);
    assert_eq!(
        generated::HYPER_NATIVE_CHANNEL_DISPOSITION_SAME_RIGHTS,
        u64::MAX
    );
    assert_eq!(generated::HYPER_NATIVE_CHANNEL_MAX_MESSAGE_BYTES, 64 * 1024);
    assert_eq!(generated::HYPER_NATIVE_CHANNEL_MAX_MESSAGE_HANDLES, 64);
    assert_eq!(generated::HYPER_NATIVE_STATUS_WOULD_BLOCK, -13);
    assert_eq!(generated::HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL, -14);
    assert_eq!(generated::HYPER_NATIVE_STATUS_PEER_CLOSED, -15);
    assert_eq!(
        generated::hyper_native_failure_result_mask(
            generated::HYPER_NATIVE_SYS_CHANNEL_READ,
            generated::HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL,
        ),
        0b11
    );
    assert_eq!(
        generated::hyper_native_failure_result_mask(
            generated::HYPER_NATIVE_SYS_CHANNEL_READ,
            generated::HYPER_NATIVE_STATUS_FAULT,
        ),
        0
    );
}
