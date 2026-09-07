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

#define HYPER_NATIVE_FEATURE_CORE (UINT64_C(1) << 0)

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
#define HYPER_NATIVE_STATUS_NOT_FOUND (-INT64_C(16))

#define HYPER_NATIVE_OBJECT_NONE UINT32_C(0)
#define HYPER_NATIVE_OBJECT_EVENT UINT32_C(1)
#define HYPER_NATIVE_OBJECT_BYTE_CHANNEL UINT32_C(2)
#define HYPER_NATIVE_OBJECT_THREAD UINT32_C(3)
#define HYPER_NATIVE_OBJECT_PROCESS UINT32_C(4)
#define HYPER_NATIVE_OBJECT_TASK_GROUP UINT32_C(5)
#define HYPER_NATIVE_OBJECT_RESOURCE_DOMAIN UINT32_C(6)
#define HYPER_NATIVE_OBJECT_TASK_FACTORY UINT32_C(7)
#define HYPER_NATIVE_OBJECT_EXECUTABLE_AUTHORITY UINT32_C(8)
#define HYPER_NATIVE_OBJECT_VMO UINT32_C(9)
#define HYPER_NATIVE_OBJECT_VMAR UINT32_C(10)
#define HYPER_NATIVE_OBJECT_CONSOLE UINT32_C(11)
#define HYPER_NATIVE_OBJECT_DIRECTORY UINT32_C(12)
#define HYPER_NATIVE_OBJECT_FILE UINT32_C(13)
#define HYPER_NATIVE_OBJECT_CAPABILITY_CHANNEL UINT32_C(14)
#define HYPER_NATIVE_OBJECT_PROCESS_BUILDER UINT32_C(15)
#define HYPER_NATIVE_OBJECT_TASK_INSPECTOR UINT32_C(16)
#define HYPER_NATIVE_OBJECT_OBJECT_INSPECTOR UINT32_C(17)

#define HYPER_NATIVE_TRANSFER_CLASS_FORBIDDEN UINT32_C(0)
#define HYPER_NATIVE_TRANSFER_CLASS_GENERAL UINT32_C(1)
#define HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY UINT32_C(2)

static inline uint32_t hyper_native_object_transfer_class(uint32_t object_kind) {
    switch (object_kind) {
        case HYPER_NATIVE_OBJECT_NONE: return HYPER_NATIVE_TRANSFER_CLASS_FORBIDDEN;
        case HYPER_NATIVE_OBJECT_EVENT: return HYPER_NATIVE_TRANSFER_CLASS_GENERAL;
        case HYPER_NATIVE_OBJECT_BYTE_CHANNEL: return HYPER_NATIVE_TRANSFER_CLASS_GENERAL;
        case HYPER_NATIVE_OBJECT_THREAD: return HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY;
        case HYPER_NATIVE_OBJECT_PROCESS: return HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY;
        case HYPER_NATIVE_OBJECT_TASK_GROUP: return HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY;
        case HYPER_NATIVE_OBJECT_RESOURCE_DOMAIN: return HYPER_NATIVE_TRANSFER_CLASS_GENERAL;
        case HYPER_NATIVE_OBJECT_TASK_FACTORY: return HYPER_NATIVE_TRANSFER_CLASS_GENERAL;
        case HYPER_NATIVE_OBJECT_EXECUTABLE_AUTHORITY: return HYPER_NATIVE_TRANSFER_CLASS_GENERAL;
        case HYPER_NATIVE_OBJECT_VMO: return HYPER_NATIVE_TRANSFER_CLASS_GENERAL;
        case HYPER_NATIVE_OBJECT_VMAR: return HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY;
        case HYPER_NATIVE_OBJECT_CONSOLE: return HYPER_NATIVE_TRANSFER_CLASS_GENERAL;
        case HYPER_NATIVE_OBJECT_DIRECTORY: return HYPER_NATIVE_TRANSFER_CLASS_GENERAL;
        case HYPER_NATIVE_OBJECT_FILE: return HYPER_NATIVE_TRANSFER_CLASS_GENERAL;
        case HYPER_NATIVE_OBJECT_CAPABILITY_CHANNEL: return HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY;
        case HYPER_NATIVE_OBJECT_PROCESS_BUILDER: return HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY;
        case HYPER_NATIVE_OBJECT_TASK_INSPECTOR: return HYPER_NATIVE_TRANSFER_CLASS_GENERAL;
        case HYPER_NATIVE_OBJECT_OBJECT_INSPECTOR: return HYPER_NATIVE_TRANSFER_CLASS_GENERAL;
        default: return HYPER_NATIVE_TRANSFER_CLASS_FORBIDDEN;
    }
}

