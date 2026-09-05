/* SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 *
 * Generated from schema/native.rs. Do not edit.
 */

#ifndef HYPER_NATIVE_H
#define HYPER_NATIVE_H

#include <stddef.h>
#include <stdint.h>

#if defined(__cplusplus)
#define HYPER_ABI_STATIC_ASSERT static_assert
#define HYPER_ABI_ALIGNOF alignof
#else
#define HYPER_ABI_STATIC_ASSERT _Static_assert
#define HYPER_ABI_ALIGNOF _Alignof
#endif

#define HYPER_NATIVE_ABI_REVISION UINT64_C(0)
#define HYPER_NATIVE_SYSCALL_ARGUMENT_REGISTERS UINT32_C(6)
#define HYPER_NATIVE_SYSCALL_RESULT_REGISTERS UINT32_C(2)

typedef uint64_t hyper_native_handle_t;
typedef int64_t hyper_native_status_t;

#define HYPER_NATIVE_FEATURE_CORE UINT64_C(1)

#define HYPER_NATIVE_STATUS_OK INT64_C(0)
#define HYPER_NATIVE_STATUS_INVALID_ARGUMENT (-INT64_C(1))
#define HYPER_NATIVE_STATUS_BAD_HANDLE (-INT64_C(2))
#define HYPER_NATIVE_STATUS_ACCESS_DENIED (-INT64_C(3))
#define HYPER_NATIVE_STATUS_NOT_SUPPORTED (-INT64_C(4))
#define HYPER_NATIVE_STATUS_NO_MEMORY (-INT64_C(5))
#define HYPER_NATIVE_STATUS_BAD_STATE (-INT64_C(6))
#define HYPER_NATIVE_STATUS_FAULT (-INT64_C(7))
#define HYPER_NATIVE_STATUS_RESOURCE_LIMIT (-INT64_C(8))
#define HYPER_NATIVE_STATUS_BUSY (-INT64_C(9))
#define HYPER_NATIVE_STATUS_INTERNAL (-INT64_C(10))
#define HYPER_NATIVE_STATUS_TIMED_OUT (-INT64_C(11))
#define HYPER_NATIVE_STATUS_CANCELLED (-INT64_C(12))
#define HYPER_NATIVE_STATUS_WOULD_BLOCK (-INT64_C(13))
#define HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL (-INT64_C(14))
#define HYPER_NATIVE_STATUS_PEER_CLOSED (-INT64_C(15))

#define HYPER_NATIVE_OBJECT_NONE UINT32_C(0)
#define HYPER_NATIVE_OBJECT_EVENT UINT32_C(1)
#define HYPER_NATIVE_OBJECT_CHANNEL UINT32_C(2)
#define HYPER_NATIVE_OBJECT_THREAD UINT32_C(3)
#define HYPER_NATIVE_OBJECT_PROCESS UINT32_C(4)
#define HYPER_NATIVE_OBJECT_TASK_GROUP UINT32_C(5)
#define HYPER_NATIVE_OBJECT_RESOURCE_DOMAIN UINT32_C(6)
#define HYPER_NATIVE_OBJECT_TASK_FACTORY UINT32_C(7)
#define HYPER_NATIVE_OBJECT_EXECUTABLE_AUTHORITY UINT32_C(8)
#define HYPER_NATIVE_OBJECT_VMO UINT32_C(9)
#define HYPER_NATIVE_OBJECT_VMAR UINT32_C(10)
#define HYPER_NATIVE_OBJECT_CONSOLE UINT32_C(11)

