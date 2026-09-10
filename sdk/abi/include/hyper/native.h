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
#define HYPER_NATIVE_STATUS_ALREADY_EXISTS (-INT64_C(17))
#define HYPER_NATIVE_STATUS_NOT_EMPTY (-INT64_C(18))
#define HYPER_NATIVE_STATUS_NOT_DIRECTORY (-INT64_C(19))
#define HYPER_NATIVE_STATUS_IS_DIRECTORY (-INT64_C(20))
#define HYPER_NATIVE_STATUS_SYMLINK_LOOP (-INT64_C(21))
#define HYPER_NATIVE_STATUS_CROSS_DEVICE (-INT64_C(22))

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
#define HYPER_NATIVE_OBJECT_MEMORY_INSPECTOR UINT32_C(18)
#define HYPER_NATIVE_OBJECT_CPU_INSPECTOR UINT32_C(19)
#define HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE_CREATION_AUTHORITY UINT32_C(20)
#define HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE_CREATION_LEASE UINT32_C(21)
#define HYPER_NATIVE_OBJECT_PENDING_VIRTUAL_MACHINE UINT32_C(22)
#define HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE UINT32_C(23)
#define HYPER_NATIVE_OBJECT_VIRTUAL_CPU UINT32_C(24)
#define HYPER_NATIVE_OBJECT_VIRTUAL_SERIAL UINT32_C(25)
#define HYPER_NATIVE_OBJECT_WAIT_SET UINT32_C(26)

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
        case HYPER_NATIVE_OBJECT_MEMORY_INSPECTOR: return HYPER_NATIVE_TRANSFER_CLASS_GENERAL;
        case HYPER_NATIVE_OBJECT_CPU_INSPECTOR: return HYPER_NATIVE_TRANSFER_CLASS_GENERAL;
        case HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE_CREATION_AUTHORITY: return HYPER_NATIVE_TRANSFER_CLASS_GENERAL;
        case HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE_CREATION_LEASE: return HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY;
        case HYPER_NATIVE_OBJECT_PENDING_VIRTUAL_MACHINE: return HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY;
        case HYPER_NATIVE_OBJECT_VIRTUAL_MACHINE: return HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY;
        case HYPER_NATIVE_OBJECT_VIRTUAL_CPU: return HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY;
        case HYPER_NATIVE_OBJECT_VIRTUAL_SERIAL: return HYPER_NATIVE_TRANSFER_CLASS_FORBIDDEN;
        case HYPER_NATIVE_OBJECT_WAIT_SET: return HYPER_NATIVE_TRANSFER_CLASS_FORBIDDEN;
        default: return HYPER_NATIVE_TRANSFER_CLASS_FORBIDDEN;
    }
}

#define HYPER_NATIVE_RIGHT_BIND_WAIT (UINT64_C(1) << 30)
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
#define HYPER_NATIVE_RIGHT_CREATE_VIRTUAL_MACHINE (UINT64_C(1) << 29)
#define HYPER_NATIVE_RIGHT_SET_ATTRIBUTES (UINT64_C(1) << 31)
#define HYPER_NATIVE_RIGHT_LOCK_FILE (UINT64_C(1) << 32)

#define HYPER_NATIVE_RIGHTS_MASK UINT64_C(0x1ffffffff)

#define HYPER_NATIVE_SIGNAL_VIRTUAL_SERIAL_READABLE (UINT64_C(1) << 0)
#define HYPER_NATIVE_SIGNAL_VIRTUAL_SERIAL_WRITABLE (UINT64_C(1) << 1)
#define HYPER_NATIVE_SIGNAL_VIRTUAL_SERIAL_PEER_CLOSED (UINT64_C(1) << 2)
#define HYPER_NATIVE_SIGNAL_WAIT_SET_READABLE (UINT64_C(1) << 0)
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
#define HYPER_NATIVE_SIGNAL_VIRTUAL_MACHINE_TERMINATED (UINT64_C(1) << 0)
#define HYPER_NATIVE_SIGNAL_VIRTUAL_CPU_TERMINATED (UINT64_C(1) << 0)