#define HYPER_NATIVE_RIGHT_DUPLICATE (UINT64_C(1) << 0)
#define HYPER_NATIVE_RIGHT_TRANSFER (UINT64_C(1) << 1)
#define HYPER_NATIVE_RIGHT_WAIT (UINT64_C(1) << 2)
#define HYPER_NATIVE_RIGHT_INSPECT (UINT64_C(1) << 3)
#define HYPER_NATIVE_RIGHT_READ (UINT64_C(1) << 4)
#define HYPER_NATIVE_RIGHT_WRITE (UINT64_C(1) << 5)
#define HYPER_NATIVE_RIGHT_MAP (UINT64_C(1) << 6)
#define HYPER_NATIVE_RIGHT_EXECUTE (UINT64_C(1) << 7)
#define HYPER_NATIVE_RIGHT_RESIZE (UINT64_C(1) << 8)
#define HYPER_NATIVE_RIGHT_PIN (UINT64_C(1) << 9)
#define HYPER_NATIVE_RIGHT_START (UINT64_C(1) << 10)
#define HYPER_NATIVE_RIGHT_REQUEST_STOP (UINT64_C(1) << 11)
#define HYPER_NATIVE_RIGHT_RUN_VCPU (UINT64_C(1) << 12)
#define HYPER_NATIVE_RIGHT_INJECT_INTERRUPT (UINT64_C(1) << 13)
#define HYPER_NATIVE_RIGHT_GRANT_MEMORY (UINT64_C(1) << 14)
#define HYPER_NATIVE_RIGHT_ASSIGN_DEVICE (UINT64_C(1) << 15)
#define HYPER_NATIVE_RIGHT_MAP_DMA (UINT64_C(1) << 16)
#define HYPER_NATIVE_RIGHT_ACK_INTERRUPT (UINT64_C(1) << 17)
#define HYPER_NATIVE_RIGHT_REVOKE (UINT64_C(1) << 18)
#define HYPER_NATIVE_RIGHT_SIGNAL (UINT64_C(1) << 19)
#define HYPER_NATIVE_RIGHT_CREATE_PROCESS (UINT64_C(1) << 20)
#define HYPER_NATIVE_RIGHT_CREATE_THREAD (UINT64_C(1) << 21)
#define HYPER_NATIVE_RIGHT_CREATE_TASK_GROUP (UINT64_C(1) << 22)
#define HYPER_NATIVE_RIGHT_CREATE_RESOURCE_DOMAIN (UINT64_C(1) << 23)
#define HYPER_NATIVE_RIGHT_SET_LIMITS (UINT64_C(1) << 24)
#define HYPER_NATIVE_RIGHT_CREATE_EXECUTABLE (UINT64_C(1) << 25)
#define HYPER_NATIVE_RIGHT_TASK_GROUP_ATTACH_PROCESS (UINT64_C(1) << 26)
#define HYPER_NATIVE_RIGHT_RESOURCE_DOMAIN_SPONSOR (UINT64_C(1) << 27)
#define HYPER_NATIVE_RIGHT_DERIVE (UINT64_C(1) << 28)

#define HYPER_NATIVE_RIGHTS_MASK UINT64_C(0x1fffffff)