#define HYPER_NATIVE_RIGHT_DUPLICATE UINT64_C(1)
#define HYPER_NATIVE_RIGHT_TRANSFER UINT64_C(2)
#define HYPER_NATIVE_RIGHT_WAIT UINT64_C(4)
#define HYPER_NATIVE_RIGHT_INSPECT UINT64_C(8)
#define HYPER_NATIVE_RIGHT_READ UINT64_C(16)
#define HYPER_NATIVE_RIGHT_WRITE UINT64_C(32)
#define HYPER_NATIVE_RIGHT_MAP UINT64_C(64)
#define HYPER_NATIVE_RIGHT_EXECUTE UINT64_C(128)
#define HYPER_NATIVE_RIGHT_RESIZE UINT64_C(256)
#define HYPER_NATIVE_RIGHT_PIN UINT64_C(512)
#define HYPER_NATIVE_RIGHT_START UINT64_C(1024)
#define HYPER_NATIVE_RIGHT_REQUEST_STOP UINT64_C(2048)
#define HYPER_NATIVE_RIGHT_RUN_VCPU UINT64_C(4096)
#define HYPER_NATIVE_RIGHT_INJECT_INTERRUPT UINT64_C(8192)
#define HYPER_NATIVE_RIGHT_GRANT_MEMORY UINT64_C(16384)
#define HYPER_NATIVE_RIGHT_ASSIGN_DEVICE UINT64_C(32768)
#define HYPER_NATIVE_RIGHT_MAP_DMA UINT64_C(65536)
#define HYPER_NATIVE_RIGHT_ACK_INTERRUPT UINT64_C(131072)
#define HYPER_NATIVE_RIGHT_REVOKE UINT64_C(262144)
#define HYPER_NATIVE_RIGHT_SIGNAL UINT64_C(524288)
#define HYPER_NATIVE_RIGHT_CREATE_PROCESS UINT64_C(1048576)
#define HYPER_NATIVE_RIGHT_CREATE_THREAD UINT64_C(2097152)
#define HYPER_NATIVE_RIGHT_CREATE_TASK_GROUP UINT64_C(4194304)
#define HYPER_NATIVE_RIGHT_CREATE_RESOURCE_DOMAIN UINT64_C(8388608)
#define HYPER_NATIVE_RIGHT_SET_LIMITS UINT64_C(16777216)
#define HYPER_NATIVE_RIGHT_CREATE_EXECUTABLE UINT64_C(33554432)

#define HYPER_NATIVE_RIGHTS_MASK UINT64_C(67108863)

#define HYPER_NATIVE_SIGNAL_EVENT_SIGNALED UINT64_C(1)
#define HYPER_NATIVE_SIGNAL_CHANNEL_READABLE UINT64_C(1)
#define HYPER_NATIVE_SIGNAL_CHANNEL_WRITABLE UINT64_C(2)
#define HYPER_NATIVE_SIGNAL_CHANNEL_PEER_CLOSED UINT64_C(4)
#define HYPER_NATIVE_SIGNAL_THREAD_TERMINATED UINT64_C(1)
#define HYPER_NATIVE_SIGNAL_PROCESS_TERMINATED UINT64_C(1)
#define HYPER_NATIVE_SIGNAL_CONSOLE_READABLE UINT64_C(1)
#define HYPER_NATIVE_SIGNAL_CONSOLE_WRITABLE UINT64_C(2)

#define HYPER_NATIVE_ELF_OSABI UINT64_C(63)
#define HYPER_NATIVE_ELF_ABI_VERSION UINT64_C(0)
#define HYPER_NATIVE_AUXV_STARTUP_HANDLES UINT64_C(1213792257)
#define HYPER_NATIVE_AUXV_STARTUP_HANDLE_COUNT UINT64_C(1213792258)
#define HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_RESOURCE_DOMAIN UINT64_C(1)
#define HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_TASK_GROUP UINT64_C(2)
#define HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_TASK_FACTORY UINT64_C(3)
#define HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_EXECUTABLE_AUTHORITY UINT64_C(4)
#define HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_ROOT_VMAR UINT64_C(5)
#define HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_CONSOLE UINT64_C(6)
#define HYPER_NATIVE_DEADLINE_INFINITE UINT64_C(18446744073709551615)
#define HYPER_NATIVE_CHANNEL_DISPOSITION_SAME_RIGHTS UINT64_C(18446744073709551615)
#define HYPER_NATIVE_CHANNEL_MAX_MESSAGE_BYTES UINT64_C(65536)
#define HYPER_NATIVE_CHANNEL_MAX_MESSAGE_HANDLES UINT64_C(64)
#define HYPER_NATIVE_CHANNEL_MAX_QUEUED_MESSAGES UINT64_C(16)
#define HYPER_NATIVE_CHANNEL_MAX_QUEUED_BYTES UINT64_C(1048576)
#define HYPER_NATIVE_CHANNEL_MAX_QUEUED_HANDLES UINT64_C(1024)
#define HYPER_NATIVE_CONSOLE_MAX_TRANSFER_BYTES UINT64_C(4096)

