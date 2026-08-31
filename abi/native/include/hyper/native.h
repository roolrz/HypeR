/* SPDX-FileCopyrightText: 2026 roolrz
 * SPDX-License-Identifier: Apache-2.0
 *
 * Generated from abi/native/schema.rs. Do not edit.
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

#define HYPER_NATIVE_OBJECT_NONE UINT32_C(0)
#define HYPER_NATIVE_OBJECT_EVENT UINT32_C(1)

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

#define HYPER_NATIVE_RIGHTS_MASK UINT64_C(1048575)

#define HYPER_NATIVE_SIGNAL_EVENT_SIGNALED UINT64_C(1)

#define HYPER_NATIVE_DEADLINE_INFINITE UINT64_C(18446744073709551615)

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

#undef HYPER_ABI_ALIGNOF
#undef HYPER_ABI_STATIC_ASSERT

#endif /* HYPER_NATIVE_H */