#define HYPER_NATIVE_SIGNAL_EVENT_SIGNALED (UINT64_C(1) << 0)
#define HYPER_NATIVE_SIGNAL_BYTE_CHANNEL_READABLE (UINT64_C(1) << 0)
#define HYPER_NATIVE_SIGNAL_BYTE_CHANNEL_WRITABLE (UINT64_C(1) << 1)
#define HYPER_NATIVE_SIGNAL_BYTE_CHANNEL_PEER_CLOSED (UINT64_C(1) << 2)
#define HYPER_NATIVE_SIGNAL_CAPABILITY_CHANNEL_PEER_RECEIVING (UINT64_C(1) << 0)
#define HYPER_NATIVE_SIGNAL_CAPABILITY_CHANNEL_PEER_CLOSED (UINT64_C(1) << 1)
#define HYPER_NATIVE_SIGNAL_THREAD_TERMINATED (UINT64_C(1) << 0)
#define HYPER_NATIVE_SIGNAL_PROCESS_TERMINATED (UINT64_C(1) << 0)
#define HYPER_NATIVE_SIGNAL_CONSOLE_READABLE (UINT64_C(1) << 0)
#define HYPER_NATIVE_SIGNAL_CONSOLE_WRITABLE (UINT64_C(1) << 1)

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
#define HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_ROOT_DIRECTORY UINT64_C(7)
#define HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_TASK_INSPECTOR UINT64_C(8)
#define HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_OBJECT_INSPECTOR UINT64_C(9)
#define HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_DYNAMIC_LIBRARY_DIRECTORY UINT64_C(10)
#define HYPER_NATIVE_DIRECTORY_ENTRY_PAGE_CAPACITY UINT64_C(4)
#define HYPER_NATIVE_DIRECTORY_ENTRY_NAME_MAX_BYTES UINT64_C(255)
#define HYPER_NATIVE_DIRECTORY_ENTRY_KIND_FILE UINT64_C(1)
#define HYPER_NATIVE_DIRECTORY_ENTRY_KIND_DIRECTORY UINT64_C(2)
#define HYPER_NATIVE_DIRECTORY_ENTRY_KIND_SYMLINK UINT64_C(3)
#define HYPER_NATIVE_DIRECTORY_ENTRY_KIND_OTHER UINT64_C(4)
#define HYPER_NATIVE_STARTUP_MAX_HANDLES UINT64_C(256)
#define HYPER_NATIVE_DEADLINE_INFINITE UINT64_C(18446744073709551615)
#define HYPER_NATIVE_OBJECT_WAIT_MANY_MAX_ITEMS UINT64_C(64)
#define HYPER_NATIVE_CAPABILITY_DISPOSITION_SAME_RIGHTS UINT64_C(18446744073709551615)
#define HYPER_NATIVE_BYTE_CHANNEL_MAX_MESSAGE_BYTES UINT64_C(65536)
#define HYPER_NATIVE_BYTE_CHANNEL_MAX_QUEUED_MESSAGES UINT64_C(16)
#define HYPER_NATIVE_BYTE_CHANNEL_MAX_QUEUED_BYTES UINT64_C(1048576)
#define HYPER_NATIVE_CAPABILITY_CHANNEL_MAX_MESSAGE_BYTES UINT64_C(4096)
#define HYPER_NATIVE_CAPABILITY_CHANNEL_MAX_HANDLES UINT64_C(16)
#define HYPER_NATIVE_CAPABILITY_DISPOSITION_MOVE UINT64_C(0)
#define HYPER_NATIVE_CAPABILITY_DISPOSITION_DUPLICATE UINT64_C(1)
#define HYPER_NATIVE_CONSOLE_MAX_TRANSFER_BYTES UINT64_C(4096)
#define HYPER_NATIVE_DIRECTORY_MAX_PATH_BYTES UINT64_C(4096)
#define HYPER_NATIVE_FILE_MAX_READ_BYTES UINT64_C(65536)
#define HYPER_NATIVE_VMO_MAX_SIZE_BYTES UINT64_C(67108864)
#define HYPER_NATIVE_VMO_MAX_TRANSFER_BYTES UINT64_C(65536)
#define HYPER_NATIVE_VMAR_PERMISSION_READ UINT64_C(1)
#define HYPER_NATIVE_VMAR_PERMISSION_WRITE UINT64_C(2)
#define HYPER_NATIVE_VMAR_PERMISSION_EXECUTE UINT64_C(4)
#define HYPER_NATIVE_PROCESS_NAME_MAX_BYTES UINT64_C(64)
#define HYPER_NATIVE_PROCESS_ARGUMENT_MAX_BYTES UINT64_C(4096)
#define HYPER_NATIVE_PROCESS_ENVIRONMENT_MAX_BYTES UINT64_C(4096)
#define HYPER_NATIVE_PROCESS_MAX_ARGUMENTS UINT64_C(64)
#define HYPER_NATIVE_PROCESS_MAX_ENVIRONMENT UINT64_C(64)
#define HYPER_NATIVE_PROCESS_AFFINITY_MAX_WORDS UINT64_C(4)
#define HYPER_NATIVE_PROCESS_AFFINITY_MAX_CPUS UINT64_C(256)
#define HYPER_NATIVE_PROCESS_PHASE_PREPARED UINT64_C(0)
#define HYPER_NATIVE_PROCESS_PHASE_CREATED UINT64_C(1)
#define HYPER_NATIVE_PROCESS_PHASE_RUNNING UINT64_C(2)
#define HYPER_NATIVE_PROCESS_PHASE_STOPPING UINT64_C(3)
#define HYPER_NATIVE_PROCESS_PHASE_STOPPED UINT64_C(4)
#define HYPER_NATIVE_PROCESS_PHASE_RETIRING UINT64_C(5)
#define HYPER_NATIVE_PROCESS_PHASE_RETIRED UINT64_C(6)
#define HYPER_NATIVE_PROCESS_TERMINAL_NONE UINT64_C(0)
#define HYPER_NATIVE_PROCESS_TERMINAL_REQUESTED UINT64_C(1)
#define HYPER_NATIVE_PROCESS_TERMINAL_THREAD_EXITED UINT64_C(2)
#define HYPER_NATIVE_PROCESS_TERMINAL_PROCESS_EXITED UINT64_C(3)
#define HYPER_NATIVE_PROCESS_TERMINAL_LAST_THREAD_EXITED UINT64_C(4)
#define HYPER_NATIVE_PROCESS_TERMINAL_FAULT UINT64_C(5)
#define HYPER_NATIVE_PROCESS_TERMINAL_TASK_GROUP_STOP UINT64_C(6)
#define HYPER_NATIVE_TASK_INSPECTOR_PROCESS_PAGE_CAPACITY UINT64_C(8)
#define HYPER_NATIVE_TASK_INSPECTOR_THREAD_PAGE_CAPACITY UINT64_C(8)
#define HYPER_NATIVE_OBJECT_INSPECTOR_OBJECT_PAGE_CAPACITY UINT64_C(8)
#define HYPER_NATIVE_OBJECT_INSPECTOR_HANDLE_PAGE_CAPACITY UINT64_C(8)
#define HYPER_NATIVE_THREAD_ROLE_BOOTSTRAP UINT64_C(1)
#define HYPER_NATIVE_THREAD_ROLE_IDLE UINT64_C(2)
#define HYPER_NATIVE_THREAD_ROLE_KERNEL UINT64_C(3)
#define HYPER_NATIVE_THREAD_ROLE_USER UINT64_C(4)
#define HYPER_NATIVE_THREAD_ROLE_VCPU UINT64_C(5)
#define HYPER_NATIVE_THREAD_REGISTRY_RESIDENT UINT64_C(1)
#define HYPER_NATIVE_THREAD_REGISTRY_RETIRING UINT64_C(2)
#define HYPER_NATIVE_OBJECT_HANDLE_STATE_UNPUBLISHED UINT64_C(1)
#define HYPER_NATIVE_OBJECT_HANDLE_STATE_ACTIVE UINT64_C(2)
#define HYPER_NATIVE_OBJECT_HANDLE_STATE_RETIRED UINT64_C(3)

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
#define HYPER_NATIVE_SYS_BYTE_CHANNEL_CREATE UINT64_C(12)
#define HYPER_NATIVE_SYS_BYTE_CHANNEL_WRITE UINT64_C(13)
#define HYPER_NATIVE_SYS_BYTE_CHANNEL_READ UINT64_C(14)
#define HYPER_NATIVE_SYS_CONSOLE_READ UINT64_C(15)
#define HYPER_NATIVE_SYS_CONSOLE_WRITE UINT64_C(16)
#define HYPER_NATIVE_SYS_DIRECTORY_OPEN_FILE UINT64_C(17)
#define HYPER_NATIVE_SYS_FILE_READ_AT UINT64_C(18)
#define HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_CREATE UINT64_C(19)
#define HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_TRY_SEND UINT64_C(20)
#define HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_RECEIVE UINT64_C(21)
#define HYPER_NATIVE_SYS_PROCESS_BUILDER_CREATE UINT64_C(22)
#define HYPER_NATIVE_SYS_PROCESS_BUILDER_SET_NAME UINT64_C(23)
#define HYPER_NATIVE_SYS_PROCESS_BUILDER_ADD_ARGUMENT UINT64_C(24)
#define HYPER_NATIVE_SYS_PROCESS_BUILDER_ADD_ENVIRONMENT UINT64_C(25)
#define HYPER_NATIVE_SYS_PROCESS_BUILDER_SET_AFFINITY UINT64_C(26)
#define HYPER_NATIVE_SYS_PROCESS_BUILDER_ADD_HANDLE UINT64_C(27)
#define HYPER_NATIVE_SYS_PROCESS_BUILDER_SEAL UINT64_C(28)
#define HYPER_NATIVE_SYS_PROCESS_BUILDER_START UINT64_C(29)
#define HYPER_NATIVE_SYS_PROCESS_BUILDER_ABORT UINT64_C(30)
#define HYPER_NATIVE_SYS_PROCESS_REQUEST_STOP UINT64_C(31)
#define HYPER_NATIVE_SYS_OBJECT_WAIT_MANY UINT64_C(32)
#define HYPER_NATIVE_SYS_PROCESS_GET_INFO UINT64_C(33)
#define HYPER_NATIVE_SYS_TASK_INSPECTOR_SCAN_PROCESSES UINT64_C(34)
#define HYPER_NATIVE_SYS_TASK_INSPECTOR_SCAN_THREADS UINT64_C(35)
#define HYPER_NATIVE_SYS_TASK_INSPECTOR_DERIVE_PROCESS UINT64_C(36)
#define HYPER_NATIVE_SYS_OBJECT_INSPECTOR_SCAN_OBJECTS UINT64_C(37)
#define HYPER_NATIVE_SYS_OBJECT_INSPECTOR_SCAN_HANDLES UINT64_C(38)
#define HYPER_NATIVE_SYS_OBJECT_INSPECTOR_DERIVE_PROCESS UINT64_C(39)
#define HYPER_NATIVE_SYS_TASK_INSPECTOR_DERIVE_TASK_GROUP UINT64_C(40)
#define HYPER_NATIVE_SYS_OBJECT_INSPECTOR_DERIVE_TASK_GROUP UINT64_C(41)
#define HYPER_NATIVE_SYS_TASK_INSPECTOR_DERIVE_RESOURCE_DOMAIN UINT64_C(42)
#define HYPER_NATIVE_SYS_OBJECT_INSPECTOR_DERIVE_RESOURCE_DOMAIN UINT64_C(43)
#define HYPER_NATIVE_SYS_DIRECTORY_OPEN_DIRECTORY UINT64_C(44)
#define HYPER_NATIVE_SYS_VMO_CREATE UINT64_C(45)
#define HYPER_NATIVE_SYS_FILE_CREATE_EXECUTABLE_VMO UINT64_C(46)
#define HYPER_NATIVE_SYS_VMO_READ UINT64_C(47)
#define HYPER_NATIVE_SYS_VMO_WRITE UINT64_C(48)
#define HYPER_NATIVE_SYS_VMAR_ALLOCATE UINT64_C(49)
#define HYPER_NATIVE_SYS_VMAR_MAP UINT64_C(50)
#define HYPER_NATIVE_SYS_VMAR_PROTECT UINT64_C(51)
#define HYPER_NATIVE_SYS_VMAR_UNMAP UINT64_C(52)
#define HYPER_NATIVE_SYS_VMAR_DESTROY UINT64_C(53)
#define HYPER_NATIVE_SYS_DIRECTORY_READ UINT64_C(54)