#define HYPER_NATIVE_PAGE_SIZE UINT64_C(4096)
#define HYPER_NATIVE_EXTENSIBLE_RECORD_MAX_BYTES UINT64_C(4096)
#define HYPER_NATIVE_VIRTUAL_SERIAL_MAX_TRANSFER_BYTES UINT64_C(4096)
#define HYPER_NATIVE_VIRTUAL_SERIAL_OUTPUT_CAPACITY UINT64_C(65536)
#define HYPER_NATIVE_VIRTUAL_SERIAL_OUTPUT_HEADER_BYTES UINT64_C(4096)
#define HYPER_NATIVE_VIRTUAL_SERIAL_OUTPUT_BYTES UINT64_C(69632)
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
#define HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_MEMORY_INSPECTOR UINT64_C(11)
#define HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_CPU_INSPECTOR UINT64_C(12)
#define HYPER_NATIVE_STARTUP_HANDLE_PURPOSE_VIRTUAL_MACHINE_CREATION_AUTHORITY UINT64_C(13)
#define HYPER_NATIVE_VIRTUAL_MACHINE_ARCHITECTURE_AARCH64 UINT64_C(1)
#define HYPER_NATIVE_VIRTUAL_MACHINE_ARCHITECTURE_RISCV64 UINT64_C(2)
#define HYPER_NATIVE_VIRTUAL_MACHINE_ARCHITECTURE_X86_64 UINT64_C(3)
#define HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE UINT64_C(1)
#define HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_GUEST_RAM_BASE UINT64_C(1073741824)
#define HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_DTB_OFFSET UINT64_C(65536)
#define HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_GIC_DISTRIBUTOR_BASE UINT64_C(134217728)
#define HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_GIC_DISTRIBUTOR_SIZE UINT64_C(65536)
#define HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_GIC_REDISTRIBUTOR_BASE UINT64_C(134873088)
#define HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_GIC_REDISTRIBUTOR_SIZE UINT64_C(131072)
#define HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_UART_BASE UINT64_C(150994944)
#define HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_UART_SIZE UINT64_C(4096)
#define HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_UART_INTERRUPT UINT64_C(33)
#define HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_TIMER_INTERRUPT UINT64_C(27)
#define HYPER_NATIVE_VIRTUAL_MACHINE_PHASE_INSTALLED UINT64_C(1)
#define HYPER_NATIVE_VIRTUAL_MACHINE_PHASE_RUNNING UINT64_C(2)
#define HYPER_NATIVE_VIRTUAL_MACHINE_PHASE_STOPPING UINT64_C(3)
#define HYPER_NATIVE_VIRTUAL_MACHINE_PHASE_STOPPED UINT64_C(4)
#define HYPER_NATIVE_VIRTUAL_CPU_PHASE_DORMANT UINT64_C(1)
#define HYPER_NATIVE_VIRTUAL_CPU_PHASE_STARTED UINT64_C(2)
#define HYPER_NATIVE_VIRTUAL_CPU_PHASE_STOPPED UINT64_C(3)
#define HYPER_NATIVE_VIRTUAL_CPU_TERMINAL_NONE UINT64_C(0)
#define HYPER_NATIVE_VIRTUAL_CPU_TERMINAL_MEMORY_FAULT UINT64_C(1)
#define HYPER_NATIVE_VIRTUAL_CPU_TERMINAL_MMIO UINT64_C(2)
#define HYPER_NATIVE_VIRTUAL_CPU_TERMINAL_SYNCHRONOUS UINT64_C(3)
#define HYPER_NATIVE_VIRTUAL_CPU_TERMINAL_ADMINISTRATIVE UINT64_C(4)
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
#define HYPER_NATIVE_VMO_MAX_SIZE_BYTES UINT64_C(4294967296)
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
#define HYPER_NATIVE_SYS_MEMORY_INSPECTOR_READ UINT64_C(55)
#define HYPER_NATIVE_SYS_CPU_INSPECTOR_READ UINT64_C(56)
#define HYPER_NATIVE_SYS_FILE_GET_INFO UINT64_C(57)
#define HYPER_NATIVE_SYS_DIRECTORY_GET_INFO UINT64_C(58)
#define HYPER_NATIVE_SYS_VIRTUAL_MACHINE_CREATION_LEASE_CREATE UINT64_C(59)
#define HYPER_NATIVE_SYS_VIRTUAL_MACHINE_CREATE UINT64_C(60)
#define HYPER_NATIVE_SYS_PENDING_VIRTUAL_MACHINE_SET_MEMORY UINT64_C(61)
#define HYPER_NATIVE_SYS_PENDING_VIRTUAL_MACHINE_SET_BOOTSTRAP UINT64_C(62)
#define HYPER_NATIVE_SYS_PENDING_VIRTUAL_MACHINE_SEAL UINT64_C(63)
#define HYPER_NATIVE_SYS_PENDING_VIRTUAL_MACHINE_INSTALL UINT64_C(64)
#define HYPER_NATIVE_SYS_PENDING_VIRTUAL_MACHINE_ABORT UINT64_C(65)
#define HYPER_NATIVE_SYS_VIRTUAL_MACHINE_REQUEST_STOP UINT64_C(66)
#define HYPER_NATIVE_SYS_VIRTUAL_MACHINE_GET_INFO UINT64_C(67)
#define HYPER_NATIVE_SYS_VIRTUAL_CPU_GET_INFO UINT64_C(68)
#define HYPER_NATIVE_SYS_RESOURCE_DOMAIN_CREATE UINT64_C(69)
#define HYPER_NATIVE_SYS_TASK_GROUP_CREATE UINT64_C(70)
#define HYPER_NATIVE_SYS_VIRTUAL_CPU_START UINT64_C(71)
#define HYPER_NATIVE_SYS_PENDING_VIRTUAL_MACHINE_SET_VIRTUAL_SERIAL UINT64_C(72)
#define HYPER_NATIVE_SYS_CLOCK_GET_MONOTONIC UINT64_C(73)
#define HYPER_NATIVE_SYS_VIRTUAL_SERIAL_CREATE UINT64_C(74)
#define HYPER_NATIVE_SYS_VIRTUAL_SERIAL_REGISTER_OUTPUT UINT64_C(75)
#define HYPER_NATIVE_SYS_VIRTUAL_SERIAL_WRITE UINT64_C(76)
#define HYPER_NATIVE_SYS_THREAD_CREATE UINT64_C(77)
#define HYPER_NATIVE_SYS_THREAD_START UINT64_C(78)
#define HYPER_NATIVE_SYS_THREAD_REQUEST_STOP UINT64_C(79)
#define HYPER_NATIVE_SYS_ATOMIC_WAIT UINT64_C(80)
#define HYPER_NATIVE_SYS_ATOMIC_WAKE UINT64_C(81)
#define HYPER_NATIVE_SYS_THREAD_SLEEP UINT64_C(82)
#define HYPER_NATIVE_SYS_FILE_WRITE_AT UINT64_C(83)
#define HYPER_NATIVE_SYS_FILE_RESIZE UINT64_C(84)
#define HYPER_NATIVE_SYS_DIRECTORY_CREATE_FILE UINT64_C(85)
#define HYPER_NATIVE_SYS_DIRECTORY_CREATE_DIRECTORY UINT64_C(86)
#define HYPER_NATIVE_SYS_DIRECTORY_REMOVE UINT64_C(87)
#define HYPER_NATIVE_SYS_WAIT_SET_CREATE UINT64_C(88)
#define HYPER_NATIVE_SYS_WAIT_SET_ADD UINT64_C(89)
#define HYPER_NATIVE_SYS_WAIT_SET_REARM UINT64_C(90)
#define HYPER_NATIVE_SYS_WAIT_SET_REMOVE UINT64_C(91)
#define HYPER_NATIVE_SYS_WAIT_SET_WAIT UINT64_C(92)
#define HYPER_NATIVE_SYS_PROCESS_GET_CURRENT_ID UINT64_C(93)
#define HYPER_NATIVE_SYS_VIRTUAL_SERIAL_ACKNOWLEDGE_OUTPUT UINT64_C(94)
#define HYPER_NATIVE_SYS_DIRECTORY_SCOPE_CREATE UINT64_C(95)
#define HYPER_NATIVE_SYS_DIRECTORY_GET_METADATA UINT64_C(96)
#define HYPER_NATIVE_SYS_FILE_GET_METADATA UINT64_C(97)
#define HYPER_NATIVE_SYS_DIRECTORY_GET_SELF_METADATA UINT64_C(98)
#define HYPER_NATIVE_SYS_DIRECTORY_SET_METADATA UINT64_C(99)
#define HYPER_NATIVE_SYS_FILE_SET_METADATA UINT64_C(100)
#define HYPER_NATIVE_SYS_DIRECTORY_RENAME UINT64_C(101)
#define HYPER_NATIVE_SYS_DIRECTORY_LINK UINT64_C(102)
#define HYPER_NATIVE_SYS_DIRECTORY_SYMLINK UINT64_C(103)
#define HYPER_NATIVE_SYS_DIRECTORY_READ_LINK UINT64_C(104)
#define HYPER_NATIVE_SYS_DIRECTORY_CANONICALIZE UINT64_C(105)
#define HYPER_NATIVE_SYS_DIRECTORY_REMOVE_IF UINT64_C(106)
#define HYPER_NATIVE_SYS_DIRECTORY_OPEN_DIRECTORY_NOFOLLOW UINT64_C(107)
#define HYPER_NATIVE_SYS_FILE_SYNC UINT64_C(108)
#define HYPER_NATIVE_SYS_FILE_LOCK UINT64_C(109)
#define HYPER_NATIVE_SYS_FILE_UNLOCK UINT64_C(110)
#define HYPER_NATIVE_SYS_CLOCK_GET_REALTIME UINT64_C(111)
#define HYPER_NATIVE_SYS_DIRECTORY_OPEN_FILE_WITH_OPTIONS UINT64_C(112)

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

