// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use hyper_abi as generated;

#[test]
fn generated_rust_layouts_are_compiler_checked() {
    assert_eq!(core::mem::size_of::<generated::HyperNativeHandleInfo>(), 16);
    assert_eq!(
        core::mem::align_of::<generated::HyperNativeObjectBasicInfo>(),
        8
    );
    assert_eq!(generated::HYPER_NATIVE_SYS_HANDLE_CLOSE, 1);
    assert_eq!(generated::HYPER_NATIVE_RIGHTS_MASK, 0x3ff_ffff);
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
    assert_eq!(generated::HYPER_NATIVE_OBJECT_THREAD, 3);
    assert_eq!(generated::HYPER_NATIVE_OBJECT_PROCESS, 4);
    assert_eq!(generated::HYPER_NATIVE_OBJECT_TASK_GROUP, 5);
    assert_eq!(generated::HYPER_NATIVE_OBJECT_RESOURCE_DOMAIN, 6);
    assert_eq!(generated::HYPER_NATIVE_OBJECT_TASK_FACTORY, 7);
    assert_eq!(generated::HYPER_NATIVE_OBJECT_EXECUTABLE_AUTHORITY, 8);
    assert_eq!(generated::HYPER_NATIVE_OBJECT_VMO, 9);
    assert_eq!(generated::HYPER_NATIVE_OBJECT_VMAR, 10);
    assert_eq!(generated::HYPER_NATIVE_OBJECT_CONSOLE, 11);
    assert_eq!(generated::HYPER_NATIVE_SYS_CHANNEL_CREATE, 12);
    assert_eq!(generated::HYPER_NATIVE_SYS_CHANNEL_WRITE, 13);
    assert_eq!(generated::HYPER_NATIVE_SYS_CHANNEL_READ, 14);
    assert_eq!(generated::HYPER_NATIVE_SIGNAL_CHANNEL_READABLE, 1);
    assert_eq!(generated::HYPER_NATIVE_SIGNAL_CHANNEL_WRITABLE, 2);
    assert_eq!(generated::HYPER_NATIVE_SIGNAL_CHANNEL_PEER_CLOSED, 4);
    assert_eq!(generated::HYPER_NATIVE_SIGNAL_THREAD_TERMINATED, 1);
    assert_eq!(generated::HYPER_NATIVE_SIGNAL_PROCESS_TERMINATED, 1);
    assert_eq!(generated::HYPER_NATIVE_SIGNAL_CONSOLE_READABLE, 1);
    assert_eq!(generated::HYPER_NATIVE_SIGNAL_CONSOLE_WRITABLE, 2);
    assert_eq!(generated::HYPER_NATIVE_ELF_OSABI, 63);
    assert_eq!(generated::HYPER_NATIVE_ELF_ABI_VERSION, 0);
    assert_eq!(generated::HYPER_NATIVE_AUXV_STARTUP_HANDLES, 0x4859_0001);
    assert_eq!(
        generated::HYPER_NATIVE_AUXV_STARTUP_HANDLE_COUNT,
        0x4859_0002
    );
    assert_eq!(
        generated::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_RESOURCE_DOMAIN,
        1
    );
    assert_eq!(generated::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_TASK_GROUP, 2);
    assert_eq!(
        generated::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_TASK_FACTORY,
        3
    );
    assert_eq!(
        generated::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_EXECUTABLE_AUTHORITY,
        4
    );
    assert_eq!(generated::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_ROOT_VMAR, 5);
    assert_eq!(generated::HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_CONSOLE, 6);
    assert_eq!(
        core::mem::size_of::<generated::HyperNativeStartupHandle>(),
        16
    );
    assert_eq!(
        core::mem::align_of::<generated::HyperNativeStartupHandle>(),
        8
    );
    assert_eq!(
        generated::HYPER_NATIVE_CHANNEL_DISPOSITION_SAME_RIGHTS,
        u64::MAX
    );
    assert_eq!(generated::HYPER_NATIVE_CHANNEL_MAX_MESSAGE_BYTES, 64 * 1024);
    assert_eq!(generated::HYPER_NATIVE_CHANNEL_MAX_MESSAGE_HANDLES, 64);
    assert_eq!(generated::HYPER_NATIVE_CONSOLE_MAX_TRANSFER_BYTES, 4 * 1024);
    assert_eq!(generated::HYPER_NATIVE_SYS_CONSOLE_READ, 15);
    assert_eq!(generated::HYPER_NATIVE_SYS_CONSOLE_WRITE, 16);
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
    assert_eq!(
        generated::hyper_native_failure_result_mask(
            generated::HYPER_NATIVE_SYS_CONSOLE_READ,
            generated::HYPER_NATIVE_STATUS_WOULD_BLOCK,
        ),
        0b1
    );
    assert_eq!(
        generated::hyper_native_failure_result_mask(
            generated::HYPER_NATIVE_SYS_CONSOLE_WRITE,
            generated::HYPER_NATIVE_STATUS_WOULD_BLOCK,
        ),
        0b1
    );
}