static inline uint64_t hyper_native_failure_result_mask(
    uint64_t syscall_number, hyper_native_status_t status)
{
    if (syscall_number == HYPER_NATIVE_SYS_BYTE_CHANNEL_READ &&
        status == HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL) {
        return UINT64_C(1);
    }
    if (syscall_number == HYPER_NATIVE_SYS_CONSOLE_READ &&
        status == HYPER_NATIVE_STATUS_WOULD_BLOCK) {
        return UINT64_C(1);
    }
    if (syscall_number == HYPER_NATIVE_SYS_CONSOLE_WRITE &&
        status == HYPER_NATIVE_STATUS_WOULD_BLOCK) {
        return UINT64_C(1);
    }
    if (syscall_number == HYPER_NATIVE_SYS_CAPABILITY_CHANNEL_RECEIVE &&
        status == HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL) {
        return UINT64_C(3);
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

typedef struct hyper_native_object_wait_item_t {
    uint64_t handle;
    uint64_t signals;
} hyper_native_object_wait_item_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_object_wait_item_t) == 16, "object_wait_item size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_object_wait_item_t) == 8, "object_wait_item alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_wait_item_t, handle) == 0, "object_wait_item.handle offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_wait_item_t, signals) == 8, "object_wait_item.signals offset");

typedef struct hyper_native_process_info_t {
    uint32_t phase;
    uint32_t terminal_reason;
    uint64_t detail0;
    uint64_t detail1;
    uint64_t reserved;
} hyper_native_process_info_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_process_info_t) == 32, "process_info size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_process_info_t) == 8, "process_info alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_process_info_t, phase) == 0, "process_info.phase offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_process_info_t, terminal_reason) == 4, "process_info.terminal_reason offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_process_info_t, detail0) == 8, "process_info.detail0 offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_process_info_t, detail1) == 16, "process_info.detail1 offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_process_info_t, reserved) == 24, "process_info.reserved offset");