#define HYPER_NATIVE_FILE_METADATA_MIN_SIZE UINT64_C(112)
typedef struct hyper_native_file_metadata_t {
    uint64_t filesystem_id;
    uint64_t mount_id;
    uint64_t node_id;
    uint64_t size;
    uint32_t mode;
    uint32_t kind;
    uint32_t valid_times;
    uint32_t reserved;
    int64_t accessed_seconds;
    uint32_t accessed_nanoseconds;
    uint32_t accessed_reserved;
    int64_t modified_seconds;
    uint32_t modified_nanoseconds;
    uint32_t modified_reserved;
    int64_t created_seconds;
    uint32_t created_nanoseconds;
    uint32_t created_reserved;
    int64_t changed_seconds;
    uint32_t changed_nanoseconds;
    uint32_t changed_reserved;
} hyper_native_file_metadata_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_file_metadata_t) == 112, "file_metadata size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_file_metadata_t) == 8, "file_metadata alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, filesystem_id) == 0, "file_metadata.filesystem_id offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, mount_id) == 8, "file_metadata.mount_id offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, node_id) == 16, "file_metadata.node_id offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, size) == 24, "file_metadata.size offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, mode) == 32, "file_metadata.mode offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, kind) == 36, "file_metadata.kind offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, valid_times) == 40, "file_metadata.valid_times offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, reserved) == 44, "file_metadata.reserved offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, accessed_seconds) == 48, "file_metadata.accessed_seconds offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, accessed_nanoseconds) == 56, "file_metadata.accessed_nanoseconds offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, accessed_reserved) == 60, "file_metadata.accessed_reserved offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, modified_seconds) == 64, "file_metadata.modified_seconds offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, modified_nanoseconds) == 72, "file_metadata.modified_nanoseconds offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, modified_reserved) == 76, "file_metadata.modified_reserved offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, created_seconds) == 80, "file_metadata.created_seconds offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, created_nanoseconds) == 88, "file_metadata.created_nanoseconds offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, created_reserved) == 92, "file_metadata.created_reserved offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, changed_seconds) == 96, "file_metadata.changed_seconds offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, changed_nanoseconds) == 104, "file_metadata.changed_nanoseconds offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_t, changed_reserved) == 108, "file_metadata.changed_reserved offset");