#define HYPER_NATIVE_SYS_ABI_QUERY UINT64_C(0)
#define HYPER_NATIVE_SYS_HANDLE_CLOSE UINT64_C(1)
#define HYPER_NATIVE_SYS_HANDLE_DUPLICATE UINT64_C(2)
#define HYPER_NATIVE_SYS_HANDLE_REPLACE UINT64_C(3)
#define HYPER_NATIVE_SYS_HANDLE_GET_INFO UINT64_C(4)
#define HYPER_NATIVE_SYS_OBJECT_GET_BASIC_INFO UINT64_C(5)
#define HYPER_NATIVE_SYS_THREAD_YIELD UINT64_C(6)
#define HYPER_NATIVE_SYS_THREAD_EXIT UINT64_C(7)
#define HYPER_NATIVE_SYS_PROCESS_EXIT UINT64_C(8)
#define HYPER_NATIVE_SYS_EVENT_CREATE UINT64_C(9)
#define HYPER_NATIVE_SYS_EVENT_SIGNAL UINT64_C(10)
#define HYPER_NATIVE_SYS_OBJECT_WAIT_ONE UINT64_C(11)
#define HYPER_NATIVE_SYS_CHANNEL_CREATE UINT64_C(12)
#define HYPER_NATIVE_SYS_CHANNEL_WRITE UINT64_C(13)
#define HYPER_NATIVE_SYS_CHANNEL_READ UINT64_C(14)
#define HYPER_NATIVE_SYS_CONSOLE_READ UINT64_C(15)
#define HYPER_NATIVE_SYS_CONSOLE_WRITE UINT64_C(16)

static inline uint64_t hyper_native_failure_result_mask(
    uint64_t syscall_number, hyper_native_status_t status)
{
    if (syscall_number == HYPER_NATIVE_SYS_CHANNEL_READ &&
        status == HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL) {
        return UINT64_C(3);
    }
    if (syscall_number == HYPER_NATIVE_SYS_CONSOLE_READ &&
        status == HYPER_NATIVE_STATUS_WOULD_BLOCK) {
        return UINT64_C(1);
    }
    if (syscall_number == HYPER_NATIVE_SYS_CONSOLE_WRITE &&
        status == HYPER_NATIVE_STATUS_WOULD_BLOCK) {
        return UINT64_C(1);
    }
    return UINT64_C(0);
}

typedef struct hyper_native_handle_info_t {
    uint32_t object_kind;
    uint32_t flags;
    uint64_t rights;
} hyper_native_handle_info_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_handle_info_t) == 16, "handle_info size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_handle_info_t) == 8, "handle_info alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_handle_info_t, object_kind) == 0, "handle_info.object_kind offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_handle_info_t, flags) == 4, "handle_info.flags offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_handle_info_t, rights) == 8, "handle_info.rights offset");

typedef struct hyper_native_object_basic_info_t {
    uint64_t koid;
    uint32_t object_kind;
    uint32_t reserved;
} hyper_native_object_basic_info_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_object_basic_info_t) == 16, "object_basic_info size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_object_basic_info_t) == 8, "object_basic_info alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_basic_info_t, koid) == 0, "object_basic_info.koid offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_basic_info_t, object_kind) == 8, "object_basic_info.object_kind offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_basic_info_t, reserved) == 12, "object_basic_info.reserved offset");

typedef struct hyper_native_channel_disposition_t {
    uint64_t handle;
    uint64_t rights;
    uint32_t expected_kind;
    uint32_t reserved;
} hyper_native_channel_disposition_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_channel_disposition_t) == 24, "channel_disposition size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_channel_disposition_t) == 8, "channel_disposition alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_channel_disposition_t, handle) == 0, "channel_disposition.handle offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_channel_disposition_t, rights) == 8, "channel_disposition.rights offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_channel_disposition_t, expected_kind) == 16, "channel_disposition.expected_kind offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_channel_disposition_t, reserved) == 20, "channel_disposition.reserved offset");

typedef struct hyper_native_startup_handle_t {
    uint32_t purpose;
    uint32_t flags;
    uint64_t handle;
} hyper_native_startup_handle_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_startup_handle_t) == 16, "startup_handle size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_startup_handle_t) == 8, "startup_handle alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_startup_handle_t, purpose) == 0, "startup_handle.purpose offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_startup_handle_t, flags) == 4, "startup_handle.flags offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_startup_handle_t, handle) == 8, "startup_handle.handle offset");

#undef HYPER_ABI_ALIGNOF
#undef HYPER_ABI_STATIC_ASSERT

#endif /* HYPER_NATIVE_H */