typedef struct hyper_native_capability_disposition_t {
    uint64_t handle;
    uint64_t rights;
    uint32_t expected_kind;
    uint32_t operation;
} hyper_native_capability_disposition_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_capability_disposition_t) == 24, "capability_disposition size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_capability_disposition_t) == 8, "capability_disposition alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_capability_disposition_t, handle) == 0, "capability_disposition.handle offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_capability_disposition_t, rights) == 8, "capability_disposition.rights offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_capability_disposition_t, expected_kind) == 16, "capability_disposition.expected_kind offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_capability_disposition_t, operation) == 20, "capability_disposition.operation offset");

typedef struct hyper_native_capability_receive_slot_t {
    uint64_t handle;
    uint64_t rights;
    uint32_t expected_kind;
    uint32_t flags;
} hyper_native_capability_receive_slot_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_capability_receive_slot_t) == 24, "capability_receive_slot size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_capability_receive_slot_t) == 8, "capability_receive_slot alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_capability_receive_slot_t, handle) == 0, "capability_receive_slot.handle offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_capability_receive_slot_t, rights) == 8, "capability_receive_slot.rights offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_capability_receive_slot_t, expected_kind) == 16, "capability_receive_slot.expected_kind offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_capability_receive_slot_t, flags) == 20, "capability_receive_slot.flags offset");

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