#define HYPER_NATIVE_FILE_METADATA_UPDATE_MIN_SIZE UINT64_C(40)
typedef struct hyper_native_file_metadata_update_t {
    uint32_t mask;
    uint32_t mode;
    int64_t accessed_seconds;
    uint32_t accessed_nanoseconds;
    uint32_t accessed_reserved;
    int64_t modified_seconds;
    uint32_t modified_nanoseconds;
    uint32_t modified_reserved;
} hyper_native_file_metadata_update_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_file_metadata_update_t) == 40, "file_metadata_update size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_file_metadata_update_t) == 8, "file_metadata_update alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_update_t, mask) == 0, "file_metadata_update.mask offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_update_t, mode) == 4, "file_metadata_update.mode offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_update_t, accessed_seconds) == 8, "file_metadata_update.accessed_seconds offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_update_t, accessed_nanoseconds) == 16, "file_metadata_update.accessed_nanoseconds offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_update_t, accessed_reserved) == 20, "file_metadata_update.accessed_reserved offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_update_t, modified_seconds) == 24, "file_metadata_update.modified_seconds offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_update_t, modified_nanoseconds) == 32, "file_metadata_update.modified_nanoseconds offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_metadata_update_t, modified_reserved) == 36, "file_metadata_update.modified_reserved offset");

#define HYPER_NATIVE_WAIT_SET_EVENT_MIN_SIZE UINT64_C(24)
typedef struct hyper_native_wait_set_event_t {
    uint64_t registration;
    uint64_t signals;
    uint64_t sequence;
} hyper_native_wait_set_event_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_wait_set_event_t) == 24, "wait_set_event size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_wait_set_event_t) == 8, "wait_set_event alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_wait_set_event_t, registration) == 0, "wait_set_event.registration offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_wait_set_event_t, signals) == 8, "wait_set_event.signals offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_wait_set_event_t, sequence) == 16, "wait_set_event.sequence offset");

#define HYPER_NATIVE_HANDLE_INFO_MIN_SIZE UINT64_C(16)
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

#define HYPER_NATIVE_OBJECT_BASIC_INFO_MIN_SIZE UINT64_C(16)
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

#define HYPER_NATIVE_OBJECT_WAIT_ITEM_MIN_SIZE UINT64_C(16)
typedef struct hyper_native_object_wait_item_t {
    uint64_t handle;
    uint64_t signals;
} hyper_native_object_wait_item_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_object_wait_item_t) == 16, "object_wait_item size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_object_wait_item_t) == 8, "object_wait_item alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_wait_item_t, handle) == 0, "object_wait_item.handle offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_wait_item_t, signals) == 8, "object_wait_item.signals offset");

#define HYPER_NATIVE_PROCESS_INFO_MIN_SIZE UINT64_C(32)
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

#define HYPER_NATIVE_CAPABILITY_DISPOSITION_MIN_SIZE UINT64_C(24)
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

#define HYPER_NATIVE_CAPABILITY_RECEIVE_SLOT_MIN_SIZE UINT64_C(24)
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

#define HYPER_NATIVE_STARTUP_HANDLE_MIN_SIZE UINT64_C(16)
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

#define HYPER_NATIVE_TASK_PROCESS_MIN_SIZE UINT64_C(96)
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

#define HYPER_NATIVE_TASK_THREAD_MIN_SIZE UINT64_C(104)
typedef struct hyper_native_task_thread_t {
    uint64_t koid;
    uint64_t process_koid;
    uint32_t role;
    uint32_t registry_phase;
    uint32_t name_length;
    uint32_t reserved;
    uint64_t runtime_ticks;
    uint8_t name[64];
} hyper_native_task_thread_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_task_thread_t) == 104, "task_thread size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_task_thread_t) == 8, "task_thread alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_thread_t, koid) == 0, "task_thread.koid offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_thread_t, process_koid) == 8, "task_thread.process_koid offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_thread_t, role) == 16, "task_thread.role offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_thread_t, registry_phase) == 20, "task_thread.registry_phase offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_thread_t, name_length) == 24, "task_thread.name_length offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_thread_t, reserved) == 28, "task_thread.reserved offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_thread_t, runtime_ticks) == 32, "task_thread.runtime_ticks offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_task_thread_t, name) == 40, "task_thread.name offset");