typedef struct hyper_native_task_process_t {
    uint64_t koid;
    uint32_t phase;
    uint32_t terminal_reason;
    uint32_t pending_threads;
    uint32_t active_threads;
    uint32_t name_length;
    uint32_t reserved;
    uint8_t name[64];
} hyper_native_task_process_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_task_process_t) == 96, "task_process size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_task_process_t) == 8, "task_process alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_process_t, koid) == 0, "task_process.koid offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_process_t, phase) == 8, "task_process.phase offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_process_t, terminal_reason) == 12, "task_process.terminal_reason offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_process_t, pending_threads) == 16, "task_process.pending_threads offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_process_t, active_threads) == 20, "task_process.active_threads offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_process_t, name_length) == 24, "task_process.name_length offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_process_t, reserved) == 28, "task_process.reserved offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_process_t, name) == 32, "task_process.name offset");

typedef struct hyper_native_task_thread_t {
    uint64_t koid;
    uint64_t process_koid;
    uint32_t role;
    uint32_t registry_phase;
    uint32_t name_length;
    uint32_t reserved;
    uint8_t name[64];
} hyper_native_task_thread_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_task_thread_t) == 96, "task_thread size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_task_thread_t) == 8, "task_thread alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_thread_t, koid) == 0, "task_thread.koid offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_thread_t, process_koid) == 8, "task_thread.process_koid offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_thread_t, role) == 16, "task_thread.role offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_thread_t, registry_phase) == 20, "task_thread.registry_phase offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_thread_t, name_length) == 24, "task_thread.name_length offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_thread_t, reserved) == 28, "task_thread.reserved offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_thread_t, name) == 32, "task_thread.name offset");