#define HYPER_NATIVE_MEMORY_OBSERVATION_MIN_SIZE UINT64_C(128)
typedef struct hyper_native_memory_observation_t {
    uint64_t captured_at_ns;
    uint64_t page_size;
    uint64_t total_bytes;
    uint64_t reserved_bytes;
    uint64_t managed_bytes;
    uint64_t free_bytes;
    uint64_t used_bytes;
    uint64_t kernel_bytes;
    uint64_t heap_bytes;
    uint64_t page_table_bytes;
    uint64_t user_bytes;
    uint64_t guest_bytes;
    uint64_t unattributed_bytes;
    uint64_t reclaimable_bytes;
    uint64_t cache_sample_complete;
    uint64_t buffered_bytes;
} hyper_native_memory_observation_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_memory_observation_t) == 128, "memory_observation size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_memory_observation_t) == 8, "memory_observation alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_observation_t, captured_at_ns) == 0, "memory_observation.captured_at_ns offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_observation_t, page_size) == 8, "memory_observation.page_size offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_observation_t, total_bytes) == 16, "memory_observation.total_bytes offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_observation_t, reserved_bytes) == 24, "memory_observation.reserved_bytes offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_observation_t, managed_bytes) == 32, "memory_observation.managed_bytes offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_observation_t, free_bytes) == 40, "memory_observation.free_bytes offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_observation_t, used_bytes) == 48, "memory_observation.used_bytes offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_observation_t, kernel_bytes) == 56, "memory_observation.kernel_bytes offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_observation_t, heap_bytes) == 64, "memory_observation.heap_bytes offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_observation_t, page_table_bytes) == 72, "memory_observation.page_table_bytes offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_observation_t, user_bytes) == 80, "memory_observation.user_bytes offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_observation_t, guest_bytes) == 88, "memory_observation.guest_bytes offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_observation_t, unattributed_bytes) == 96, "memory_observation.unattributed_bytes offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_observation_t, reclaimable_bytes) == 104, "memory_observation.reclaimable_bytes offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_observation_t, cache_sample_complete) == 112, "memory_observation.cache_sample_complete offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_memory_observation_t, buffered_bytes) == 120, "memory_observation.buffered_bytes offset");

#define HYPER_NATIVE_CPU_OBSERVATION_MIN_SIZE UINT64_C(64)
typedef struct hyper_native_cpu_observation_t {
    uint64_t captured_at_ns;
    uint64_t ticks_per_second;
    uint64_t online_cpus;
    uint64_t idle_ticks;
    uint64_t kernel_thread_ticks;
    uint64_t user_thread_ticks;
    uint64_t vcpu_ticks;
    uint64_t reserved;
} hyper_native_cpu_observation_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_cpu_observation_t) == 64, "cpu_observation size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_cpu_observation_t) == 8, "cpu_observation alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_cpu_observation_t, captured_at_ns) == 0, "cpu_observation.captured_at_ns offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_cpu_observation_t, ticks_per_second) == 8, "cpu_observation.ticks_per_second offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_cpu_observation_t, online_cpus) == 16, "cpu_observation.online_cpus offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_cpu_observation_t, idle_ticks) == 24, "cpu_observation.idle_ticks offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_cpu_observation_t, kernel_thread_ticks) == 32, "cpu_observation.kernel_thread_ticks offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_cpu_observation_t, user_thread_ticks) == 40, "cpu_observation.user_thread_ticks offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_cpu_observation_t, vcpu_ticks) == 48, "cpu_observation.vcpu_ticks offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_cpu_observation_t, reserved) == 56, "cpu_observation.reserved offset");

#define HYPER_NATIVE_OBJECT_INSPECTION_MIN_SIZE UINT64_C(104)
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
    uint64_t vm_device_binding_references;
} hyper_native_object_inspection_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_object_inspection_t) == 104, "object_inspection size");
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
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_object_inspection_t, vm_device_binding_references) == 96, "object_inspection.vm_device_binding_references offset");

#define HYPER_NATIVE_HANDLE_INSPECTION_MIN_SIZE UINT64_C(40)
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

#define HYPER_NATIVE_DIRECTORY_ENTRY_MIN_SIZE UINT64_C(280)
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

#define HYPER_NATIVE_FILE_INFO_MIN_SIZE UINT64_C(40)
typedef struct hyper_native_file_info_t {
    uint64_t filesystem_id;
    uint64_t mount_id;
    uint64_t node_id;
    uint64_t size;
    uint32_t mode;
    uint32_t reserved;
} hyper_native_file_info_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_file_info_t) == 40, "file_info size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_file_info_t) == 8, "file_info alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_info_t, filesystem_id) == 0, "file_info.filesystem_id offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_info_t, mount_id) == 8, "file_info.mount_id offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_info_t, node_id) == 16, "file_info.node_id offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_info_t, size) == 24, "file_info.size offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_info_t, mode) == 32, "file_info.mode offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_file_info_t, reserved) == 36, "file_info.reserved offset");

#define HYPER_NATIVE_DIRECTORY_INFO_MIN_SIZE UINT64_C(32)
typedef struct hyper_native_directory_info_t {
    uint64_t filesystem_id;
    uint64_t mount_id;
    uint64_t node_id;
    uint32_t mode;
    uint32_t reserved;
} hyper_native_directory_info_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_directory_info_t) == 32, "directory_info size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_directory_info_t) == 8, "directory_info alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_directory_info_t, filesystem_id) == 0, "directory_info.filesystem_id offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_directory_info_t, mount_id) == 8, "directory_info.mount_id offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_directory_info_t, node_id) == 16, "directory_info.node_id offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_directory_info_t, mode) == 24, "directory_info.mode offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_directory_info_t, reserved) == 28, "directory_info.reserved offset");

#define HYPER_NATIVE_VIRTUAL_MACHINE_CONFIGURATION_MIN_SIZE UINT64_C(32)
typedef struct hyper_native_virtual_machine_configuration_t {
    uint64_t guest_physical_base;
    uint64_t memory_size;
    uint32_t vcpu_count;
    uint32_t architecture;
    uint32_t platform_profile;
    uint32_t flags;
} hyper_native_virtual_machine_configuration_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_virtual_machine_configuration_t) == 32, "virtual_machine_configuration size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_virtual_machine_configuration_t) == 8, "virtual_machine_configuration alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_configuration_t, guest_physical_base) == 0, "virtual_machine_configuration.guest_physical_base offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_configuration_t, memory_size) == 8, "virtual_machine_configuration.memory_size offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_configuration_t, vcpu_count) == 16, "virtual_machine_configuration.vcpu_count offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_configuration_t, architecture) == 20, "virtual_machine_configuration.architecture offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_configuration_t, platform_profile) == 24, "virtual_machine_configuration.platform_profile offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_configuration_t, flags) == 28, "virtual_machine_configuration.flags offset");

#define HYPER_NATIVE_VIRTUAL_CPU_BOOTSTRAP_MIN_SIZE UINT64_C(64)
typedef struct hyper_native_virtual_cpu_bootstrap_t {
    uint64_t entry;
    uint64_t stack;
    uint64_t argument0;
    uint64_t argument1;
    uint64_t argument2;
    uint64_t argument3;
    uint32_t vcpu_id;
    uint32_t flags;
    uint64_t reserved;
} hyper_native_virtual_cpu_bootstrap_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_virtual_cpu_bootstrap_t) == 64, "virtual_cpu_bootstrap size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_virtual_cpu_bootstrap_t) == 8, "virtual_cpu_bootstrap alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_bootstrap_t, entry) == 0, "virtual_cpu_bootstrap.entry offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_bootstrap_t, stack) == 8, "virtual_cpu_bootstrap.stack offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_bootstrap_t, argument0) == 16, "virtual_cpu_bootstrap.argument0 offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_bootstrap_t, argument1) == 24, "virtual_cpu_bootstrap.argument1 offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_bootstrap_t, argument2) == 32, "virtual_cpu_bootstrap.argument2 offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_bootstrap_t, argument3) == 40, "virtual_cpu_bootstrap.argument3 offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_bootstrap_t, vcpu_id) == 48, "virtual_cpu_bootstrap.vcpu_id offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_bootstrap_t, flags) == 52, "virtual_cpu_bootstrap.flags offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_bootstrap_t, reserved) == 56, "virtual_cpu_bootstrap.reserved offset");