typedef struct hyper_native_object_inspection_t {
    uint64_t koid;
    uint32_t object_kind;
    uint32_t handle_state;
    uint64_t active_handles;
    uint64_t supported_rights;
    uint64_t strong_references;
    uint64_t kernel_service_references;
    uint64_t scheduler_references;
    uint64_t operation_references;
    uint64_t user_authority_references;
    uint64_t publication_references;
    uint64_t diagnostic_references;
    uint64_t retirement_references;
} hyper_native_object_inspection_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_object_inspection_t) == 96, "object_inspection size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_object_inspection_t) == 8, "object_inspection alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_inspection_t, koid) == 0, "object_inspection.koid offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_inspection_t, object_kind) == 8, "object_inspection.object_kind offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_inspection_t, handle_state) == 12, "object_inspection.handle_state offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_inspection_t, active_handles) == 16, "object_inspection.active_handles offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_inspection_t, supported_rights) == 24, "object_inspection.supported_rights offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_inspection_t, strong_references) == 32, "object_inspection.strong_references offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_inspection_t, kernel_service_references) == 40, "object_inspection.kernel_service_references offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_inspection_t, scheduler_references) == 48, "object_inspection.scheduler_references offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_inspection_t, operation_references) == 56, "object_inspection.operation_references offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_inspection_t, user_authority_references) == 64, "object_inspection.user_authority_references offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_inspection_t, publication_references) == 72, "object_inspection.publication_references offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_inspection_t, diagnostic_references) == 80, "object_inspection.diagnostic_references offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_inspection_t, retirement_references) == 88, "object_inspection.retirement_references offset");

typedef struct hyper_native_handle_inspection_t {
    uint64_t process_koid;
    uint64_t handle;
    uint64_t object_koid;
    uint64_t rights;
    uint32_t object_kind;
    uint32_t flags;
} hyper_native_handle_inspection_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_handle_inspection_t) == 40, "handle_inspection size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_handle_inspection_t) == 8, "handle_inspection alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_handle_inspection_t, process_koid) == 0, "handle_inspection.process_koid offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_handle_inspection_t, handle) == 8, "handle_inspection.handle offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_handle_inspection_t, object_koid) == 16, "handle_inspection.object_koid offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_handle_inspection_t, rights) == 24, "handle_inspection.rights offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_handle_inspection_t, object_kind) == 32, "handle_inspection.object_kind offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_handle_inspection_t, flags) == 36, "handle_inspection.flags offset");

typedef struct hyper_native_directory_entry_t {
    uint64_t size;
    uint32_t mode;
    uint32_t kind;
    uint32_t name_length;
    uint32_t reserved;
    uint8_t name[256];
} hyper_native_directory_entry_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_directory_entry_t) == 280, "directory_entry size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_directory_entry_t) == 8, "directory_entry alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_directory_entry_t, size) == 0, "directory_entry.size offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_directory_entry_t, mode) == 8, "directory_entry.mode offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_directory_entry_t, kind) == 12, "directory_entry.kind offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_directory_entry_t, name_length) == 16, "directory_entry.name_length offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_directory_entry_t, reserved) == 20, "directory_entry.reserved offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_directory_entry_t, name) == 24, "directory_entry.name offset");

#undef HYPER_ABI_ALIGNOF
#undef HYPER_ABI_STATIC_ASSERT

#endif /* HYPER_NATIVE_H */