#define HYPER_NATIVE_VIRTUAL_MACHINE_INFO_MIN_SIZE UINT64_C(32)
typedef struct hyper_native_virtual_machine_info_t {
    uint32_t phase;
    uint32_t vcpu_count;
    uint64_t guest_physical_base;
    uint64_t memory_size;
    uint32_t architecture;
    uint32_t platform_profile;
} hyper_native_virtual_machine_info_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_virtual_machine_info_t) == 32, "virtual_machine_info size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_virtual_machine_info_t) == 8, "virtual_machine_info alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_info_t, phase) == 0, "virtual_machine_info.phase offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_info_t, vcpu_count) == 4, "virtual_machine_info.vcpu_count offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_info_t, guest_physical_base) == 8, "virtual_machine_info.guest_physical_base offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_info_t, memory_size) == 16, "virtual_machine_info.memory_size offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_info_t, architecture) == 24, "virtual_machine_info.architecture offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_machine_info_t, platform_profile) == 28, "virtual_machine_info.platform_profile offset");

#define HYPER_NATIVE_VIRTUAL_CPU_INFO_MIN_SIZE UINT64_C(24)
typedef struct hyper_native_virtual_cpu_info_t {
    uint32_t vcpu_id;
    uint32_t phase;
    uint64_t scheduler_thread_id;
    uint32_t terminal_reason;
    uint32_t reserved;
} hyper_native_virtual_cpu_info_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_virtual_cpu_info_t) == 24, "virtual_cpu_info size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_virtual_cpu_info_t) == 8, "virtual_cpu_info alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_info_t, vcpu_id) == 0, "virtual_cpu_info.vcpu_id offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_info_t, phase) == 4, "virtual_cpu_info.phase offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_info_t, scheduler_thread_id) == 8, "virtual_cpu_info.scheduler_thread_id offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_info_t, terminal_reason) == 16, "virtual_cpu_info.terminal_reason offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_virtual_cpu_info_t, reserved) == 20, "virtual_cpu_info.reserved offset");

#define HYPER_NATIVE_RESOURCE_LIMITS_MIN_SIZE UINT64_C(160)
typedef struct hyper_native_resource_limits_t {
    uint64_t kernel_memory_bytes;
    uint64_t processes;
    uint64_t threads;
    uint64_t handles;
    uint64_t kernel_objects;
    uint64_t committed_pages;
    uint64_t pinned_pages;
    uint64_t guest_pages;
    uint64_t ipc_messages;
    uint64_t ipc_bytes;
    uint64_t ipc_handles;
    uint64_t subscriptions;
    uint64_t timers;
    uint64_t virtual_machines;
    uint64_t virtual_cpus;
    uint64_t device_leases;
    uint64_t dma_mappings;
    uint64_t user_address_spaces;
    uint64_t user_mappings;
    uint64_t reserved;
} hyper_native_resource_limits_t;
HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_resource_limits_t) == 160, "resource_limits size");
HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_resource_limits_t) == 8, "resource_limits alignment");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, kernel_memory_bytes) == 0, "resource_limits.kernel_memory_bytes offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, processes) == 8, "resource_limits.processes offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, threads) == 16, "resource_limits.threads offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, handles) == 24, "resource_limits.handles offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, kernel_objects) == 32, "resource_limits.kernel_objects offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, committed_pages) == 40, "resource_limits.committed_pages offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, pinned_pages) == 48, "resource_limits.pinned_pages offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, guest_pages) == 56, "resource_limits.guest_pages offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, ipc_messages) == 64, "resource_limits.ipc_messages offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, ipc_bytes) == 72, "resource_limits.ipc_bytes offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, ipc_handles) == 80, "resource_limits.ipc_handles offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, subscriptions) == 88, "resource_limits.subscriptions offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, timers) == 96, "resource_limits.timers offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, virtual_machines) == 104, "resource_limits.virtual_machines offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, virtual_cpus) == 112, "resource_limits.virtual_cpus offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, device_leases) == 120, "resource_limits.device_leases offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, dma_mappings) == 128, "resource_limits.dma_mappings offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, user_address_spaces) == 136, "resource_limits.user_address_spaces offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, user_mappings) == 144, "resource_limits.user_mappings offset");
HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_resource_limits_t, reserved) == 152, "resource_limits.reserved offset");

#undef HYPER_ABI_ALIGNOF
#undef HYPER_ABI_STATIC_ASSERT

#endif /* HYPER_NATIVE_H */
