<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# HypeR Native ABI reference

This file is generated from `schema/native.rs`. Do not edit it directly.

ABI revision: `0`.

## Status values

| Value | Name |
| ---: | --- |
| 0 | `ok` |
| -1 | `invalid_argument` |
| -2 | `bad_handle` |
| -3 | `access_denied` |
| -4 | `not_supported` |
| -5 | `no_memory` |
| -6 | `bad_state` |
| -7 | `fault` |
| -8 | `resource_limit` |
| -9 | `busy` |
| -10 | `internal` |
| -11 | `timed_out` |
| -12 | `cancelled` |
| -13 | `would_block` |
| -14 | `buffer_too_small` |
| -15 | `peer_closed` |
| -16 | `not_found` |
| -17 | `already_exists` |
| -18 | `not_empty` |
| -19 | `not_directory` |
| -20 | `is_directory` |
| -21 | `symlink_loop` |
| -22 | `cross_device` |
| -23 | `io_error` |
| -24 | `no_space` |
| -25 | `read_only` |
| -26 | `revoked` |

## Object kinds

| Value | Name | Transfer |
| ---: | --- | --- |
| 0 | `none` | `forbidden` |
| 1 | `event` | `general` |
| 2 | `byte_channel` | `general` |
| 3 | `thread` | `rendezvous_only` |
| 4 | `process` | `rendezvous_only` |
| 5 | `task_group` | `rendezvous_only` |
| 6 | `resource_domain` | `general` |
| 7 | `task_factory` | `general` |
| 8 | `executable_authority` | `general` |
| 9 | `vmo` | `general` |
| 10 | `vmar` | `rendezvous_only` |
| 11 | `console` | `general` |
| 12 | `directory` | `general` |
| 13 | `file` | `general` |
| 14 | `capability_channel` | `rendezvous_only` |
| 15 | `process_builder` | `rendezvous_only` |
| 16 | `task_inspector` | `general` |
| 17 | `object_inspector` | `general` |
| 18 | `memory_inspector` | `general` |
| 19 | `cpu_inspector` | `general` |
| 20 | `virtual_machine_creation_authority` | `general` |
| 21 | `virtual_machine_creation_lease` | `rendezvous_only` |
| 22 | `pending_virtual_machine` | `rendezvous_only` |
| 23 | `virtual_machine` | `rendezvous_only` |
| 24 | `virtual_cpu` | `rendezvous_only` |
| 25 | `virtual_serial` | `rendezvous_only` |
| 26 | `wait_set` | `forbidden` |
| 27 | `guest_memory` | `rendezvous_only` |
| 28 | `device_assignment_authority` | `general` |
| 29 | `physical_device` | `rendezvous_only` |
| 30 | `guest_mailbox` | `rendezvous_only` |
| 31 | `guest_notification` | `rendezvous_only` |
| 32 | `native_block` | `rendezvous_only` |
| 33 | `guest_mapping` | `rendezvous_only` |
| 34 | `file_protection` | `forbidden` |
| 35 | `directory_stream` | `general` |
| 36 | `root_transition` | `rendezvous_only` |
| 37 | `guest_interrupt` | `rendezvous_only` |
| 38 | `process_retirement` | `rendezvous_only` |
| 39 | `crash_view` | `general` |
| 40 | `backend_memory_lease` | `forbidden` |
| 41 | `execution_context` | `forbidden` |
| 42 | `object_factory` | `forbidden` |
| 43 | `system_info` | `general` |

## Object signals

| Object | Bit | Name |
| --- | ---: | --- |
| `process_retirement` | 0 | `ready` |
| `process_retirement` | 1 | `released` |
| `physical_device` | 0 | `readable` |
| `physical_device` | 1 | `dma_fault` |
| `native_block` | 0 | `peer_closed` |
| `virtual_serial` | 0 | `readable` |
| `virtual_serial` | 1 | `writable` |
| `virtual_serial` | 2 | `peer_closed` |
| `wait_set` | 0 | `readable` |
| `event` | 0 | `signaled` |
| `byte_channel` | 0 | `readable` |
| `byte_channel` | 1 | `writable` |
| `byte_channel` | 2 | `peer_closed` |
| `capability_channel` | 0 | `peer_receiving` |
| `capability_channel` | 1 | `peer_closed` |
| `thread` | 0 | `terminated` |
| `task_group` | 0 | `terminated` |
| `process` | 0 | `terminated` |
| `console` | 0 | `readable` |
| `console` | 1 | `writable` |
| `virtual_machine` | 0 | `terminated` |
| `virtual_machine` | 1 | `power_request` |
| `virtual_machine` | 2 | `vcpu_terminated` |
| `virtual_cpu` | 0 | `terminated` |
| `virtual_cpu` | 1 | `mmio_request` |
| `guest_notification` | 0 | `peer_closed` |
| `guest_mailbox` | 0 | `readable` |
| `guest_mailbox` | 1 | `writable` |
| `guest_mailbox` | 2 | `peer_closed` |

## Scalar domains

Enum domains contain mutually exclusive values; flags contain independent bits.
Constants contain bounds, sizes, addresses and sentinels. All are exported as
fixed-width integer constants; grouping does not change wire field types.

### `handle_encoding` (constants)

Handle value classes; the remaining bits are opaque process-local data.

| Name | Value | Meaning |
| --- | ---: | --- |
| `handle_invalid` | `0` | No handle is present. This value never identifies a live handle. |
| `handle_prefix_mask` | `18374686479671623680` | Selects the most significant byte of a wire handle. |
| `handle_prefix` | `1080863910568919040` | Live handles have prefix 0x0F; all other prefixes are reserved. |

### `core_limits` (constants)

Page geometry, record bounds, identity encoding and wait limits.

| Name | Value | Meaning |
| --- | ---: | --- |
| `page_size` | `4096` | Native memory-management granule in bytes. |
| `extensible_record_max_bytes` | `4096` | Upper bound on a caller-supplied extensible record, including its tail. |
| `koid_slot_mask` | `4294967295` | Low slot bits of a KOID; the generation is required for identity. |
| `deadline_infinite` | `18446744073709551615` | Wait without a deadline; finite deadlines use absolute monotonic nanoseconds. |
| `object_wait_many_max_items` | `64` |  |

### `system_config` (enum)

Selectors for system configuration queries.

| Name | Value | Meaning |
| --- | ---: | --- |
| `system_config_page_size` | `1` |  |
| `system_config_application_address_limit` | `2` |  |

### `startup_limits` (constants)

Executable identifiers and kernel-to-loader transport bounds.

| Name | Value | Meaning |
| --- | ---: | --- |
| `elf_osabi` | `63` | `HypeR`'s ELF OSABI byte; this is not a Linux or generic System V image. |
| `elf_abi_version` | `0` |  |
| `process_startup_data_max_bytes` | `16384` |  |
| `startup_max_handles` | `256` |  |
| `boot_command_line_max_bytes` | `512` | Maximum UTF-8 boot-argument payload, excluding its length prefix. |

### `loader_startup` (enum)

Kernel-to-loader startup message tags; payload semantics belong to userspace.

| Name | Value | Meaning |
| --- | ---: | --- |
| `loader_startup_reply` | `1` |  |

### `startup_handle_purpose` (enum)

Purposes assigned to capabilities in the process startup table.

| Name | Value | Meaning |
| --- | ---: | --- |
| `startup_handle_purpose_resource_domain` | `1` |  |
| `startup_handle_purpose_task_group` | `2` |  |
| `startup_handle_purpose_task_factory` | `3` |  |
| `startup_handle_purpose_executable_authority` | `4` |  |
| `startup_handle_purpose_root_vmar` | `5` |  |
| `startup_handle_purpose_console` | `6` |  |
| `startup_handle_purpose_root_directory` | `7` |  |
| `startup_handle_purpose_task_inspector` | `8` |  |
| `startup_handle_purpose_object_inspector` | `9` |  |
| `startup_handle_purpose_dynamic_library_directory` | `10` |  |
| `startup_handle_purpose_memory_inspector` | `11` |  |
| `startup_handle_purpose_cpu_inspector` | `12` |  |
| `startup_handle_purpose_virtual_machine_creation_authority` | `13` |  |
| `startup_handle_purpose_device_assignment_authority` | `14` |  |
| `startup_handle_purpose_initial_stack_vmar` | `15` |  |
| `startup_handle_purpose_execution_context` | `16` |  |
| `startup_handle_purpose_object_factory` | `17` |  |
| `startup_handle_purpose_system_info` | `18` |  |
| `startup_handle_purpose_boot_command_line` | `19` | Unconsumed boot arguments: immutable VMO with READ \| INSPECT rights.  Contains a little-endian u64 byte length followed by that many UTF-8  bytes (no NUL), bounded by `BOOT_COMMAND_LINE_MAX_BYTES`. The kernel  passes unrecognized arguments to its initial process. Each userspace  boot stage may pass its own remainder to the successor under this purpose. |

### `process_phase` (enum)

Process lifecycle phases.

| Name | Value | Meaning |
| --- | ---: | --- |
| `process_phase_prepared` | `0` |  |
| `process_phase_created` | `1` |  |
| `process_phase_running` | `2` |  |
| `process_phase_stopping` | `3` |  |
| `process_phase_stopped` | `4` |  |
| `process_phase_retiring` | `5` |  |
| `process_phase_retired` | `6` |  |

### `process_terminal` (enum)

Reasons a process stopped.

| Name | Value | Meaning |
| --- | ---: | --- |
| `process_terminal_none` | `0` |  |
| `process_terminal_requested` | `1` |  |
| `process_terminal_thread_exited` | `2` |  |
| `process_terminal_process_exited` | `3` |  |
| `process_terminal_last_thread_exited` | `4` |  |
| `process_terminal_fault` | `5` |  |
| `process_terminal_task_group_stop` | `6` |  |

### `thread_role` (enum)

Scheduler thread roles reported by inspection.

| Name | Value | Meaning |
| --- | ---: | --- |
| `thread_role_bootstrap` | `1` |  |
| `thread_role_idle` | `2` |  |
| `thread_role_kernel` | `3` |  |
| `thread_role_user` | `4` |  |
| `thread_role_vcpu` | `5` |  |

### `scheduling_policy` (enum)

Scheduling algorithms; these do not identify the thread execution payload.

| Name | Value | Meaning |
| --- | ---: | --- |
| `scheduling_policy_fair` | `1` |  |
| `scheduling_policy_budget` | `2` |  |
| `scheduling_policy_fifo` | `3` |  |

### `scheduling_role` (enum)

Eligibility for CPU-local priority and minimum-progress allowances.

| Name | Value | Meaning |
| --- | ---: | --- |
| `scheduling_role_ordinary` | `0` |  |
| `scheduling_role_priority` | `1` |  |
| `scheduling_role_protected` | `2` |  |

### `scheduling_fair_backend` (enum)

Fair queue implementations supported by the selected kernel image.

| Name | Value | Meaning |
| --- | ---: | --- |
| `scheduling_fair_backend_stride` | `0` |  |
| `scheduling_fair_backend_round_robin` | `1` |  |
| `scheduling_fair_backend_default` | `2` |  |

### `thread_registry` (enum)

Thread registry membership states.

| Name | Value | Meaning |
| --- | ---: | --- |
| `thread_registry_resident` | `1` |  |
| `thread_registry_retiring` | `2` |  |

### `task_limits` (constants)

Task names, affinity bounds and inspection page sizes.

| Name | Value | Meaning |
| --- | ---: | --- |
| `thread_name_max_bytes` | `64` | UTF-8 bytes excluding any NUL terminator; control characters are rejected. |
| `process_name_max_bytes` | `64` |  |
| `process_affinity_max_words` | `4` |  |
| `process_affinity_max_cpus` | `256` |  |
| `task_inspector_process_page_capacity` | `8` |  |
| `task_inspector_thread_page_capacity` | `8` |  |
| `object_inspector_object_page_capacity` | `8` |  |
| `object_inspector_handle_page_capacity` | `8` |  |

### `object_detail` (enum)

Payload tags for detailed object inspection.

| Name | Value | Meaning |
| --- | ---: | --- |
| `object_detail_empty` | `0` |  |
| `object_detail_thread` | `1` |  |
| `object_detail_vmar` | `2` |  |
| `object_detail_mapping` | `3` |  |
| `object_detail_channel` | `4` |  |
| `object_detail_device` | `5` |  |
| `object_detail_device_resource` | `6` |  |

### `object_handle_state` (enum)

Userspace handle publication states.

| Name | Value | Meaning |
| --- | ---: | --- |
| `object_handle_state_unpublished` | `1` |  |
| `object_handle_state_active` | `2` |  |
| `object_handle_state_retired` | `3` |  |

### `process_retirement` (enum)

Retained process lifecycle phases.

| Name | Value | Meaning |
| --- | ---: | --- |
| `process_retirement_prepared` | `0` |  |
| `process_retirement_running` | `1` |  |
| `process_retirement_stopping` | `2` |  |
| `process_retirement_ready` | `3` |  |
| `process_retirement_released` | `4` |  |

### `process_retirement_release` (enum)

Reasons retained process state was released.

| Name | Value | Meaning |
| --- | ---: | --- |
| `process_retirement_release_none` | `0` |  |
| `process_retirement_release_explicit` | `1` |  |
| `process_retirement_release_owner_lost` | `2` |  |
| `process_retirement_release_deadline` | `3` |  |
| `process_retirement_release_aborted` | `4` |  |

### `crash_partial` (flags)

Incomplete sections of retained crash state.

| Name | Value | Meaning |
| --- | ---: | --- |
| `crash_partial_memory` | `1` |  |
| `crash_partial_threads` | `2` |  |
| `crash_partial_handles` | `4` |  |
| `crash_partial_mappings` | `8` |  |

### `crash_mapping_status` (flags)

Retention and exclusion flags for a crash mapping.

| Name | Value | Meaning |
| --- | ---: | --- |
| `crash_mapping_retained` | `1` |  |
| `crash_mapping_partial` | `2` |  |
| `crash_mapping_excluded_shared` | `4` |  |
| `crash_mapping_excluded_file` | `8` |  |

### `crash_mapping_kind` (enum)

Backing kinds reported for a crash mapping.

| Name | Value | Meaning |
| --- | ---: | --- |
| `crash_mapping_anonymous` | `1` |  |
| `crash_mapping_private_file` | `2` |  |
| `crash_mapping_shared` | `3` |  |

### `crash_limits` (constants)

Maximum bytes returned by one crash-memory read.

| Name | Value | Meaning |
| --- | ---: | --- |
| `crash_read_max_bytes` | `65536` |  |

### `memory_statistics` (enum)

Selectors for detailed memory statistics.

| Name | Value | Meaning |
| --- | ---: | --- |
| `memory_statistics_heap` | `1` |  |
| `memory_statistics_buddy` | `2` |  |
| `memory_statistics_owner` | `3` |  |
| `memory_statistics_file_cache` | `4` |  |
| `memory_statistics_heap_class` | `5` |  |

### `memory_statistics_dimensions` (constants)

Dimensions of memory statistics records.

| Name | Value | Meaning |
| --- | ---: | --- |
| `memory_statistics_buddy_order_count` | `19` |  |
| `memory_statistics_owner_count` | `5` |  |
| `memory_statistics_heap_class_count` | `18` |  |

### `private_mapping` (enum)

Private mapping population policies.

| Name | Value | Meaning |
| --- | ---: | --- |
| `private_mapping_copy_on_write` | `0` | Share source pages until the mapping is written; modifications remain private. |
| `private_mapping_eager` | `1` | Populate private pages immediately instead of deferring copying to write faults. |

### `vmar_permission` (flags)

Permissions which may be combined for a mapping.

| Name | Value | Meaning |
| --- | ---: | --- |
| `vmar_permission_read` | `1` |  |
| `vmar_permission_write` | `2` |  |
| `vmar_permission_execute` | `4` |  |

### `vmar_allocate` (flags)

Options for address-space allocation.

| Name | Value | Meaning |
| --- | ---: | --- |
| `vmar_allocate_exact` | `1` |  |

### `vmar_memory` (flags)

Options for locking currently mapped memory.

| Name | Value | Meaning |
| --- | ---: | --- |
| `vmar_memory_current` | `1` | Lock mappings that exist now; future mappings do not inherit the lock.  Unmapping and process teardown still release the corresponding residency. |

### `vmo_limits` (constants)

VMO size and transfer bounds.

| Name | Value | Meaning |
| --- | ---: | --- |
| `vmo_max_size_bytes` | `4294967296` |  |
| `vmo_max_transfer_bytes` | `65536` |  |
| `vmo_max_contiguous_size_bytes` | `134217728` |  |

### `capability_disposition` (enum)

Ownership operations for a transferred capability.

| Name | Value | Meaning |
| --- | ---: | --- |
| `capability_disposition_move` | `0` | Transfer ownership only when the operation commits; failures preserve the source. |
| `capability_disposition_duplicate` | `1` | Keep the source handle; requires DUPLICATE in addition to transfer authority. |

### `channel_limits` (constants)

Channel message, queue and capability-transfer bounds.

| Name | Value | Meaning |
| --- | ---: | --- |
| `byte_channel_max_message_bytes` | `65536` |  |
| `byte_channel_max_queued_messages` | `16` |  |
| `byte_channel_max_queued_bytes` | `1048576` |  |
| `capability_channel_max_message_bytes` | `4096` |  |
| `capability_channel_max_handles` | `64` |  |
| `capability_disposition_same_rights` | `18446744073709551615` | Request the source's current rights, not every right defined by the ABI. |

### `console_limits` (constants)

Console transfer bounds and virtual serial shared-memory layout.

| Name | Value | Meaning |
| --- | ---: | --- |
| `console_max_transfer_bytes` | `4096` |  |
| `virtual_serial_max_transfer_bytes` | `4096` |  |
| `virtual_serial_output_capacity` | `65536` | Byte slots in the producer ring; unread bytes are never overwritten. |
| `virtual_serial_output_header_bytes` | `4096` | Byte offset of ring data after its shared atomic counter header. |
| `virtual_serial_output_bytes` | `69632` | Exact VMO size required by `virtual_serial_register_output`. |

### `directory_entry_kind` (enum)

Kinds reported by directory enumeration.

| Name | Value | Meaning |
| --- | ---: | --- |
| `directory_entry_kind_file` | `1` |  |
| `directory_entry_kind_directory` | `2` |  |
| `directory_entry_kind_symlink` | `3` |  |
| `directory_entry_kind_other` | `4` |  |

### `filesystem_limits` (constants)

Path, directory-page, transfer and root-switch bounds.

| Name | Value | Meaning |
| --- | ---: | --- |
| `directory_entry_page_capacity` | `4` |  |
| `directory_entry_name_max_bytes` | `255` |  |
| `directory_max_path_bytes` | `4096` |  |
| `file_max_read_bytes` | `2097152` |  |
| `filesystem_switch_root_max_moves` | `16` |  |

### `device_profile` (enum)

Physical-device assignment profiles.

| Name | Value | Meaning |
| --- | ---: | --- |
| `device_profile_virtio_mmio_scsi` | `1` |  |
| `device_profile_userspace` | `2` |  |
| `device_profile_virtio_mmio_net` | `3` |  |
| `device_profile_pci_function` | `4` |  |

### `device_identity` (enum)

Selectors for device identity matching.

| Name | Value | Meaning |
| --- | ---: | --- |
| `device_identity_compatible` | `1` |  |
| `device_identity_fdt_path` | `2` |  |
| `device_identity_pci_id` | `3` |  |

### `device_firmware_field` (enum)

Fields exposed by firmware inspection.

| Name | Value | Meaning |
| --- | ---: | --- |
| `device_firmware_field_info` | `0` |  |
| `device_firmware_field_path` | `1` |  |
| `device_firmware_field_compatible` | `2` |  |
| `device_firmware_field_registers` | `3` |  |
| `device_firmware_field_property` | `4` |  |
| `device_firmware_field_property_names` | `5` |  |

### `device_resource_ids` (constants)

PCI resource identifiers; BAR0 is the base of the BAR index range.

| Name | Value | Meaning |
| --- | ---: | --- |
| `device_resource_pci_ecam` | `256` |  |
| `device_resource_pci_msi` | `257` |  |
| `device_resource_pci_bar0` | `512` | Add the zero-based BAR index to identify a PCI BAR resource. |
| `device_resource_pci_intx` | `258` |  |

### `device_resource_flags` (flags)

PCI resource attributes.

| Name | Value | Meaning |
| --- | ---: | --- |
| `device_resource_memory_64` | `1` |  |
| `device_resource_prefetchable` | `2` |  |

### `device_limits` (constants)

Device discovery, assignment and PCI aperture bounds.

| Name | Value | Meaning |
| --- | ---: | --- |
| `device_firmware_max_bytes` | `65536` |  |
| `device_firmware_name_max_bytes` | `128` |  |
| `device_bundle_max_entries` | `8` |  |
| `device_pci_aperture_size` | `8388608` |  |
| `device_assignment_max_devices` | `4` |  |

### `dma_fault_source` (enum)

DMA isolation mechanisms reporting a fault.

| Name | Value | Meaning |
| --- | ---: | --- |
| `dma_fault_source_smmuv3` | `1` |  |

### `dma_fault_scope` (enum)

Scope of a DMA fault report.

| Name | Value | Meaning |
| --- | ---: | --- |
| `dma_fault_scope_stream` | `1` |  |
| `dma_fault_scope_controller` | `2` |  |

### `dma_fault_flags` (flags)

Valid-address and access-direction flags in DMA fault reports.

| Name | Value | Meaning |
| --- | ---: | --- |
| `dma_fault_address_valid` | `1` | The reported address is meaningful; a controller-wide report may omit it. |
| `dma_fault_read` | `2` |  |
| `dma_fault_write` | `4` |  |

### `native_block_layout` (constants)

Shared-memory geometry and transfer bounds for the Native block initiator.

| Name | Value | Meaning |
| --- | ---: | --- |
| `native_block_transfer_bytes` | `524288` |  |
| `native_block_batch_max` | `4` |  |
| `native_block_batch_record_bytes` | `16` |  |
| `native_block_transfer_frame_bytes` | `524352` |  |
| `native_block_memory_bytes` | `1048576` |  |
| `native_block_queue_count` | `6` |  |
| `native_block_queue_size` | `8` |  |
| `native_block_queue_stride` | `4096` |  |
| `native_block_available_offset` | `256` |  |
| `native_block_used_offset` | `512` |  |

### `guest_notification` (enum)

Guest notification control operations.

| Name | Value | Meaning |
| --- | ---: | --- |
| `guest_notification_disable` | `0` |  |
| `guest_notification_enable` | `1` |  |
| `guest_notification_raise_config` | `2` |  |
| `guest_notification_disconnect` | `3` |  |

### `io_transport_limits` (constants)

I/O client, mailbox and guest alias-window bounds.

| Name | Value | Meaning |
| --- | ---: | --- |
| `io_max_clients` | `9` |  |
| `guest_mailbox_max_message_bytes` | `256` |  |
| `guest_dynamic_alias_offset` | `68719476736` |  |
| `guest_dynamic_physical_limit` | `274877906944` |  |

### `virtual_machine_architecture` (enum)

Architecture identifiers for guest construction.

| Name | Value | Meaning |
| --- | ---: | --- |
| `virtual_machine_architecture_aarch64` | `1` |  |
| `virtual_machine_architecture_riscv64` | `2` |  |
| `virtual_machine_architecture_x86_64` | `3` |  |

### `virtual_machine_power` (enum)

Guest power-management requests.

| Name | Value | Meaning |
| --- | ---: | --- |
| `virtual_machine_power_cpu_on` | `1` |  |
| `virtual_machine_power_cpu_off` | `2` |  |
| `virtual_machine_power_system_off` | `3` |  |
| `virtual_machine_power_system_reset` | `4` |  |

### `virtual_machine_phase` (enum)

Installed guest lifecycle phases.

| Name | Value | Meaning |
| --- | ---: | --- |
| `virtual_machine_phase_installed` | `1` |  |
| `virtual_machine_phase_running` | `2` |  |
| `virtual_machine_phase_stopping` | `3` |  |
| `virtual_machine_phase_stopped` | `4` |  |

### `virtual_cpu_phase` (enum)

Virtual CPU lifecycle phases.

| Name | Value | Meaning |
| --- | ---: | --- |
| `virtual_cpu_phase_dormant` | `1` |  |
| `virtual_cpu_phase_started` | `2` |  |
| `virtual_cpu_phase_stopped` | `3` |  |

### `virtual_cpu_terminal` (enum)

Virtual CPU termination reasons.

| Name | Value | Meaning |
| --- | ---: | --- |
| `virtual_cpu_terminal_none` | `0` |  |
| `virtual_cpu_terminal_memory_fault` | `1` |  |
| `virtual_cpu_terminal_mmio` | `2` |  |
| `virtual_cpu_terminal_synchronous` | `3` |  |
| `virtual_cpu_terminal_administrative` | `4` |  |

### `virtual_cpu_sentinels` (constants)

Sentinel for an unknown host CPU assignment.

| Name | Value | Meaning |
| --- | ---: | --- |
| `vcpu_host_cpu_unknown` | `4294967295` | No host CPU assignment is available; do not treat this as a CPU index. |

### `virtual_platform` (enum)

Reference platform identifiers.

| Name | Value | Meaning |
| --- | ---: | --- |
| `virtual_platform_aarch64_reference` | `1` |  |
| `virtual_platform_riscv64_reference` | `2` |  |

### `virtual_platform_aarch64_layout` (constants)

Immutable `AArch64` reference platform geometry shared by kernel and userspace.

| Name | Value | Meaning |
| --- | ---: | --- |
| `virtual_platform_aarch64_reference_max_vcpus` | `8` |  |
| `virtual_platform_aarch64_reference_guest_ram_base` | `1073741824` |  |
| `virtual_platform_aarch64_reference_dtb_offset` | `65536` |  |
| `virtual_platform_aarch64_reference_gic_distributor_base` | `134217728` |  |
| `virtual_platform_aarch64_reference_gic_distributor_size` | `65536` |  |
| `virtual_platform_aarch64_reference_gic_redistributor_base` | `134873088` |  |
| `virtual_platform_aarch64_reference_gic_redistributor_size` | `131072` |  |
| `virtual_platform_aarch64_reference_gicv2_distributor_size` | `4096` |  |
| `virtual_platform_aarch64_reference_gicv2_cpu_base` | `134283264` |  |
| `virtual_platform_aarch64_reference_gicv2_cpu_size` | `8192` |  |
| `virtual_platform_aarch64_reference_uart_base` | `150994944` |  |
| `virtual_platform_aarch64_reference_user_mmio_base` | `167772160` |  |
| `virtual_platform_aarch64_reference_user_mmio_size` | `16777216` |  |
| `virtual_platform_aarch64_reference_uart_size` | `4096` |  |
| `virtual_platform_aarch64_reference_uart_baud` | `115200` |  |
| `virtual_platform_aarch64_reference_uart_interrupt` | `33` |  |
| `virtual_platform_aarch64_reference_timer_interrupt` | `27` |  |
| `virtual_platform_aarch64_reference_interrupt_count` | `256` |  |
| `virtual_platform_aarch64_reference_device_interrupt_base` | `40` |  |

### `virtual_platform_riscv64_layout` (constants)

Immutable RISC-V reference platform geometry shared by kernel and userspace.

| Name | Value | Meaning |
| --- | ---: | --- |
| `virtual_platform_riscv64_reference_guest_ram_base` | `2147483648` |  |
| `virtual_platform_riscv64_reference_dtb_offset` | `65536` |  |
| `virtual_platform_riscv64_reference_uart_base` | `268435456` |  |
| `virtual_platform_riscv64_reference_uart_size` | `4096` |  |
| `virtual_platform_riscv64_reference_uart_interrupt` | `10` |  |
| `virtual_platform_riscv64_reference_uart_clock_frequency` | `3686400` |  |
| `virtual_platform_riscv64_reference_plic_base` | `201326592` |  |
| `virtual_platform_riscv64_reference_plic_size` | `4194304` |  |
| `virtual_platform_riscv64_reference_plic_num_sources` | `31` |  |
| `virtual_platform_riscv64_reference_plic_supervisor_context` | `1` |  |

### `riscv_isa` (flags)

RISC-V ISA extensions accepted in guest configuration.

| Name | Value | Meaning |
| --- | ---: | --- |
| `riscv_isa_i` | `1` |  |
| `riscv_isa_m` | `2` |  |
| `riscv_isa_a` | `4` |  |
| `riscv_isa_f` | `8` |  |
| `riscv_isa_d` | `16` |  |
| `riscv_isa_c` | `32` |  |
| `riscv_isa_zicsr` | `64` |  |
| `riscv_isa_zifencei` | `128` |  |
| `riscv_isa_sstc` | `256` |  |

## Semantic rules

- Native invocations carry a process-local target handle in the selector register, a separate object-local operation, and six business argument registers. AArch64 uses x8/x6/x0..x5; RISC-V uses a7/a6/a0..a5. No global syscall number or implicit privileged target exists. Common handle operations use 0..5; object-specific operations start at 256 and may repeat across object kinds. Unknown target handles fail before operation dispatch; unknown operations return NOT_SUPPORTED. The receiver is not duplicated in the business arguments.
- ExecutionContext and ObjectFactory are ordinary process-local, nontransferable capabilities bound to their creating Process. They never grant operations on another Process, Thread, address space or resource domain. ExecutionContext methods act on the invoking Thread or its Process; ObjectFactory charges every allocation to that Process's resource domain. SystemInfo grants public read-only ABI, configuration and clock observations. Closing or attenuating a capability does not implicitly recreate its authority.
- device_dma_fault_info requires INSPECT and ASSIGN_DEVICE on the physical-device handle. It returns the first retained DMA containment report without consuming it; an all-zero record means no report. DMA_FAULT is a persistent level signal observed with WAIT. Claim and claim_matching grant WAIT. Fault evidence follows the device object across handle transfer; querying does not acknowledge or resume DMA. The kernel never selects a userspace process or stops a VM for this notification. The formal I/O runtime queries after adoption and journals each device's first report; bootstrap does not consume it. timestamp_ns is monotonic boot time. source 1 identifies SMMUv3. scope 1 means stream quarantine, with code holding the SMMUv3 event ID; scope 2 means controller containment, with code holding the captured GERROR bits (zero is possible for a software-detected failure). stream_id always identifies this handle's device. ADDRESS_VALID, READ and WRITE describe the address only for translation/access faults; controller reports disclose no other device's address. reserved is zero. A first stream report remains unchanged if the whole controller later fails. Quarantine and backing retention remain independent of observer presence or progress.
- Scheduling reads require INSPECT on ExecutionContext, Thread or VirtualCpu. ExecutionContext targets only the invoking Thread; VirtualCpu targets its scheduler Thread. Configuration is a complete 64-byte record with canonical zero inactive/reserved fields. Fair weight is 1..1024 and quantum 1..1000000000 nanoseconds; Fair backend 2 requests the configured default, while reads return concrete backend 0 or 1. Budget Q, P and burst are positive, Q and burst do not exceed P, and P must equal the kernel arbitration period. Disabled policies or malformed parameters return INVALID_ARGUMENT before mutation.
- Scheduling changes compare old and requested parameters atomically under scheduler ownership. SCHED_LOWER permits lowering band, Fair weight or FIFO priority; SCHED_PRIORITY permits raising those fields and changing quantum; SCHED_CLASS permits policy, arbitration role or Fair-backend changes; SCHED_BUDGET permits any budget change. A compound change may require multiple rights. WRITE, thread ownership, executable names and configuration text confer no elevation. Immutable target ceilings remain enforced after handle attenuation or transfer. Rejection preserves the old parameters.
- ProcessBuilder scheduling configuration also declares the child's execution-context scheduling subset. It cannot delegate rights absent from the builder. Ordinary process creation uses the native App boundary and preserves ordinary demotion; in-process thread_create inherits its caller's current parameters and cannot restore a higher initial weight. All threads receive SCHED_LOWER for self demotion; child Thread handles receive only scheduling rights present in both the execution-context receiver and process grant. VM creation authorities propagate explicitly held scheduling rights through leases, pending machines, installed machines and vCPUs. Thread handles may transfer only through explicit capability rendezvous, with the receiver retaining an attenuated subset of the sender's rights. Creating, blocking, migrating or switching policy does not authorize resetting an existing Thread's budget accounting. Budget entitlement is per Thread, not an aggregate Process or VM cgroup quota; separately authorized construction creates independently accounted Threads.
- thread_set_current_name changes only the calling Native Thread's diagnostic label. The caller must hold its ExecutionContext with WRITE; no other Thread or Process can be renamed. Input is 1..thread_name_max_bytes (64) bytes of UTF-8 without control characters, excluding any NUL terminator. Empty, oversized, invalid UTF-8, control-containing names and nonzero unused arguments return INVALID_ARGUMENT without changing the old name. Inspectors receive complete old or new snapshots; names confer no identity or authority. Process names remain unchanged.
- memory_inspector_read_statistics requires MemoryInspector INSPECT; it is observational and never drains caches or initiates reclaim. Topic selects a typed extensible record: heap (index 0), buddy (orders 0..18), owner (0 kernel, 1 page tables, 2 guest, 3 user, 4 file cache), file_cache (index 0), or heap_class (0..8 kernel, 9..17 file-cache pool; eight slab sizes 16..2048 then large allocations). Unknown topic/index and a nonzero unused argument are INVALID_ARGUMENT. Heap-class records require CONFIG_HEAP_PROFILING, otherwise NOT_SUPPORTED. Other records are always available after allocator initialization.
- Memory statistics are independently sampled, including fields sourced from atomic counters; they are not a transaction across topics, CPUs, or logical and physical accounting. Page counts use memory_observation.page_size. Heap-class reserved slots include live objects, magazine objects and in-flight transfers; large buckets report zero capacity/reserved/cached and object_size=0. File-cache metadata physical pages and reservations overlap other accounting and must not be added twice. Cumulative heap-class allocation/deallocation counters saturate and reset only on reboot. No address or allocation authority is exposed.
- object_inspector_read_details requires INSPECT and INSPECT_DETAILS on an ObjectInspector. With process_koid zero, target is a KOID and only system scope is allowed. Otherwise target is a generation-qualified handle in the named, scope-visible process. Inspection takes a diagnostic reference under the lookup lock, releases that lock, and copies object-local metadata only; it does not grant operation authority. Cursor zero starts a query; next_cursor zero finishes it. Empty records may advance a bounded scan without yielding a mapping. Concurrent mutation makes pages weakly consistent. Unsupported object kinds return NOT_SUPPORTED; inaccessible or expired identities return NOT_FOUND. Inspector derivation continues to produce only the original basic rights, never implicitly acquiring INSPECT_DETAILS.
- Object-details payload consists of eight little-endian u64 words. All unused words are zero. Record 0 is empty. Thread (1): scheduler TID (zero is a valid bootstrap TID), role (task_thread role values), user lifecycle phase (0 unavailable, 1 prepared, 2 dormant, 3 runnable, 4 stop-requested, 5 detached), TID present (0/1; absent TID word is zero). VMAR (2): base, length, live (0/1). Mapping (3): base, length, current R/W/X bits, maximum R/W/X bits. Channel (4): peer KOID (zero until pair publication), local open (0/1), peer open (0/1), queued messages or receivers, queued bytes, byte-queue counters present (0/1). Device (5): profile, virtio device ID, PCI vendor/device word, host IRQ domain, first host interrupt, interrupt count, lifecycle (1 claimed, 2 attached, 3 active, 4 retired, 5 quarantined), resource count. Device resource (6): resource kind, host physical base, length, guest aperture offset, attributes. Addresses never contain kernel virtual pointers. Device details read metadata without MMIO/config-space probes; host interrupt identities are not guest IRQs.
- Device assignment authority separates INSPECT (firmware discovery), ASSIGN_DEVICE (claims), and MAP_DMA (resident DMA extent disclosure). Claim and claim_matching return TRANSFER|DUPLICATE|INSPECT|ASSIGN_DEVICE; userspace bundles additionally grant READ|WRITE|WAIT|ACK_INTERRUPT. Assigning a physical device requires ASSIGN_DEVICE on that handle and WRITE on the pending VM. device_mmio checks READ for operation zero and WRITE for operation one after validating the operation selector; its unconditional schema rights are empty. MMIO reads may have hardware side effects and still require an active assignment with retained backing. IRQ pending requires WAIT; IRQ completion requires ACK_INTERRUPT. An inspection-only handle cannot claim, assign, access registers, acknowledge interrupts, or disclose VMO physical addresses.
- Device firmware inspection reads one immutable boot snapshot. Field PROPERTY_NAMES (5) returns every property name as a NUL-separated list, without granting ownership or exposing mapping capabilities; field PROPERTY (4) reads a named property's original bytes. Other fields report node identity, compatible strings, translated registers and interrupt metadata. Empty output queries the required byte length. The caller supplies a name only for PROPERTY; all other fields require an empty name. Inspection cannot substitute for an atomic device claim.
- Physical device profiles identify the admitted transport contract: virtio-mmio SCSI (1), userspace-managed registers (2), virtio-mmio network (3), and an exclusively assigned PCI function (4). Firmware matching combines profile with identity and rejects ambiguous matches. PCI identity kind 3 uses exactly nine lowercase ASCII bytes vvvv:dddd (vendor and device identifiers). Physical MMIO is mediated against exact resource extents; a device handle never grants a host MMIO page mapping. DEVICE_PROFILE_INFO reports the complete guest aperture, resource count, interrupt count, PCI device/vendor word and admitted DMA bus offset. Non-PCI profiles use a 64 KiB aperture and zero PCI identity and DMA offset. Virtio profiles have one interrupt; userspace profiles report zero or one physical interrupt. Non-PCI assignment still reserves one virtual interrupt when no physical interrupt is present. A pending VM admits at most DEVICE_ASSIGNMENT_MAX_DEVICES controllers with disjoint apertures and interrupt ranges; all share its DMA backing lifetime. Partial installation rolls back every controller, and retirement releases pages only after every controller is quiescent.
- The PCI-function profile presents one endpoint at guest BDF 00:00.0 through an 8 MiB aperture. Resource kind PCI_ECAM describes a virtual 1 MiB ECAM window. Interrupt delivery uses either PCI_MSI, a 4 KiB GICv2m frame, or PCI_INTX with interrupt_count one and zero offset, length, flags and bus_address; PCI_BAR0 through PCI_BAR0+5 describe implemented memory BARs. Resource offsets are relative to the assigned guest aperture; bus_address is the initial virtual PCI address of a BAR. MEMORY_64 and PREFETCHABLE flags apply only to BAR resources; other resources have zero flags and bus_address. PCI configuration and MSI-X are mediated: physical host-bridge, interrupt-controller and DMA-window registers are never guest resources. The interrupt argument selects the first of interrupt_count consecutive GIC SPIs (at most 64); the AArch64 reference GIC has 256 IDs. Linux owns the entire endpoint and its child drivers. DMA bus addresses equal the admitted host physical extent plus dma_bus_offset. The offset alone is not an IOMMU boundary. On the QEMU ECAM/SMMUv3 path the kernel binds requester IDs to the VM domain and maps only retained initial RAM and live grants before bus mastering; MMIO and BCM2712 assignments remain trusted. Firmware-initialized devices remain fail-closed if link or DMA translation validation fails. Clearing bus mastering alone is not proof of DMA retirement; a PCI assignment without a proven reset/quiescence protocol retains its DMA backing and physical claim on teardown.
- system_config queries a public scalar system property by u64 key. PAGE_SIZE (1) returns the Native mapping granule in bytes, immutable for the lifetime of a process. Unknown keys return NOT_SUPPORTED. Unused argument registers must be zero. Success returns the value in value0 and zero in value1. This query requires no inspector capability and does not expose privileged observations.
- vmar_allocate(parent, address, size, options) uses address as the low end. Options zero treats nonzero address as a hint, choosing the nearest fitting free interval with lower-base tie breaking; address zero selects the lowest free interval. VMAR_ALLOCATE_EXACT requires the exact address, including zero. Unknown flags are invalid. Size is nonzero, addresses and size page aligned, and intervals must not overflow. Success returns child in value0 and actual base in value1. Selection and reservation commit atomically within parent authority. No fitting free interval returns NO_MEMORY.
- system_config key SYSTEM_CONFIG_APPLICATION_ADDRESS_LIMIT returns the HAL application-exclusive address limit in value0 and zero in value1. It describes profile geometry, not VMAR authority. The kernel grants ROOT_VMAR from one page to that limit.
- Every Native process enters a caller-selected loader. Process-builder create derives the executable File into the child using explicit READ|EXECUTE|PROTECT|DUPLICATE|TRANSFER authority, and borrows the loader with READ|EXECUTE. The target is not frozen by builder creation. The kernel takes only a temporary stable loader image while preparing its restricted bootstrap mappings. Seal validates and maps only the loader's restricted Native ET_DYN bootstrap image and an empty guarded stack; the first argument register holds a ByteChannel handle. Only initial boot selects a loader by pathname: /lib/userspace-loader-hyper-&lt;architecture&gt; launches /init from the ramdisk root. The queued message contains loader_startup, handle_count startup_handle records, then data_size opaque bytes. size is 96; flags permits only loader_startup_reply. execution_context, object_factory and system_info are process-installed capabilities for current execution, local object allocation and public system queries. Executable is an ordinary File with READ|EXECUTE|PROTECT. The user loader acquires content protection before querying size or parsing ELF. root_vmar and stack_vmar identify child-owned mapping authority; stack_base and stack_size describe mapped usable stack bytes with an unmapped guard at each end. loader_base and loader_size describe the loader mappings. Library directories are ordinary explicitly supplied startup capabilities; the kernel neither selects nor opens them. No argv, environment, auxv, application ELF parsing or relocation belongs to this kernel entry contract. The selected runtime owns bootstrap-handle closure and mapping retirement. Closing a VMAR handle alone does not unmap or destroy its reservation.
- Virtual CPU set_affinity borrows a VirtualCpu with WRITE and accepts the same nonempty little-endian u64 CPU-mask word array as process-builder affinity. Bits above the kernel CPU limit are rejected; the mask must contain a schedulable CPU. The affinity update retains the current assignment when allowed; otherwise the scheduler selects an allowed registered CPU and safely migrates the saved context. Automatic Fair balancing may later move a Ready vCPU within this allowed set; a singleton mask prevents automatic movement. OK means the update or required handoff was accepted; migration may complete before or after return. An overlapping incompatible handoff or CPU-local wait returns busy. VM stop or guest power operations may supersede the request. Stopping, detached, and reaped endpoints reject new affinity updates. VirtualCpu info extends its 24-byte prefix with host_cpu and migration_target at offsets 24 and 28, one scheduler placement observation rather than running state or a completion token. Value 0xffffffff means unavailable assignment or no pending migration; older kernels return only the original prefix.
- Guest mapping creation borrows backend VirtualMachine WRITE and GuestMemory MAP, populates the full stable grant, and returns an owned mapping handle plus a backend-VM-local non-reused token. The backend IPA envelope must contain all sparse aliases: alias = 64 GiB + host physical address, with host addresses below 256 GiB. Unadmitted envelope holes have no RAM or page bitmap entries. The kernel constructs immutable frontend-ordered, coalesced physical extents; neither Native policy nor Linux may replace their addresses. Backend notification write64 0x28 admits one token exactly once to that unique notification route; read64 0x30/0x38/0x40/0x48 return frontend base, total bytes, status, extent count. Write64 0x50 selects an extent index; read64 0x58/0x60/0x68/0x70 return alias, frontend-relative offset, byte length, and query status. Rejection clears the old reply. Extents cover the entire frontend range without holes or overlapping physical pages. After draining vhost and releasing all page pins, only that backend route may write64 the token at 0x20 to certify quiescence. Mapping release accepts never-admitted or quiescent tokens, withdraws lookup, clears leaves, waits for every CPU translation acknowledgement and, for an IOMMU-bound backend, completed DMA invalidation, then releases backing. Closing a mapping handle alone quarantines its pages until backend VM retirement. Notification control operation 3 permanently disconnects both routes, only when no admitted mapping remains; a disconnected route never affects an address or IRQ reused by a subsequent connection.
- Native block creation dedicates one fully resident 1048576-byte guest-memory grant permanently to one kernel virtio-scsi initiator; repeated creation from that grant fails. The backend VM must retain the matching mapping until execution and physical DMA are quiescent. Six standard split queues (control, event, and four request queues) have size 8, descriptor offsets n*4096, available offsets n*4096+256, and used offsets n*4096+512, relative to guest_base. Userspace negotiates VERSION_1 with the backend before activation. Activation enables notifications and issues READ CAPACITY(16), accepting only 512-byte sectors; capacity is never supplied by userspace. The block capability remains the data-plane owner across filesystem worker handoff; a clean unmount returns the same activated object rather than creating or activating a replacement. Reads use bounded batches across the four request queues; writes and flushes remain ordered. Requests use shared queues and blocking kernel notifications, not per-request userspace RPC. Unretired request failures permanently disconnect the initiator and never recycle its buffer. PEER_CLOSED reports disconnection. The physical pages remain retained by backend mappings until VM/DMA retirement; notification closure alone never proves DMA quiescence.
- filesystem_mount borrows a READ|WRITE|WAIT ByteChannel, a READ|WRITE|MAP shared VMO and a WRITE|EXECUTE Directory. The kernel retains the transport and buffer, validates the worker root, and publishes a fresh mount and filesystem identity at the capability-relative path. The VMO must satisfy the published filesystem protocol capacity and pinning contract. read_only is exactly zero or one; read-only mounts reject mutating protocol operations in the kernel independently of worker policy. Media discovery, format selection and exclusive NativeBlock ownership belong to the userspace filesystem manager and format worker, not this syscall.
- filesystem_mount_ramfs and filesystem_switch_root require MANAGE_NAMESPACE|WRITE|EXECUTE on the live namespace-root Directory anchor. Ordinary directory opens never confer MANAGE_NAMESPACE; only explicit capability duplication, delegation or root-anchor scope creation preserves it. Ramfs mount does not create or configure block filesystems. switch_root copies its request, at most 16 mount moves and every bounded UTF-8 path before namespace mutation; zero moves requires a null moves pointer. new_root and move sources resolve in the old namespace, while put_old and move targets resolve inside new_root. The root and all requested moves publish atomically. Existing non-anchor Directory and File capabilities retain their opened locations; existing root anchors follow the published root. A rejected transaction preserves all mount edges and the current root. Paths are at most 4096 bytes, nonempty and contain no NUL bytes. Layout, boot stages and the choice of put_old are userspace policy.
- filesystem_unmount requires Directory WRITE|EXECUTE and an exact mounted-root path. The namespace root cannot be detached. Live File, Directory, cwd, path-resolution or nested-mount uses return busy. Admission closes before synchronous backend flush and namespace publication; no mount-table spinlock is held across backend IPC. A failed flush or concurrent topology change leaves the mount attached. Successful detach permanently retires its transport health and content identity, so retained snapshots or speculative cache work cannot revive the old mount. Userspace then stops and joins the worker before reusing the returned NativeBlock; the syscall itself does not terminate processes or release backend DMA mappings.
- VM power control requires WRITE authority. virtual_machine_get_power_request returns a non-consuming snapshot of one pending request, or would_block when none exists. Request IDs identify one completion and stale IDs are rejected. Operations are CPU_ON (1), CPU_OFF (2), SYSTEM_OFF (3), and SYSTEM_RESET (4); accept is strictly 0 or 1. Reserved output is zero. POWER_REQUEST signals pending work, and VCPU_TERMINATED prompts inspection of per-vCPU terminal state. virtual_machine_open_vcpu returns a control handle for an already configured vCPU; it does not change the immutable topology. The runtime owns lifecycle policy; no guest request is forwarded to host firmware. Guest suspend operations remain unsupported.
- PROTECT grants authority to acquire content protection, not READ, WRITE or EXECUTE. Directory/file derivation can only attenuate this right. file_protect returns a process-local nonduplicable, nontransferable FileProtection with MAP|INSPECT and the size observed under protection. Protection rejects writes and resize through every alias but does not freeze names or other metadata. Closing the token releases its reference; protected file mappings retain separate references until unmap or owner-process teardown. After all owner threads stop and its handles close, teardown detaches protected sources without allocation. External VMAR owners can retain the dead address space and its charged pages, but not protection or permission to refill from the file. vmar_map_file requires File READ, additionally EXECUTE for executable mappings, FileProtection MAP for that same canonical file, and MAP on a VMAR of the calling process. It uses private_mapping range/zero-fill semantics. COPY_ON_WRITE permits clean pages to retire after translation invalidation and fault back from the protected file. Both file mapping modes populate clean pageable contents; EAGER does not grant residency. Full clean pages share canonical cache storage, while partial pages and private writes remain isolated. LOCK_MEMORY on a process-owned VMAR authorizes vmar_lock_memory/ vmar_unlock_memory. Range calls require page-aligned nonempty ranges; VMAR_MEMORY_CURRENT selects only currently mapped pages, including descendants, with address and size zero. Lock success populates the range and charges PinnedPages to the requester; repeated lock is idempotent. Unmap and process teardown release locks; future mappings are not locked. Shared pages remain cached while a mapping retains them. ProcessBuilder memory-lock delegation requires TaskFactory LOCK_MEMORY and explicit enablement before seal. Both modes retain protection until unmap or owner-process teardown; private modified pages cannot be discarded without another backing store. File snapshot VMOs are not exported to userspace. Explicit mapping and user-buffer copy syscalls report allocation, quota or I/O failures. Hardware refaults have no syscall result; when bounded reclaim/refill retries fail, the kernel terminates the Process as an unhandled page fault. Reclaim is best effort and page-table withdrawal may require temporary transaction resources before releasing pages.
- Immutable VMO snapshots grant READ and MAP but never WRITE. vmo_create_snapshot coherently copies its source and reports Busy while writable mappings or hardware writers exist. Private mappings require source READ|MAP and VMAR MAP: source_offset is page aligned, data_offset is less than one page, source_length bytes starting at source_offset+data_offset initialize destination data_offset; all other destination bytes are zero. Writable private views never permit EXECUTE. Executable private views additionally require an executable source VMO and source EXECUTE rights; sanitized pages are sealed and instruction-published before mapping, with no writable alias. Copy-on-write mode shares immutable full pages until first write; eager mode allocates all private pages before publication. Private writes, including kernel copyout and atomic waits, cannot change the source or another private view. Ordinary writable/shared VMOs and exclusive hardware leases keep their stable backing contract. Snapshot bytes and old page versions remain owned through acknowledged translation retirement.
- VM platform inspection borrows an INSPECT creation lease without consuming it. Metadata describes the selected local platform: counter_frequency_hz is the actual guest counter frequency, and riscv_isa is a guaranteed subset across all admitted CPUs, not a complete host ISA listing. Other architectures return a zero RISC-V mask. The guarantee remains valid across local CPU migration; cross-machine migration is not implied.
- WaitSets are process-local, non-transferable objects with capacity 1..1024. BIND_WAIT authorizes add/rearm/remove, WAIT authorizes consumption. Add requires source WAIT and reserves one event slot; WaitSet and CapabilityChannel sources are unsupported. Registration IDs are globally non-reused. Bind/rearm observe signal levels and sequence under the source lock; one-shot publication does not allocate. Rearm is busy until successful event consumption. Wait returns one exact 24-byte record (registration ID, signal bits, sequence), using an absolute monotonic deadline. Copyout failure restores the event unless removal or closure cancelled it. Source handle close does not cancel object-lifetime subscriptions; final set handle close detaches registrations and wakes consumers. Future CapabilityChannel subscriptions require ownership-epoch invalidation.
- ByteChannel duplicate authority permits shared endpoint ownership; peer_closed is published only when the last active endpoint handle closes. Internal operation pins, including WaitSet subscriptions, do not retain active endpoint authority.
- process_get_current_id returns the calling Process KOID for observation only. It creates no handle or operational authority and is not a PID-to-handle lookup.
- Handles are opaque nonzero process-local u64 values with bits 63..56 fixed to 0x0F. Zero is the absent/invalid handle sentinel; all other high-byte values are reserved and rejected as handles. Invocations return a separate signed status and unsigned result words. The low 56 bits are masked table-local data: callers must not infer slot/generation fields, allocation order, or numeric relationships. Slot reuse advances a wrapping generation without permanently retiring capacity; numeric values may repeat. Using a closed, replaced, or moved handle is a caller error and may operate on a later entry with that value. Inspector and retained crash records report the exact value held by the observed Process; queries use that Process namespace. Transfers publish destination-local values. Numeric values alone never grant authority outside the owning Process.
- KOIDs encode a nonzero 32-bit slot in bits 0..31 and a 32-bit generation in bits 32..63. A slot remains unique until final payload destruction, after which reuse advances its generation, wrapping to one at the maximum. Live objects have distinct IDs, but complete values may repeat after destruction. Diagnostic queries describe the current object and do not establish identity across destruction and reuse. A language runtime with a 32-bit process ID surface may expose the KOID slot, accepting reuse after object destruction. Neither form grants authority or permits identifier-to-handle conversion; KOIDs do not encode creation order. Kernel operations retain direct counted object references rather than recovering authority from historical IDs.
- Ramfs storage and namespace policy remain kernel-owned; disk filesystem parsing runs in userspace. Open File and Directory objects pin their nodes after unlink. Directory WRITE permits namespace mutation; File WRITE permits content mutation. directory_create_file exclusively creates a regular file and publishes its requested handle atomically with the new name. Mode accepts permission bits 0777. directory_remove options 0 removes a non-directory entry without following the final symlink; 1 removes an empty directory. file_write_at options 0 uses offset, 1 atomically appends (offset must be zero); returns actual bytes and end offset. Transfers may complete short. file_resize zero-fills extension. Active file protections reject content changes through every File alias.
- Thread creation prepares a dormant thread in the calling Process and returns duplicate, wait, inspect, start and request_stop authority. Entry, 16-byte-aligned stack, TLS and an opaque first argument define its initial context. Affinity pointer/count zero together inherit the calling Thread current allowed CPU set; otherwise they name one to four little-endian u64 bitmap words. An explicit mask must be nonempty, fit supported CPU indices and include at least one schedulable CPU. The child remains dormant until start, with affinity already applied. The caller retains stack/TLS ownership through the thread terminated signal. Start publishes runnable execution; request_stop abandons execution without language destructors. Closing the last handle to a dormant thread cancels it; running threads continue independently of handle ownership. Existing thread_exit terminates only the caller, while process_exit stops every thread.
- Atomic wait/wake currently operate on aligned writable u32 words in the calling Process. Wait checks the value and publishes its generation atomically with respect to wake; mismatch returns ok, and spurious wakes are allowed. Wait uses an absolute monotonic nanosecond deadline, with UINT64_MAX meaning infinite; timeout returns timed_out and cancellation returns cancelled. Wake returns the number of registrations actually notified, bounded by count. Wake does not replace the caller's release operation or the waiter's acquire predicate check. Keys include the non-reused mapping identity and virtual address; aliases and other Processes have separate wait domains. Keep the mapping live while waiting: unmap/protect does not move an existing wait to a replacement mapping. Pinned backing survives concurrent unmap until admitted accesses finish; process stop cancels outstanding waits.
- Thread sleep accepts an absolute monotonic nanosecond deadline and returns ok when it expires, including an already elapsed deadline. UINT64_MAX sleeps until cancellation. It parks the current thread through scheduler timeout arbitration rather than polling or yielding.
- The monotonic clock operation requires SystemInfo READ and returns absolute nanoseconds from the kernel's monotonic clock domain. The read-only capability confers no mutable clock authority.
- Extensible input records carry their caller size in the syscall's explicit byte-count argument. The size must be at least the record's published minimum prefix and no greater than extensible_record_max_bytes; missing bytes through the kernel's current record size default to zero, while bytes beyond that size must be zero. Extensible information outputs accept any capacity from the record's minimum prefix through extensible_record_max_bytes, write only the intersection of caller capacity and kernel-supported size, and return the kernel-supported size in value0 on ok. Bytes beyond that intersection remain untouched.
- Directory lookup is capability-relative. A leading slash restarts at that Directory's traversal root, and parent components cannot escape it. Every open requires read traversal authority, and every requested File right must already be present on the source Directory before the result is further bounded by the File object's node-specific ceiling.
- directory_begin_read requires only Directory READ and creates an independent DirectoryStream. Its grant is the source Directory grant intersected with READ|DUPLICATE|TRANSFER|INSPECT, so attenuated propagation or inspection authority is never restored. Each stream retains its opened namespace location and small trusted backend position, never directory cache pages. Duplicated or transferred stream handles share progress. directory_read now requires DirectoryStream READ and the exact published page capacity. Cookie zero reads the first page; subsequent cookies must have been returned by that stream. Nonzero cookies are globally monotonic and never reused. Only the most recent input cookie can be replayed for copyout retry; older, forged or foreign-stream cookies are invalid. A zero next_cookie marks EOF; short nonempty pages may still have a continuation. Every name is one valid path component encoded as name_length UTF-8 bytes followed by zero-filled capacity. Remote mutations of the scanned directory, or renames of that directory or an ancestor, invalidate its ongoing scans and require a new stream; mutations of unrelated directories and file-content changes preserve traversal. Ramfs enumeration remains weakly consistent across mutations. No scan is an immutable directory snapshot. Callers must not interpret or synthesize cookies. Closing the final stream handle releases the position and namespace pin.
- Native task and object inspectors are immutable capability-scoped views. Process, thread, and object KOIDs plus scan cursors are observation-only values and can never be exchanged for operational authority. Out-of-scope targeted lookup returns not_found.
- Task inspector records carry a bounded UTF-8 name as name_length bytes followed by zero-filled capacity. Process names are the immutable labels committed by ProcessBuilder publication; Thread names are immutable scheduler identity labels retained through the retiring registry phase.
- Inspector derivation is monotonic: a derived Process, TaskGroup, or ResourceDomain view cannot widen its parent's task scope, object scope, visibility, or rights. Derivation requires DUPLICATE | TRANSFER | INSPECT | DERIVE and returns exactly that base set; INSPECT_DETAILS is never added or preserved by the existing derivation calls. Callers attenuate the returned handle before delegation. Native task operations remain handle-based; numeric PID and TID namespaces belong exclusively to compatibility personalities.
- process_derive_observer requires WAIT | TRANSFER | REQUEST_STOP on a Process and returns a new WAIT | TRANSFER handle for the same KOID. It never copies supervisor stop or inspect authority and the derived observer cannot itself derive further observers. General handle duplication still requires DUPLICATE.
- task_group_request_stop closes new membership admission and requests asynchronous member stop. TASK_GROUP_TERMINATED is sticky and becomes observable only after pending construction reservations have completed or rolled back and every member Process has completed retirement. It covers this flat TaskGroup only, not processes attached to other groups.
- console_discard_input requires READ and atomically discards the currently accepted physical Console input prefix. It returns the byte count; input arriving later remains queued. An outstanding transactional read claim returns would_block without discarding any bytes. This operation has no terminal or character interpretation.
- Every live TaskGroup handle participates in shared group-lifetime ownership regardless of its attenuated rights. Closing the last TaskGroup handle asynchronously requests stop for every member. Rights control operations available through a handle; they do not change this ownership effect.
- Inspector scans require the exact published page capacity for their record type. Cursor zero starts a scan and a returned next_cursor of zero ends it. Pages and complete scans are weakly consistent with concurrent task, object, and handle-table mutation; generation-qualified handle values prevent slot reuse from aliasing an earlier observation.
- Memory and CPU inspectors publish immutable point-in-time copies. Their handles grant observation only; they never expose writable accounting storage or allocator and scheduler synchronization to userspace. CPU categories and thread runtime_ns are elapsed monotonic nanoseconds, including a snapshot-time charge for executing Threads. reserved_clock is zero. A multi-CPU snapshot is weakly consistent across CPUs.
- File and Directory information reports attribute snapshots plus filesystem, mount, and node identities for diagnostics and correlation. These identities do not grant authority, cannot be resolved back into handles, and do not define a pathname; directory entries, hard links, renames, mount namespaces, and unlinks make pathnames namespace-dependent observations rather than object identity.
- Object wait-many borrows every input handle for the complete wait, canonicalizes duplicate object identities, and selects the lowest input index whose requested mask intersects the winning object's committed level snapshot. Source-handle close after resolution does not cancel the wait.
- Process terminal detail fields are reason-specific: exit reasons encode the signed status as two's-complement in detail0; fault encodes class in detail0 and code in detail1; task-group stop encodes generation in detail0; unused details are zero.
- process_builder_retain requires WRITE | RETAIN_PROCESS on an unsealed ProcessBuilder. Only a TaskFactory carrying RETAIN_PROCESS can produce that builder authority. Registration is external and precedes process start; a running process cannot register itself. The returned ProcessRetirement is a nonduplicable rendezvous-only controller. It may be transferred without creating a buffered owner. Closing its last active handle, explicit release, builder abort, or its monotonic timeout revokes all CrashView derivatives. Ordinary Process observers never extend retention. No kernel callback or application crash handler is installed.
- Process stop closes admission and withdraws live handles while outstanding transactions cancel or finish safely. After execution quiesces, retained evidence contains register copies, numeric handle records and bounded resident private pages. It contains no live handle authority, device lease, file protection, filesystem refill source or target address space. Shared writable and clean file-backed pages are excluded; unavailable bytes return NOT_FOUND rather than faulting the target. Independently retained device and DMA owners must still finish their normal physical retirement.
- ProcessRetirement READY publishes a bounded scene; RELEASED is sticky and may occur before READY. The timeout starts on the first stop request, including quiescence time. Current limits are 64 armed registrations, four retained scenes and 64 MiB of retained page payload per registering supervisor, with at most 60 seconds per registration. Actual limits may tighten and quota/allocation failure is observable. At most 32 thread records, 256 numeric handle records and 1024 mapping records are retained; truncation is flagged. Bounded metadata is separately resource-domain charged. Controllers should be explicitly released after observation and before joining final process or group retirement.
- CrashView grants only read/inspect and may be duplicated or delegated. Release revokes new reads even when views remain open; already admitted bounded reads may finish. Read copies at most 64 KiB and may return a contiguous short prefix before an unavailable page. Indexed thread, mapping and handle queries begin at zero and return next_index zero at the end; absent indices return NOT_FOUND. Thread machine is ELF e_machine; general, vector and fp_control byte arrays encode little-endian u64 words in architecture register order. Mapping object_koid zero means no canonical object identity is exported. Numeric addresses and KOIDs confer no authority.
- Object transfer classes constrain generic capability transports. General objects may be retained by buffered or rendezvous transports. Rendezvous-only objects may move or duplicate only by a direct source-to-destination commit which never creates an in-transit owner. Forbidden objects cannot cross a userspace handle table boundary.
- AtomicOnOk capability transactions commit handle-table ownership, the rendezvous message, and live output-handle installation together only when both participants return ok. Every non-ok status preserves all input owners and the message. Non-fault failures leave output memory unchanged; fault may partially modify output byte or slot memory, but no handle value written by a failed call is live or installed. Bindings must ignore every output-memory byte after any non-ok status.
- A capability-channel receiver advertises peer_receiving only after its byte range, typed capability slots, and destination handle-table capacity are validated, reserved, and fully published on the endpoint's FIFO receiver queue. The signal is level-triggered but may race another sender.
- A capability disposition's rights are the sender's offered ceiling; capability_disposition_same_rights offers all source rights. A receive slot's rights are the exact installed grant, so receiver rights must be a subset of the sender offer and the offer a subset of source rights. Sender and receiver expected_kind values are nonzero and must both equal the actual object kind.
- A capability receive slot enters with handle and flags zero. On ok, only handle changes. Implementations reserve destination entries, prewrite their future handle values, then perform an infallible handle-table commit. Buffer-too-small changes no slot or byte memory and reports both required counts.
- Capability disposition move consumes its source only on ok and requires transfer. Capability disposition duplicate retains its source and requires transfer plus duplicate. Unknown operations, repeated source handles, kind mismatches, and rights violations reject the complete transaction.
- Capability-channel try_send returns peer_closed when no peer remains; otherwise it returns would_block only when no receiver is queued. It FIFO-matches the oldest fully published receiver. A size, kind, rights, operation, or copy mismatch rejects that transaction for both participants without scanning later receivers.
- Capability-channel timeout, cancellation, and peer close can win only before a receiver is matched. Once matched, sender completion or rejection owns the transaction through commit, including races with the deadline, cancellation, or close; both participants observe the same committed outcome.
- Process-builder create borrows its authority handles, retains the target File identity for an attenuated child grant, and captures a kernel-only stable copy of the selected bootstrap loader. Closing or replacing either source handle does not change the captured identities. The target File contents may change until the userspace loader acquires PROTECT; builder creation does not freeze them. Failure before builder publication releases these owners without consuming either input File. Builders are mutable only before seal. Every successful mutator applies completely; every failure leaves the builder unchanged. Seal is irreversible, prepares only the bootstrap process image and releases the temporary loader copy. Start requires a sealed builder, consumes it atomically, and returns a supervisor Process plus a READ/WAIT startup ByteChannel. It commits child publication, not successful application loading. Invocation failure preserves the builder; asynchronous bootstrap failure does not restore it. The SDK waits for its loader result protocol and stops failed children before returning a committed launch error.
- A process-builder name is nonempty UTF-8 without embedded NUL bytes. set_data replaces bounded opaque bytes, up to process_startup_data_max_bytes, without kernel interpretation. Arguments, environment, their limits and the loader result protocol are SDK policy. Boot init receives an empty payload.
- Process-builder set_name, set_data and set_affinity replace their prior values. Affinity is a nonempty little-endian array of u64 CPU-mask words. Bits above process_affinity_max_cpus and bits which cannot designate an allowed CPU are rejected.
- Process-builder add_handle requires a nonzero purpose unique within the builder, an expected nonzero exact object kind, and either exact granted rights or capability_disposition_same_rights. Move consumes the source only when the mutator returns ok; duplicate retains it and additionally requires duplicate. Failure preserves both builder and source.
- A VirtualMachineCreationAuthority may derive one resource-domain-bound VirtualMachineCreationLease. The lease is single-use and is consumed only when VirtualMachine creation publishes a PendingVirtualMachine handle successfully.
- Virtual-machine info appends resident_memory_bytes after the original 32-byte prefix. UINT64_MAX means unavailable during retirement. It counts currently allocated primary VMO backing, including RAM, explicitly admitted shared pools, and uploaded pages not yet mapped by stage-2; overlapping ranges of the same VMO count once. Dynamic backend alias windows, translation tables, and Native runtime memory are excluded. Shared pages are attributed independently in each guest and are not additive across guests. Inspection samples bounded page chunks and is not a globally atomic memory snapshot.
- A PendingVirtualMachine is mutable until seal. It must own at least one explicitly backed, non-overlapping writable region inside the configured IPA envelope and one bootstrap record for boot vCPU 0. Bootstrap entry and stack must lie in admitted backing; sparse envelope gaps remain unbacked and never become anonymous RAM. The configured vcpu_count fixes immutable topology; architecture power-on protocols supply secondary-vCPU runtime entry state, and virtual_machine_open_vcpu exposes their control handles. A guest serial route is optional and exists only when a caller transfers a VirtualSerial handle with assign-device authority before seal. Successful binding consumes the supplied handle and commits a VM-owned reference until VM retirement; a rejected binding leaves the handle unchanged. Guest output is published to a read-only shared VMO consumed by the owning runtime, and VM retirement disconnects the input route without invalidating existing output mappings. Seal is irreversible; install consumes the pending handle only on ok and publishes the installed VirtualMachine and dormant boot VirtualCpu handles together. VirtualCpu start is a separate operation after handle publication. The started VirtualCpu phase means that start committed successfully; it is not an observation that the scheduler currently considers the vCPU runnable or executing. AArch64 reference guests accept 1..8 vCPUs; RISC-V reference guests currently accept one.
- VirtualSerial handles may move through capability rendezvous or staged startup; buffered transfer is forbidden. Device assignment consumes a same-process handle. A runtime handoff transfers the serial consumer together with its output VMO and absolute consumed cursor, without closing the last active handle or re-registering the output buffer. register_output borrows a caller-allocated writable VMO of exactly 69632 bytes and registers it once before assignment. An exclusive write lease rejects existing writable mappings, direct accesses, snapshots, and further writers until port retirement; read-only mappings may coexist. Offset 0 is an atomic u64 produced count, offset 8 a saturating dropped-byte count, and offset 4096 begins 65536 atomic byte slots. Registration initializes counters; callers must not access contents during registration. The producer release-publishes bytes; the runtime acquire-loads production and reads a batch. acknowledge_output requires READ and submits the absolute consumed position after reading. Regressing or future positions return invalid_argument without mutation. Publication, acknowledgement, and closure serialize on the port: READABLE means unacknowledged output, WRITABLE means a connected input route has queue space, and PEER_CLOSED means no future output or input. WAIT authorizes object waits and WaitSet subscriptions. Acknowledgement clears READABLE only when caught up; new output reasserts it, without lost wakeups or periodic polling. Full output never overwrites unconsumed slots. The AArch64 SBSA UART retains pending bytes in its TX FIFO and exposes TXFF until runtime acknowledgement frees transport space; other producers discard new bytes and count loss when full. Counters never wrap. Last active handle closure synchronizes with writers and closes publication; registered pages remain pinned through final VM/object retirement. The SDK maps output read-only and acknowledges batches by syscall without copying payload through the syscall. Write remains nonblocking input injection: busy means the queue is full, bad_state means disconnected. Runtime policy owns retention and client transport.
- The creating process retains its guest VMO handle, but attaching it to a PendingVirtualMachine acquires exclusive hardware-write ownership and rejects any active Native writable mapping or direct VMO operation. Direct VMO access, snapshots, and writable Native mappings remain closed until VM retirement removes and invalidates every stage-2 mapping and releases the independent backing reference; read-only Native mappings may coexist.
- guest_memory_transfer copies 1..4096 bytes within a GuestMemory grant; operation 0 requires READ and copies into the caller, operation 1 requires WRITE and copies from the caller. The grant retains hardware-owned backing independently of any VMO handle or VM lifetime. The original VMO still forbids direct writes, snapshots and writable Native mappings. Payload copies are not snapshots or atomic word operations; a failed write may have partial effects. Native payload operations serialize without blocking on one another (busy if occupied), and dedication to a NativeBlock initiator permanently rejects these operations even through retained grants.
- guest_memory_scalar uses the same READ/WRITE authority and dedication exclusion: operation 0 acquire-loads an aligned 1/2/4/8-byte word (value must be zero), operation 1 release-stores a width-fitting word and executes a full memory fence before return. Offset and width must fit the grant. The store fence orders queue publication before a later suppression-flags read on weakly ordered CPUs. No pointer or mapping is exported; unsupported widths and overflow are rejected.
- guest_interrupt_create borrows VirtualMachine WRITE and exclusively reserves an AArch64 device interrupt line, excluding built-in and physically assigned interrupts and all existing notification sources. It returns a GuestInterrupt with WRITE, TRANSFER and INSPECT. Creation is allowed while installed or running, serialized with VM stop. guest_interrupt_set_level accepts only 0 or 1 and fails once the VM begins retirement or the source closes. Closing the final active handle deasserts the line; its reservation persists until VM retirement. The source implements no UART, PCI or virtio device policy.
- guest_notification_access requires frontend VirtualMachine WRITE and addresses an existing connection by original_base. Operation 0 reads pending interrupt status without acknowledgement, 1 reads and acknowledges that status atomically, and 2 submits a queue-index notification (value &lt; 32). For operations 0 and 1 value must be zero; unknown operations are rejected. It shares the direct MMIO source state and serialization. PCI bus-master disable suppresses notifications; PCI memory-decode disable does not suppress this explicit transport fallback. Closure rejects accesses. No backend identity, control operation, DMA lease or epoch can be changed through this call.
- guest_notification_configure requires frontend VirtualMachine WRITE. original_base identifies an existing connection by its creation address; base is a page-aligned 4 KiB window wholly contained in an installed userspace MMIO reservation and, while decoding is enabled, may not overlap another enabled I/O route. Flags 0 select the original virtio-MMIO register layout; bit 0 selects modern virtio-PCI, bit 1 enables memory decoding, bit 2 masks INTx, and bit 3 enables queue notifications (bus master). Unknown flags or PCI flags without bit 0 are rejected. PCI uses a 16-bit queue-index write at BAR+0x50 and a read-to-clear 8-bit ISR at BAR+0x60. Relocation, decode, and IRQ-mask changes serialize with source state and VM retirement. The operation cannot change the admitted backend, IRQs, DMA ownership, or epoch; disabling decode is not proof of backend quiescence.
- cpu_inspector_trace_start requires CpuInspector SET_ATTRIBUTES and an initialized trace_config. It starts one global generation, rejects an active/stopping capture, and replaces a stopped capture. Categories are bit flags 1 scheduler, 2 IPC, 4 Native syscall, 8 block I/O, 16 memory faults, 32 IRQ, 64 VM; zero and unknown bits fail. Duration is 1..300000 ms; capacity is 64..4096 retained events per CPU. cpu_mask is four little-endian u64 words; zero selects all online CPUs, and offline bits fail. The kernel closes capture at the deadline through an ordinary callout, independent of the collector lifetime; execution can be later if that worker is delayed. Feature-disabled kernels return NOT_SUPPORTED.
- cpu_inspector_trace_stop requires SET_ATTRIBUTES and the exact generation. It closes producer admission before joining short in-flight writers; BUSY requires retry. Repeated successful stop is idempotent until another capture starts. Generation is an observation/control token, not a capability or private capture owner. Any appropriately authorized inspector may control the global session.
- cpu_inspector_trace_read requires INSPECT_DETAILS. CPU UINT64_MAX selects trace_info (160 bytes, generation and cursor zero). CPU UINT64_MAX-1 selects trace_thread (72 bytes, exact generation, cursor is scheduler ThreadId); names are live best-effort observations, not authority. Ordinary online CPU indices select trace_batch (2656 bytes, eight trace_record entries) with exact generation and a per-CPU cursor. Returned value0 is the exact written size. next advances across overwritten and returned records; count zero while running is not EOF. incomplete is sticky per generation/CPU and means unquantified producer loss. Stop freezes records and loss metadata until next start. Strings are UTF-8 with NUL padding, truncated to fixed bounds; all reserved/unused bytes are zero. No kernel pointer, raw handle encoding or userspace payload is exported. Event vocabulary is diagnostic and identified by the included build revision, not a stable behavioral ABI.

## Invocations

Auxiliary result registers are defined only for `ok` unless a result is annotated with
`also-on=<status>`. Element-count memory ranges are checked as count times the declared
element size before any user-memory access.

### `abi_query`

Object-local method `abi_query`.

| Property | Contract |
| --- | --- |
| Receiver | `receiver: Borrow, kind=system_info, rights=0x10` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | — |
| Results | `revision: u64`, `features: u64` |
| Capability effects | `receiver: Borrow, kind=system_info, rights=0x10` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Abi` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `handle_close`

Object-local method `handle_close`.

| Property | Contract |
| --- | --- |
| Receiver | `handle: ConsumeOnCommit, any, rights=0x0` |
| Operation | 0 |
| Feature | `Core` |
| Arguments | — |
| Results | — |
| Capability effects | `handle: ConsumeOnCommit, any, rights=0x0` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `handle_duplicate`

Object-local method `handle_duplicate`.

| Property | Contract |
| --- | --- |
| Receiver | `source: Borrow, any, rights=0x1` |
| Operation | 1 |
| Feature | `Core` |
| Arguments | `requested_rights: rights` |
| Results | `handle: handle` |
| Capability effects | `source: Borrow, any, rights=0x1`, `handle: produce, same-as(source), subset-from(requested_rights)` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `handle_replace`

Object-local method `handle_replace`.

| Property | Contract |
| --- | --- |
| Receiver | `source: ConsumeOnCommit, any, rights=0x0` |
| Operation | 2 |
| Feature | `Core` |
| Arguments | `requested_rights: rights` |
| Results | `handle: handle` |
| Capability effects | `source: ConsumeOnCommit, any, rights=0x0`, `handle: produce, same-as(source), subset-from(requested_rights)` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `handle_get_info`

Object-local method `handle_get_info`.

| Property | Contract |
| --- | --- |
| Receiver | `handle: Borrow, any, rights=0x0` |
| Operation | 3 |
| Feature | `Core` |
| Arguments | `output: user_address`, `output_size: byte_count` |
| Results | `supported_size: byte_count` |
| Capability effects | `handle: Borrow, any, rights=0x0` |
| User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=handle_info; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `object_get_basic_info`

Object-local method `object_get_basic_info`.

| Property | Contract |
| --- | --- |
| Receiver | `handle: Borrow, any, rights=0x8` |
| Operation | 4 |
| Feature | `Core` |
| Arguments | `output: user_address`, `output_size: byte_count` |
| Results | `supported_size: byte_count` |
| Capability effects | `handle: Borrow, any, rights=0x8` |
| User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=object_basic_info; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Object` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `thread_yield`

Object-local method `thread_yield`.

| Property | Contract |
| --- | --- |
| Receiver | `receiver: Borrow, kind=execution_context, rights=0x20` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | — |
| Results | — |
| Capability effects | `receiver: Borrow, kind=execution_context, rights=0x20` |
| User memory | — |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Task` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `thread_exit`

Object-local method `thread_exit`.

| Property | Contract |
| --- | --- |
| Receiver | `receiver: Borrow, kind=execution_context, rights=0x800` |
| Operation | 257 |
| Feature | `Core` |
| Arguments | `status: i64` |
| Results | — |
| Capability effects | `receiver: Borrow, kind=execution_context, rights=0x800` |
| User memory | — |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=NoReturn, flags=None` |
| Audit | `Task` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `process_exit`

Object-local method `process_exit`.

| Property | Contract |
| --- | --- |
| Receiver | `receiver: Borrow, kind=execution_context, rights=0x800` |
| Operation | 258 |
| Feature | `Core` |
| Arguments | `status: i64` |
| Results | — |
| Capability effects | `receiver: Borrow, kind=execution_context, rights=0x800` |
| User memory | — |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=NoReturn, flags=None` |
| Audit | `Task` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `event_create`

Object-local method `event_create`.

| Property | Contract |
| --- | --- |
| Receiver | `receiver: Borrow, kind=object_factory, rights=0x8000000000` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | `options: u32` |
| Results | `handle: handle` |
| Capability effects | `receiver: Borrow, kind=object_factory, rights=0x8000000000`, `handle: produce, kind=event, fixed=0x8000f` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Object` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `event_signal`

Object-local method `event_signal`.

| Property | Contract |
| --- | --- |
| Receiver | `event: Borrow, kind=event, rights=0x80000` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | `clear_mask: u64`, `set_mask: u64` |
| Results | — |
| Capability effects | `event: Borrow, kind=event, rights=0x80000` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Object` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `object_wait_one`

Object-local method `object_wait_one`.

| Property | Contract |
| --- | --- |
| Receiver | `object: Borrow, any, rights=0x4` |
| Operation | 5 |
| Feature | `Core` |
| Arguments | `signals: u64`, `deadline: u64` |
| Results | `observed: u64` |
| Capability effects | `object: Borrow, any, rights=0x4` |
| User memory | — |
| Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` |
| Audit | `Object` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `byte_channel_create`

Object-local method `byte_channel_create`.

| Property | Contract |
| --- | --- |
| Receiver | `receiver: Borrow, kind=object_factory, rights=0x8000000000` |
| Operation | 257 |
| Feature | `Core` |
| Arguments | `options: u32` |
| Results | `endpoint0: handle`, `endpoint1: handle` |
| Capability effects | `receiver: Borrow, kind=object_factory, rights=0x8000000000`, `endpoint0: produce, kind=byte_channel, fixed=0x3f`, `endpoint1: produce, kind=byte_channel, fixed=0x3f` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Object` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `byte_channel_write`

Object-local method `byte_channel_write`.

| Property | Contract |
| --- | --- |
| Receiver | `endpoint: Borrow, kind=byte_channel, rights=0x20` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | `options: u32`, `bytes: user_address`, `byte_count: byte_count` |
| Results | — |
| Capability effects | `endpoint: Borrow, kind=byte_channel, rights=0x20` |
| User memory | `bytes: Read, len=byte_count bytes, max-bytes=65536; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `byte_channel_read`

Object-local method `byte_channel_read`.

| Property | Contract |
| --- | --- |
| Receiver | `endpoint: Borrow, kind=byte_channel, rights=0x10` |
| Operation | 257 |
| Feature | `Core` |
| Arguments | `options: u32`, `bytes: user_address`, `byte_capacity: byte_count` |
| Results | `actual_bytes: byte_count; also-on=buffer_too_small` |
| Capability effects | `endpoint: Borrow, kind=byte_channel, rights=0x10` |
| User memory | `bytes: Write, len=byte_capacity bytes, max-bytes=65536; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `console_read`

Object-local method `console_read`.

| Property | Contract |
| --- | --- |
| Receiver | `console: Borrow, kind=console, rights=0x10` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | `options: u32`, `bytes: user_address`, `byte_capacity: byte_count` |
| Results | `actual_bytes: byte_count; also-on=would_block` |
| Capability effects | `console: Borrow, kind=console, rights=0x10` |
| User memory | `bytes: Write, len=byte_capacity bytes, max-bytes=4096; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `console_write`

Object-local method `console_write`.

| Property | Contract |
| --- | --- |
| Receiver | `console: Borrow, kind=console, rights=0x20` |
| Operation | 257 |
| Feature | `Core` |
| Arguments | `options: u32`, `bytes: user_address`, `byte_count: byte_count` |
| Results | `actual_bytes: byte_count; also-on=would_block` |
| Capability effects | `console: Borrow, kind=console, rights=0x20` |
| User memory | `bytes: Read, len=byte_count bytes, max-bytes=4096; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `directory_open_file`

Object-local method `directory_open_file`.

| Property | Contract |
| --- | --- |
| Receiver | `directory: Borrow, kind=directory, rights=0x10` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | `path: user_address`, `path_size: byte_count`, `requested_rights: rights`, `options: u32` |
| Results | `file: handle` |
| Capability effects | `directory: Borrow, kind=directory, rights=0x10`, `file: produce, kind=file, exact-from(requested_rights), allowed=0x9800000bb, required-from=directory` |
| User memory | `path: Read, len=path_size bytes, max-bytes=4096; order=0` |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `file_read_at`

Object-local method `file_read_at`.

| Property | Contract |
| --- | --- |
| Receiver | `file: Borrow, kind=file, rights=0x10` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | `options: u32`, `offset: u64`, `output: user_address`, `output_capacity: byte_count` |
| Results | `actual_bytes: byte_count`, `file_size: byte_count` |
| Capability effects | `file: Borrow, kind=file, rights=0x10` |
| User memory | `output: Write, len=output_capacity bytes, max-bytes=2097152; order=0` |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `capability_channel_create`

Object-local method `capability_channel_create`.

| Property | Contract |
| --- | --- |
| Receiver | `receiver: Borrow, kind=object_factory, rights=0x8000000000` |
| Operation | 258 |
| Feature | `Core` |
| Arguments | `options: u32` |
| Results | `endpoint0: handle`, `endpoint1: handle` |
| Capability effects | `receiver: Borrow, kind=object_factory, rights=0x8000000000`, `endpoint0: produce, kind=capability_channel, fixed=0x3f`, `endpoint1: produce, kind=capability_channel, fixed=0x3f` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Object` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `capability_channel_try_send`

Object-local method `capability_channel_try_send`.

| Property | Contract |
| --- | --- |
| Receiver | `endpoint: Borrow, kind=capability_channel, rights=0x20` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | `options: u32`, `bytes: user_address`, `byte_count: byte_count`, `dispositions: user_address`, `disposition_count: element_count` |
| Results | — |
| Capability effects | `endpoint: Borrow, kind=capability_channel, rights=0x20` |
| User memory | `bytes: Read, len=byte_count bytes, max-bytes=4096; order=0`, `dispositions: Read, len=disposition_count elements, max-elements=16, element-size=24, record=capability_disposition, transactional-handles=(handle, rights, expected_kind, operation), common-rights=0x2, operations=[move=0:ConsumeOnCommit/rights=0x2+duplicate=1:Borrow/rights=0x3], commit=AtomicOnOk; order=1` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `capability_channel_receive`

Object-local method `capability_channel_receive`.

| Property | Contract |
| --- | --- |
| Receiver | `endpoint: Borrow, kind=capability_channel, rights=0x10` |
| Operation | 257 |
| Feature | `Core` |
| Arguments | `deadline: u64`, `bytes: user_address`, `byte_capacity: byte_count`, `capability_slots: user_address`, `slot_count: element_count` |
| Results | `actual_bytes: byte_count; also-on=buffer_too_small`, `actual_capabilities: element_count; also-on=buffer_too_small` |
| Capability effects | `endpoint: Borrow, kind=capability_channel, rights=0x10` |
| User memory | `bytes: Write, len=byte_capacity bytes, max-bytes=4096; order=0`, `capability_slots: ReadWrite, len=slot_count elements, max-elements=16, element-size=24, record=capability_receive_slot, typed-receive-slots=(handle, rights, expected_kind, flags), produce-transferred-handles, commit=AtomicOnOk; order=1` |
| Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `process_builder_create`

Object-local method `process_builder_create`.

| Property | Contract |
| --- | --- |
| Receiver | `factory: Borrow, kind=task_factory, rights=0x100000` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | `group: handle`, `domain: handle`, `executable: handle`, `loader: handle` |
| Results | `builder: handle` |
| Capability effects | `factory: Borrow, kind=task_factory, rights=0x100000`, `group: Borrow, kind=task_group, rights=0x4000000`, `domain: Borrow, kind=resource_domain, rights=0x8000000`, `executable: Borrow, kind=file, rights=0x800000093`, `loader: Borrow, kind=file, rights=0x90`, `builder: produce, kind=process_builder, fixed=0x10000000c2a, inherited-from(factory), mask=0xe3000000000` |
| User memory | — |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Task` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `process_builder_set_name`

Object-local method `process_builder_set_name`.

| Property | Contract |
| --- | --- |
| Receiver | `builder: Borrow, kind=process_builder, rights=0x20` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | `name: user_address`, `name_size: byte_count` |
| Results | — |
| Capability effects | `builder: Borrow, kind=process_builder, rights=0x20` |
| User memory | `name: Read, len=name_size bytes, max-bytes=64; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Task` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `process_builder_set_data`

Object-local method `process_builder_set_data`.

| Property | Contract |
| --- | --- |
| Receiver | `builder: Borrow, kind=process_builder, rights=0x20` |
| Operation | 257 |
| Feature | `Core` |
| Arguments | `data: user_address`, `data_size: byte_count` |
| Results | — |
| Capability effects | `builder: Borrow, kind=process_builder, rights=0x20` |
| User memory | `data: Read, len=data_size bytes, max-bytes=16384; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Task` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `process_builder_set_affinity`

Object-local method `process_builder_set_affinity`.

| Property | Contract |
| --- | --- |
| Receiver | `builder: Borrow, kind=process_builder, rights=0x20` |
| Operation | 258 |
| Feature | `Core` |
| Arguments | `affinity_words: user_address`, `word_count: element_count` |
| Results | — |
| Capability effects | `builder: Borrow, kind=process_builder, rights=0x20` |
| User memory | `affinity_words: Read, len=word_count elements, max-elements=4, element-size=8; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Task` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `process_builder_add_handle`

Object-local method `process_builder_add_handle`.

| Property | Contract |
| --- | --- |
| Receiver | `builder: Borrow, kind=process_builder, rights=0x20` |
| Operation | 259 |
| Feature | `Core` |
| Arguments | `source: handle`, `purpose: u32`, `expected_kind: u32`, `rights: rights`, `operation: u32` |
| Results | — |
| Capability effects | `builder: Borrow, kind=process_builder, rights=0x20`, `source: ByOperation(operation: move=0:ConsumeOnCommit/rights=0x2+duplicate=1:Borrow/rights=0x3), any, common-rights=0x2` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `process_builder_seal`

Object-local method `process_builder_seal`.

| Property | Contract |
| --- | --- |
| Receiver | `builder: Borrow, kind=process_builder, rights=0x20` |
| Operation | 260 |
| Feature | `Core` |
| Arguments | — |
| Results | — |
| Capability effects | `builder: Borrow, kind=process_builder, rights=0x20` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Task` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `process_builder_start`

Object-local method `process_builder_start`.

| Property | Contract |
| --- | --- |
| Receiver | `builder: ConsumeOnCommit, kind=process_builder, rights=0x400` |
| Operation | 261 |
| Feature | `Core` |
| Arguments | — |
| Results | `process: handle`, `startup_channel: handle` |
| Capability effects | `builder: ConsumeOnCommit, kind=process_builder, rights=0x400`, `process: produce, kind=process, fixed=0x80e`, `startup_channel: produce, kind=byte_channel, fixed=0x14` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Task` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `process_builder_abort`

Object-local method `process_builder_abort`.

| Property | Contract |
| --- | --- |
| Receiver | `builder: ConsumeOnCommit, kind=process_builder, rights=0x800` |
| Operation | 262 |
| Feature | `Core` |
| Arguments | — |
| Results | — |
| Capability effects | `builder: ConsumeOnCommit, kind=process_builder, rights=0x800` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Task` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `process_request_stop`

Object-local method `process_request_stop`.

| Property | Contract |
| --- | --- |
| Receiver | `process: Borrow, kind=process, rights=0x800` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | — |
| Results | — |
| Capability effects | `process: Borrow, kind=process, rights=0x800` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Task` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `object_wait_many`

Object-local method `object_wait_many`.

| Property | Contract |
| --- | --- |
| Receiver | `receiver: Borrow, kind=execution_context, rights=0x20` |
| Operation | 259 |
| Feature | `Core` |
| Arguments | `items: user_address`, `item_count: element_count`, `deadline: u64` |
| Results | `index: element_count`, `observed: u64` |
| Capability effects | `receiver: Borrow, kind=execution_context, rights=0x20` |
| User memory | `items: Read, len=item_count elements, max-elements=64, element-size=16, record=object_wait_item, borrowed-handles=(handle), required-rights=0x4; order=0` |
| Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` |
| Audit | `Object` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `process_get_info`

Object-local method `process_get_info`.

| Property | Contract |
| --- | --- |
| Receiver | `process: Borrow, kind=process, rights=0x8` |
| Operation | 257 |
| Feature | `Core` |
| Arguments | `info: user_address`, `info_size: byte_count` |
| Results | `supported_size: byte_count` |
| Capability effects | `process: Borrow, kind=process, rights=0x8` |
| User memory | `info: Write, len=info_size bytes, max-bytes=4096, record=process_info; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Task` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `task_inspector_scan_processes`

Object-local method `task_inspector_scan_processes`.

| Property | Contract |
| --- | --- |
| Receiver | `inspector: Borrow, kind=task_inspector, rights=0x8` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | `cursor: u64`, `records: user_address`, `capacity: element_count` |
| Results | `count: element_count`, `next_cursor: u64` |
| Capability effects | `inspector: Borrow, kind=task_inspector, rights=0x8` |
| User memory | `records: Write, len=capacity elements, max-elements=8, element-size=96, record=task_process; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Task` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `task_inspector_scan_threads`

Object-local method `task_inspector_scan_threads`.

| Property | Contract |
| --- | --- |
| Receiver | `inspector: Borrow, kind=task_inspector, rights=0x8` |
| Operation | 257 |
| Feature | `Core` |
| Arguments | `cursor: u64`, `records: user_address`, `capacity: element_count` |
| Results | `count: element_count`, `next_cursor: u64` |
| Capability effects | `inspector: Borrow, kind=task_inspector, rights=0x8` |
| User memory | `records: Write, len=capacity elements, max-elements=8, element-size=104, record=task_thread; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Task` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `task_inspector_derive_process`

Object-local method `task_inspector_derive_process`.

| Property | Contract |
| --- | --- |
| Receiver | `inspector: Borrow, kind=task_inspector, rights=0x1000000b` |
| Operation | 258 |
| Feature | `Core` |
| Arguments | `process: handle` |
| Results | `inspector: handle` |
| Capability effects | `inspector: Borrow, kind=task_inspector, rights=0x1000000b`, `process: Borrow, kind=process, rights=0x8`, `inspector: produce, kind=task_inspector, fixed=0x1000000b` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Task` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `object_inspector_scan_objects`

Object-local method `object_inspector_scan_objects`.

| Property | Contract |
| --- | --- |
| Receiver | `inspector: Borrow, kind=object_inspector, rights=0x8` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | `cursor: u64`, `records: user_address`, `capacity: element_count` |
| Results | `count: element_count`, `next_cursor: u64` |
| Capability effects | `inspector: Borrow, kind=object_inspector, rights=0x8` |
| User memory | `records: Write, len=capacity elements, max-elements=8, element-size=104, record=object_inspection; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Object` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `object_inspector_scan_handles`

Object-local method `object_inspector_scan_handles`.

| Property | Contract |
| --- | --- |
| Receiver | `inspector: Borrow, kind=object_inspector, rights=0x8` |
| Operation | 257 |
| Feature | `Core` |
| Arguments | `process_koid: u64`, `cursor: u64`, `records: user_address`, `capacity: element_count` |
| Results | `count: element_count`, `next_cursor: u64` |
| Capability effects | `inspector: Borrow, kind=object_inspector, rights=0x8` |
| User memory | `records: Write, len=capacity elements, max-elements=8, element-size=40, record=handle_inspection; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Object` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `object_inspector_derive_process`

Object-local method `object_inspector_derive_process`.

| Property | Contract |
| --- | --- |
| Receiver | `inspector: Borrow, kind=object_inspector, rights=0x1000000b` |
| Operation | 258 |
| Feature | `Core` |
| Arguments | `process: handle` |
| Results | `inspector: handle` |
| Capability effects | `inspector: Borrow, kind=object_inspector, rights=0x1000000b`, `process: Borrow, kind=process, rights=0x8`, `inspector: produce, kind=object_inspector, fixed=0x1000000b` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Object` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `task_inspector_derive_task_group`

Object-local method `task_inspector_derive_task_group`.

| Property | Contract |
| --- | --- |
| Receiver | `inspector: Borrow, kind=task_inspector, rights=0x1000000b` |
| Operation | 259 |
| Feature | `Core` |
| Arguments | `task_group: handle` |
| Results | `inspector: handle` |
| Capability effects | `inspector: Borrow, kind=task_inspector, rights=0x1000000b`, `task_group: Borrow, kind=task_group, rights=0x8`, `inspector: produce, kind=task_inspector, fixed=0x1000000b` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Task` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `object_inspector_derive_task_group`

Object-local method `object_inspector_derive_task_group`.

| Property | Contract |
| --- | --- |
| Receiver | `inspector: Borrow, kind=object_inspector, rights=0x1000000b` |
| Operation | 259 |
| Feature | `Core` |
| Arguments | `task_group: handle` |
| Results | `inspector: handle` |
| Capability effects | `inspector: Borrow, kind=object_inspector, rights=0x1000000b`, `task_group: Borrow, kind=task_group, rights=0x8`, `inspector: produce, kind=object_inspector, fixed=0x1000000b` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Object` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `task_inspector_derive_resource_domain`

Object-local method `task_inspector_derive_resource_domain`.

| Property | Contract |
| --- | --- |
| Receiver | `inspector: Borrow, kind=task_inspector, rights=0x1000000b` |
| Operation | 260 |
| Feature | `Core` |
| Arguments | `resource_domain: handle` |
| Results | `inspector: handle` |
| Capability effects | `inspector: Borrow, kind=task_inspector, rights=0x1000000b`, `resource_domain: Borrow, kind=resource_domain, rights=0x8`, `inspector: produce, kind=task_inspector, fixed=0x1000000b` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Task` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `object_inspector_derive_resource_domain`

Object-local method `object_inspector_derive_resource_domain`.

| Property | Contract |
| --- | --- |
| Receiver | `inspector: Borrow, kind=object_inspector, rights=0x1000000b` |
| Operation | 260 |
| Feature | `Core` |
| Arguments | `resource_domain: handle` |
| Results | `inspector: handle` |
| Capability effects | `inspector: Borrow, kind=object_inspector, rights=0x1000000b`, `resource_domain: Borrow, kind=resource_domain, rights=0x8`, `inspector: produce, kind=object_inspector, fixed=0x1000000b` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Object` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `directory_open_directory`

Object-local method `directory_open_directory`.

| Property | Contract |
| --- | --- |
| Receiver | `directory: Borrow, kind=directory, rights=0x10` |
| Operation | 257 |
| Feature | `Core` |
| Arguments | `path: user_address`, `path_size: byte_count`, `requested_rights: rights`, `options: u32` |
| Results | `child: handle` |
| Capability effects | `directory: Borrow, kind=directory, rights=0x10`, `child: produce, kind=directory, exact-from(requested_rights), allowed=0x9800000bb, required-from=directory` |
| User memory | `path: Read, len=path_size bytes, max-bytes=4096; order=0` |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `vmo_create`

Object-local method `vmo_create`.

| Property | Contract |
| --- | --- |
| Receiver | `receiver: Borrow, kind=object_factory, rights=0x8000000000` |
| Operation | 259 |
| Feature | `Core` |
| Arguments | `size: byte_count` |
| Results | `vmo: handle` |
| Capability effects | `receiver: Borrow, kind=object_factory, rights=0x8000000000`, `vmo: produce, kind=vmo, fixed=0x7b` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Object` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `vmo_read`

Object-local method `vmo_read`.

| Property | Contract |
| --- | --- |
| Receiver | `vmo: Borrow, kind=vmo, rights=0x10` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | `offset: u64`, `bytes: user_address`, `byte_count: byte_count` |
| Results | — |
| Capability effects | `vmo: Borrow, kind=vmo, rights=0x10` |
| User memory | `bytes: Write, len=byte_count bytes, max-bytes=65536; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `vmo_write`

Object-local method `vmo_write`.

| Property | Contract |
| --- | --- |
| Receiver | `vmo: Borrow, kind=vmo, rights=0x20` |
| Operation | 257 |
| Feature | `Core` |
| Arguments | `offset: u64`, `bytes: user_address`, `byte_count: byte_count` |
| Results | — |
| Capability effects | `vmo: Borrow, kind=vmo, rights=0x20` |
| User memory | `bytes: Read, len=byte_count bytes, max-bytes=65536; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `vmar_allocate`

Object-local method `vmar_allocate`.

| Property | Contract |
| --- | --- |
| Receiver | `vmar: Borrow, kind=vmar, rights=0x40` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | `address: u64`, `size: byte_count`, `options: u64` |
| Results | `child: handle`, `address: u64` |
| Capability effects | `vmar: Borrow, kind=vmar, rights=0x40`, `child: produce, kind=vmar, fixed=0x4b, inherited-from(vmar), mask=0x1000000000` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `vmar_map`

Object-local method `vmar_map`.

| Property | Contract |
| --- | --- |
| Receiver | `vmar: Borrow, kind=vmar, rights=0x40` |
| Operation | 257 |
| Feature | `Core` |
| Arguments | `vmo: handle`, `vmo_offset: u64`, `address: u64`, `size: byte_count`, `permissions: u32` |
| Results | — |
| Capability effects | `vmar: Borrow, kind=vmar, rights=0x40`, `vmo: Borrow, kind=vmo, rights=0x40` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `vmar_protect`

Object-local method `vmar_protect`.

| Property | Contract |
| --- | --- |
| Receiver | `vmar: Borrow, kind=vmar, rights=0x40` |
| Operation | 258 |
| Feature | `Core` |
| Arguments | `address: u64`, `size: byte_count`, `permissions: u32` |
| Results | — |
| Capability effects | `vmar: Borrow, kind=vmar, rights=0x40` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `vmar_unmap`

Object-local method `vmar_unmap`.

| Property | Contract |
| --- | --- |
| Receiver | `vmar: Borrow, kind=vmar, rights=0x40` |
| Operation | 259 |
| Feature | `Core` |
| Arguments | `address: u64`, `size: byte_count` |
| Results | — |
| Capability effects | `vmar: Borrow, kind=vmar, rights=0x40` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `vmar_destroy`

Object-local method `vmar_destroy`.

| Property | Contract |
| --- | --- |
| Receiver | `vmar: ConsumeOnCommit, kind=vmar, rights=0x40` |
| Operation | 260 |
| Feature | `Core` |
| Arguments | — |
| Results | — |
| Capability effects | `vmar: ConsumeOnCommit, kind=vmar, rights=0x40` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `directory_read`

Object-local method `directory_read`.

| Property | Contract |
| --- | --- |
| Receiver | `stream: Borrow, kind=directory_stream, rights=0x10` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | `cookie: u64`, `records: user_address`, `capacity: element_count`, `options: u32` |
| Results | `count: element_count`, `next_cookie: u64` |
| Capability effects | `stream: Borrow, kind=directory_stream, rights=0x10` |
| User memory | `records: Write, len=capacity elements, max-elements=4, element-size=280, record=directory_entry; order=0` |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `memory_inspector_read`

Object-local method `memory_inspector_read`.

| Property | Contract |
| --- | --- |
| Receiver | `inspector: Borrow, kind=memory_inspector, rights=0x8` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | `observation: user_address`, `observation_size: byte_count` |
| Results | `supported_size: byte_count` |
| Capability effects | `inspector: Borrow, kind=memory_inspector, rights=0x8` |
| User memory | `observation: Write, len=observation_size bytes, max-bytes=4096, record=memory_observation; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Object` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `cpu_inspector_profile_start`

Object-local method `cpu_inspector_profile_start`.

| Property | Contract |
| --- | --- |
| Receiver | `inspector: Borrow, kind=cpu_inspector, rights=0x80000000` |
| Operation | 257 |
| Feature | `Core` |
| Arguments | `period: u64`, `seconds: u64` |
| Results | `generation: u64` |
| Capability effects | `inspector: Borrow, kind=cpu_inspector, rights=0x80000000` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Object` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `cpu_inspector_profile_stop`

Object-local method `cpu_inspector_profile_stop`.

| Property | Contract |
| --- | --- |
| Receiver | `inspector: Borrow, kind=cpu_inspector, rights=0x80000000` |
| Operation | 258 |
| Feature | `Core` |
| Arguments | `generation: u64` |
| Results | — |
| Capability effects | `inspector: Borrow, kind=cpu_inspector, rights=0x80000000` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Object` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `cpu_inspector_profile_read`

Object-local method `cpu_inspector_profile_read`.

| Property | Contract |
| --- | --- |
| Receiver | `inspector: Borrow, kind=cpu_inspector, rights=0x200000000` |
| Operation | 259 |
| Feature | `Core` |
| Arguments | `generation: u64`, `cpu: u64`, `cursor: u64`, `output: user_address`, `output_size: byte_count` |
| Results | `supported_size: byte_count` |
| Capability effects | `inspector: Borrow, kind=cpu_inspector, rights=0x200000000` |
| User memory | `output: Write, len=output_size bytes, max-bytes=192; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Object` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `cpu_inspector_trace_start`

Object-local method `cpu_inspector_trace_start`.

| Property | Contract |
| --- | --- |
| Receiver | `inspector: Borrow, kind=cpu_inspector, rights=0x80000000` |
| Operation | 260 |
| Feature | `Core` |
| Arguments | `configuration: user_address` |
| Results | `generation: u64` |
| Capability effects | `inspector: Borrow, kind=cpu_inspector, rights=0x80000000` |
| User memory | `configuration: Read, fixed-bytes=56, record=trace_config; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Object` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `cpu_inspector_trace_stop`

Object-local method `cpu_inspector_trace_stop`.

| Property | Contract |
| --- | --- |
| Receiver | `inspector: Borrow, kind=cpu_inspector, rights=0x80000000` |
| Operation | 261 |
| Feature | `Core` |
| Arguments | `generation: u64` |
| Results | — |
| Capability effects | `inspector: Borrow, kind=cpu_inspector, rights=0x80000000` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Object` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `cpu_inspector_trace_read`

Object-local method `cpu_inspector_trace_read`.

| Property | Contract |
| --- | --- |
| Receiver | `inspector: Borrow, kind=cpu_inspector, rights=0x200000000` |
| Operation | 262 |
| Feature | `Core` |
| Arguments | `generation: u64`, `cpu: u64`, `cursor: u64`, `output: user_address`, `output_size: byte_count` |
| Results | `supported_size: byte_count` |
| Capability effects | `inspector: Borrow, kind=cpu_inspector, rights=0x200000000` |
| User memory | `output: Write, len=output_size bytes, max-bytes=2656; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Object` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `cpu_inspector_read`

Object-local method `cpu_inspector_read`.

| Property | Contract |
| --- | --- |
| Receiver | `inspector: Borrow, kind=cpu_inspector, rights=0x8` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | `observation: user_address`, `observation_size: byte_count` |
| Results | `supported_size: byte_count` |
| Capability effects | `inspector: Borrow, kind=cpu_inspector, rights=0x8` |
| User memory | `observation: Write, len=observation_size bytes, max-bytes=4096, record=cpu_observation; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Object` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `file_get_info`

Object-local method `file_get_info`.

| Property | Contract |
| --- | --- |
| Receiver | `file: Borrow, kind=file, rights=0x8` |
| Operation | 257 |
| Feature | `Core` |
| Arguments | `output: user_address`, `output_size: byte_count` |
| Results | `supported_size: byte_count` |
| Capability effects | `file: Borrow, kind=file, rights=0x8` |
| User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=file_info; order=0` |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Object` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `directory_get_info`

Object-local method `directory_get_info`.

| Property | Contract |
| --- | --- |
| Receiver | `directory: Borrow, kind=directory, rights=0x8` |
| Operation | 258 |
| Feature | `Core` |
| Arguments | `output: user_address`, `output_size: byte_count` |
| Results | `supported_size: byte_count` |
| Capability effects | `directory: Borrow, kind=directory, rights=0x8` |
| User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=directory_info; order=0` |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Object` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `virtual_machine_creation_lease_create`

Object-local method `virtual_machine_creation_lease_create`.

| Property | Contract |
| --- | --- |
| Receiver | `authority: Borrow, kind=virtual_machine_creation_authority, rights=0x30000000` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | `resource_domain: handle` |
| Results | `lease: handle` |
| Capability effects | `authority: Borrow, kind=virtual_machine_creation_authority, rights=0x30000000`, `resource_domain: Borrow, kind=resource_domain, rights=0x8000000`, `lease: produce, kind=virtual_machine_creation_lease, fixed=0x2000000a, inherited-from(authority), mask=0xf0000000000` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `virtual_machine_create`

Object-local method `virtual_machine_create`.

| Property | Contract |
| --- | --- |
| Receiver | `lease: ConsumeOnCommit, kind=virtual_machine_creation_lease, rights=0x20000000` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | `configuration: user_address`, `configuration_size: byte_count` |
| Results | `pending_virtual_machine: handle` |
| Capability effects | `lease: ConsumeOnCommit, kind=virtual_machine_creation_lease, rights=0x20000000`, `pending_virtual_machine: produce, kind=pending_virtual_machine, fixed=0xc2a, inherited-from(lease), mask=0xf0000000000` |
| User memory | `configuration: Read, len=configuration_size bytes, max-bytes=4096, record=virtual_machine_configuration; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `pending_virtual_machine_set_memory`

Object-local method `pending_virtual_machine_set_memory`.

| Property | Contract |
| --- | --- |
| Receiver | `pending_virtual_machine: Borrow, kind=pending_virtual_machine, rights=0x20` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | `vmo: handle` |
| Results | — |
| Capability effects | `pending_virtual_machine: Borrow, kind=pending_virtual_machine, rights=0x20`, `vmo: Borrow, kind=vmo, rights=0x70` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `pending_virtual_machine_set_bootstrap`

Object-local method `pending_virtual_machine_set_bootstrap`.

| Property | Contract |
| --- | --- |
| Receiver | `pending_virtual_machine: Borrow, kind=pending_virtual_machine, rights=0x20` |
| Operation | 257 |
| Feature | `Core` |
| Arguments | `bootstrap: user_address`, `bootstrap_size: byte_count` |
| Results | — |
| Capability effects | `pending_virtual_machine: Borrow, kind=pending_virtual_machine, rights=0x20` |
| User memory | `bootstrap: Read, len=bootstrap_size bytes, max-bytes=4096, record=virtual_cpu_bootstrap; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `pending_virtual_machine_seal`

Object-local method `pending_virtual_machine_seal`.

| Property | Contract |
| --- | --- |
| Receiver | `pending_virtual_machine: Borrow, kind=pending_virtual_machine, rights=0x20` |
| Operation | 258 |
| Feature | `Core` |
| Arguments | — |
| Results | — |
| Capability effects | `pending_virtual_machine: Borrow, kind=pending_virtual_machine, rights=0x20` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `pending_virtual_machine_install`

Object-local method `pending_virtual_machine_install`.

| Property | Contract |
| --- | --- |
| Receiver | `pending_virtual_machine: ConsumeOnCommit, kind=pending_virtual_machine, rights=0x400` |
| Operation | 259 |
| Feature | `Core` |
| Arguments | — |
| Results | `virtual_machine: handle`, `boot_virtual_cpu: handle` |
| Capability effects | `pending_virtual_machine: ConsumeOnCommit, kind=pending_virtual_machine, rights=0x400`, `virtual_machine: produce, kind=virtual_machine, fixed=0x82e, inherited-from(pending_virtual_machine), mask=0xf0000000000`, `boot_virtual_cpu: produce, kind=virtual_cpu, fixed=0x1000000042e, inherited-from(pending_virtual_machine), mask=0xe0000000000` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `pending_virtual_machine_abort`

Object-local method `pending_virtual_machine_abort`.

| Property | Contract |
| --- | --- |
| Receiver | `pending_virtual_machine: ConsumeOnCommit, kind=pending_virtual_machine, rights=0x800` |
| Operation | 260 |
| Feature | `Core` |
| Arguments | — |
| Results | — |
| Capability effects | `pending_virtual_machine: ConsumeOnCommit, kind=pending_virtual_machine, rights=0x800` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `virtual_machine_request_stop`

Object-local method `virtual_machine_request_stop`.

| Property | Contract |
| --- | --- |
| Receiver | `virtual_machine: Borrow, kind=virtual_machine, rights=0x800` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | — |
| Results | — |
| Capability effects | `virtual_machine: Borrow, kind=virtual_machine, rights=0x800` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `virtual_machine_get_info`

Object-local method `virtual_machine_get_info`.

| Property | Contract |
| --- | --- |
| Receiver | `virtual_machine: Borrow, kind=virtual_machine, rights=0x8` |
| Operation | 257 |
| Feature | `Core` |
| Arguments | `info: user_address`, `info_size: byte_count` |
| Results | `supported_size: byte_count` |
| Capability effects | `virtual_machine: Borrow, kind=virtual_machine, rights=0x8` |
| User memory | `info: Write, len=info_size bytes, max-bytes=4096, record=virtual_machine_info; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Object` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `virtual_cpu_get_info`

Object-local method `virtual_cpu_get_info`.

| Property | Contract |
| --- | --- |
| Receiver | `virtual_cpu: Borrow, kind=virtual_cpu, rights=0x8` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | `info: user_address`, `info_size: byte_count` |
| Results | `supported_size: byte_count` |
| Capability effects | `virtual_cpu: Borrow, kind=virtual_cpu, rights=0x8` |
| User memory | `info: Write, len=info_size bytes, max-bytes=4096, record=virtual_cpu_info; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Object` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `resource_domain_create`

Object-local method `resource_domain_create`.

| Property | Contract |
| --- | --- |
| Receiver | `parent: Borrow, kind=resource_domain, rights=0x800000` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | `limits: user_address`, `limits_size: byte_count` |
| Results | `child: handle` |
| Capability effects | `parent: Borrow, kind=resource_domain, rights=0x800000`, `child: produce, kind=resource_domain, fixed=0x984000b` |
| User memory | `limits: Read, len=limits_size bytes, max-bytes=4096, record=resource_limits; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `task_group_create`

Object-local method `task_group_create`.

| Property | Contract |
| --- | --- |
| Receiver | `factory: Borrow, kind=task_factory, rights=0x400000` |
| Operation | 257 |
| Feature | `Core` |
| Arguments | `resource_domain: handle` |
| Results | `group: handle` |
| Capability effects | `factory: Borrow, kind=task_factory, rights=0x400000`, `resource_domain: Borrow, kind=resource_domain, rights=0x8000000`, `group: produce, kind=task_group, fixed=0x400080f` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `virtual_cpu_start`

Object-local method `virtual_cpu_start`.

| Property | Contract |
| --- | --- |
| Receiver | `virtual_cpu: Borrow, kind=virtual_cpu, rights=0x400` |
| Operation | 257 |
| Feature | `Core` |
| Arguments | — |
| Results | — |
| Capability effects | `virtual_cpu: Borrow, kind=virtual_cpu, rights=0x400` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `pending_virtual_machine_set_virtual_serial`

Object-local method `pending_virtual_machine_set_virtual_serial`.

| Property | Contract |
| --- | --- |
| Receiver | `pending_virtual_machine: Borrow, kind=pending_virtual_machine, rights=0x20` |
| Operation | 261 |
| Feature | `Core` |
| Arguments | `virtual_serial: handle` |
| Results | — |
| Capability effects | `pending_virtual_machine: Borrow, kind=pending_virtual_machine, rights=0x20`, `virtual_serial: ConsumeOnCommit, kind=virtual_serial, rights=0x8002` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `clock_get_monotonic`

Object-local method `clock_get_monotonic`.

| Property | Contract |
| --- | --- |
| Receiver | `receiver: Borrow, kind=system_info, rights=0x10` |
| Operation | 257 |
| Feature | `Core` |
| Arguments | — |
| Results | `nanoseconds: u64` |
| Capability effects | `receiver: Borrow, kind=system_info, rights=0x10` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Abi` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `virtual_serial_create`

Object-local method `virtual_serial_create`.

| Property | Contract |
| --- | --- |
| Receiver | `receiver: Borrow, kind=object_factory, rights=0x8000000000` |
| Operation | 260 |
| Feature | `Core` |
| Arguments | — |
| Results | `virtual_serial: handle` |
| Capability effects | `receiver: Borrow, kind=object_factory, rights=0x8000000000`, `virtual_serial: produce, kind=virtual_serial, fixed=0x803f` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `virtual_serial_register_output`

Object-local method `virtual_serial_register_output`.

| Property | Contract |
| --- | --- |
| Receiver | `virtual_serial: Borrow, kind=virtual_serial, rights=0x20` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | `buffer: handle` |
| Results | — |
| Capability effects | `virtual_serial: Borrow, kind=virtual_serial, rights=0x20`, `buffer: Borrow, kind=vmo, rights=0x70` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `virtual_serial_write`

Object-local method `virtual_serial_write`.

| Property | Contract |
| --- | --- |
| Receiver | `virtual_serial: Borrow, kind=virtual_serial, rights=0x20` |
| Operation | 257 |
| Feature | `Core` |
| Arguments | `bytes: user_address`, `byte_count: byte_count` |
| Results | `actual_bytes: byte_count` |
| Capability effects | `virtual_serial: Borrow, kind=virtual_serial, rights=0x20` |
| User memory | `bytes: Read, len=byte_count bytes, max-bytes=4096; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `thread_create`

Object-local method `thread_create`.

| Property | Contract |
| --- | --- |
| Receiver | `receiver: Borrow, kind=execution_context, rights=0x200000` |
| Operation | 260 |
| Feature | `Core` |
| Arguments | `entry: u64`, `stack: u64`, `tls: u64`, `argument: u64`, `affinity_words: user_address`, `word_count: element_count` |
| Results | `thread: handle` |
| Capability effects | `receiver: Borrow, kind=execution_context, rights=0x200000`, `thread: produce, kind=thread, fixed=0x10000000c0f, inherited-from(receiver), mask=0xe0000000000` |
| User memory | `affinity_words: Read, len=word_count elements, max-elements=4, element-size=8; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Task` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `thread_start`

Object-local method `thread_start`.

| Property | Contract |
| --- | --- |
| Receiver | `thread: Borrow, kind=thread, rights=0x400` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | — |
| Results | — |
| Capability effects | `thread: Borrow, kind=thread, rights=0x400` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Task` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `thread_request_stop`

Object-local method `thread_request_stop`.

| Property | Contract |
| --- | --- |
| Receiver | `thread: Borrow, kind=thread, rights=0x800` |
| Operation | 257 |
| Feature | `Core` |
| Arguments | — |
| Results | — |
| Capability effects | `thread: Borrow, kind=thread, rights=0x800` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Task` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `atomic_wait`

Object-local method `atomic_wait`.

| Property | Contract |
| --- | --- |
| Receiver | `receiver: Borrow, kind=execution_context, rights=0x20` |
| Operation | 261 |
| Feature | `Core` |
| Arguments | `address: u64`, `expected: u32`, `deadline: u64` |
| Results | — |
| Capability effects | `receiver: Borrow, kind=execution_context, rights=0x20` |
| User memory | — |
| Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` |
| Audit | `Task` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `atomic_wake`

Object-local method `atomic_wake`.

| Property | Contract |
| --- | --- |
| Receiver | `receiver: Borrow, kind=execution_context, rights=0x20` |
| Operation | 262 |
| Feature | `Core` |
| Arguments | `address: u64`, `count: u32` |
| Results | `woken: u64` |
| Capability effects | `receiver: Borrow, kind=execution_context, rights=0x20` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Task` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `thread_sleep`

Object-local method `thread_sleep`.

| Property | Contract |
| --- | --- |
| Receiver | `receiver: Borrow, kind=execution_context, rights=0x20` |
| Operation | 263 |
| Feature | `Core` |
| Arguments | `deadline: u64` |
| Results | — |
| Capability effects | `receiver: Borrow, kind=execution_context, rights=0x20` |
| User memory | — |
| Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` |
| Audit | `Task` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `file_write_at`

Object-local method `file_write_at`.

| Property | Contract |
| --- | --- |
| Receiver | `file: Borrow, kind=file, rights=0x20` |
| Operation | 258 |
| Feature | `Core` |
| Arguments | `options: u32`, `offset: u64`, `input: user_address`, `input_size: byte_count` |
| Results | `actual_bytes: byte_count`, `end_offset: u64` |
| Capability effects | `file: Borrow, kind=file, rights=0x20` |
| User memory | `input: Read, len=input_size bytes, max-bytes=2097152; order=0` |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `file_resize`

Object-local method `file_resize`.

| Property | Contract |
| --- | --- |
| Receiver | `file: Borrow, kind=file, rights=0x20` |
| Operation | 259 |
| Feature | `Core` |
| Arguments | `size: byte_count` |
| Results | — |
| Capability effects | `file: Borrow, kind=file, rights=0x20` |
| User memory | — |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `directory_create_file`

Object-local method `directory_create_file`.

| Property | Contract |
| --- | --- |
| Receiver | `directory: Borrow, kind=directory, rights=0x30` |
| Operation | 259 |
| Feature | `Core` |
| Arguments | `path: user_address`, `path_size: byte_count`, `requested_rights: rights`, `mode: u32` |
| Results | `file: handle` |
| Capability effects | `directory: Borrow, kind=directory, rights=0x30`, `file: produce, kind=file, exact-from(requested_rights), allowed=0x9800000bb, required-from=directory` |
| User memory | `path: Read, len=path_size bytes, max-bytes=4096; order=0` |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `directory_create_directory`

Object-local method `directory_create_directory`.

| Property | Contract |
| --- | --- |
| Receiver | `directory: Borrow, kind=directory, rights=0x30` |
| Operation | 260 |
| Feature | `Core` |
| Arguments | `path: user_address`, `path_size: byte_count`, `mode: u32` |
| Results | — |
| Capability effects | `directory: Borrow, kind=directory, rights=0x30` |
| User memory | `path: Read, len=path_size bytes, max-bytes=4096; order=0` |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `directory_remove`

Object-local method `directory_remove`.

| Property | Contract |
| --- | --- |
| Receiver | `directory: Borrow, kind=directory, rights=0x30` |
| Operation | 261 |
| Feature | `Core` |
| Arguments | `path: user_address`, `path_size: byte_count`, `options: u32` |
| Results | — |
| Capability effects | `directory: Borrow, kind=directory, rights=0x30` |
| User memory | `path: Read, len=path_size bytes, max-bytes=4096; order=0` |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `wait_set_create`

Object-local method `wait_set_create`.

| Property | Contract |
| --- | --- |
| Receiver | `receiver: Borrow, kind=object_factory, rights=0x8000000000` |
| Operation | 261 |
| Feature | `Core` |
| Arguments | `capacity: element_count` |
| Results | `wait_set: handle` |
| Capability effects | `receiver: Borrow, kind=object_factory, rights=0x8000000000`, `wait_set: produce, kind=wait_set, fixed=0x4000000d` |
| User memory | — |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `wait_set_add`

Object-local method `wait_set_add`.

| Property | Contract |
| --- | --- |
| Receiver | `wait_set: Borrow, kind=wait_set, rights=0x40000000` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | `object: handle`, `signals: u64` |
| Results | `registration: u64` |
| Capability effects | `wait_set: Borrow, kind=wait_set, rights=0x40000000`, `object: Borrow, any, rights=0x4` |
| User memory | — |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `wait_set_rearm`

Object-local method `wait_set_rearm`.

| Property | Contract |
| --- | --- |
| Receiver | `wait_set: Borrow, kind=wait_set, rights=0x40000000` |
| Operation | 257 |
| Feature | `Core` |
| Arguments | `registration: u64` |
| Results | — |
| Capability effects | `wait_set: Borrow, kind=wait_set, rights=0x40000000` |
| User memory | — |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `wait_set_remove`

Object-local method `wait_set_remove`.

| Property | Contract |
| --- | --- |
| Receiver | `wait_set: Borrow, kind=wait_set, rights=0x40000000` |
| Operation | 258 |
| Feature | `Core` |
| Arguments | `registration: u64` |
| Results | — |
| Capability effects | `wait_set: Borrow, kind=wait_set, rights=0x40000000` |
| User memory | — |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `wait_set_wait`

Object-local method `wait_set_wait`.

| Property | Contract |
| --- | --- |
| Receiver | `wait_set: Borrow, kind=wait_set, rights=0x4` |
| Operation | 259 |
| Feature | `Core` |
| Arguments | `deadline: u64`, `output: user_address`, `output_size: byte_count` |
| Results | — |
| Capability effects | `wait_set: Borrow, kind=wait_set, rights=0x4` |
| User memory | `output: Write, len=output_size bytes, max-bytes=24, record=wait_set_event; order=0` |
| Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `process_get_current_id`

Object-local method `process_get_current_id`.

| Property | Contract |
| --- | --- |
| Receiver | `receiver: Borrow, kind=execution_context, rights=0x10` |
| Operation | 264 |
| Feature | `Core` |
| Arguments | — |
| Results | `koid: u64` |
| Capability effects | `receiver: Borrow, kind=execution_context, rights=0x10` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Task` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `virtual_serial_acknowledge_output`

Object-local method `virtual_serial_acknowledge_output`.

| Property | Contract |
| --- | --- |
| Receiver | `virtual_serial: Borrow, kind=virtual_serial, rights=0x10` |
| Operation | 258 |
| Feature | `Core` |
| Arguments | `consumed: u64` |
| Results | — |
| Capability effects | `virtual_serial: Borrow, kind=virtual_serial, rights=0x10` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `directory_scope_create`

Object-local method `directory_scope_create`.

| Property | Contract |
| --- | --- |
| Receiver | `root: Borrow, kind=directory, rights=0x10` |
| Operation | 262 |
| Feature | `Core` |
| Arguments | `start: handle`, `rights: rights` |
| Results | `directory: handle` |
| Capability effects | `root: Borrow, kind=directory, rights=0x10`, `start: Borrow, kind=directory, rights=0x10`, `directory: produce, kind=directory, exact-from(rights), allowed=0xd800000bb, required-from=root` |
| User memory | — |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `directory_get_metadata`

Object-local method `directory_get_metadata`.

| Property | Contract |
| --- | --- |
| Receiver | `directory: Borrow, kind=directory, rights=0x18` |
| Operation | 263 |
| Feature | `Core` |
| Arguments | `path: user_address`, `path_length: byte_count`, `options: u32`, `output: user_address`, `output_size: byte_count` |
| Results | `supported_size: byte_count` |
| Capability effects | `directory: Borrow, kind=directory, rights=0x18` |
| User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=0`, `output: Write, len=output_size bytes, max-bytes=4096, record=file_metadata; order=1` |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `file_get_metadata`

Object-local method `file_get_metadata`.

| Property | Contract |
| --- | --- |
| Receiver | `file: Borrow, kind=file, rights=0x8` |
| Operation | 260 |
| Feature | `Core` |
| Arguments | `output: user_address`, `output_size: byte_count` |
| Results | `supported_size: byte_count` |
| Capability effects | `file: Borrow, kind=file, rights=0x8` |
| User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=file_metadata; order=0` |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `directory_get_self_metadata`

Object-local method `directory_get_self_metadata`.

| Property | Contract |
| --- | --- |
| Receiver | `directory: Borrow, kind=directory, rights=0x8` |
| Operation | 264 |
| Feature | `Core` |
| Arguments | `output: user_address`, `output_size: byte_count` |
| Results | `supported_size: byte_count` |
| Capability effects | `directory: Borrow, kind=directory, rights=0x8` |
| User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=file_metadata; order=0` |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `directory_set_metadata`

Object-local method `directory_set_metadata`.

| Property | Contract |
| --- | --- |
| Receiver | `directory: Borrow, kind=directory, rights=0x80000010` |
| Operation | 265 |
| Feature | `Core` |
| Arguments | `path: user_address`, `path_length: byte_count`, `options: u32`, `input: user_address`, `input_size: byte_count` |
| Results | — |
| Capability effects | `directory: Borrow, kind=directory, rights=0x80000010` |
| User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=1`, `input: Read, len=input_size bytes, max-bytes=4096, record=file_metadata_update; order=0` |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `file_set_metadata`

Object-local method `file_set_metadata`.

| Property | Contract |
| --- | --- |
| Receiver | `file: Borrow, kind=file, rights=0x80000000` |
| Operation | 261 |
| Feature | `Core` |
| Arguments | `input: user_address`, `input_size: byte_count` |
| Results | — |
| Capability effects | `file: Borrow, kind=file, rights=0x80000000` |
| User memory | `input: Read, len=input_size bytes, max-bytes=4096, record=file_metadata_update; order=0` |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `directory_rename`

Object-local method `directory_rename`.

| Property | Contract |
| --- | --- |
| Receiver | `source: Borrow, kind=directory, rights=0x30` |
| Operation | 266 |
| Feature | `Core` |
| Arguments | `path: user_address`, `path_length: byte_count`, `destination: handle`, `new_path: user_address`, `new_path_length: byte_count` |
| Results | — |
| Capability effects | `source: Borrow, kind=directory, rights=0x30`, `destination: Borrow, kind=directory, rights=0x30` |
| User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=0`, `new_path: Read, len=new_path_length bytes, max-bytes=4096; order=1` |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `directory_link`

Object-local method `directory_link`.

| Property | Contract |
| --- | --- |
| Receiver | `source: Borrow, kind=directory, rights=0x30` |
| Operation | 267 |
| Feature | `Core` |
| Arguments | `path: user_address`, `path_length: byte_count`, `destination: handle`, `new_path: user_address`, `new_path_length: byte_count` |
| Results | — |
| Capability effects | `source: Borrow, kind=directory, rights=0x30`, `destination: Borrow, kind=directory, rights=0x30` |
| User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=0`, `new_path: Read, len=new_path_length bytes, max-bytes=4096; order=1` |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `directory_symlink`

Object-local method `directory_symlink`.

| Property | Contract |
| --- | --- |
| Receiver | `directory: Borrow, kind=directory, rights=0x30` |
| Operation | 268 |
| Feature | `Core` |
| Arguments | `path: user_address`, `path_length: byte_count`, `target: user_address`, `target_length: byte_count` |
| Results | — |
| Capability effects | `directory: Borrow, kind=directory, rights=0x30` |
| User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=0`, `target: Read, len=target_length bytes, max-bytes=4096; order=1` |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `directory_read_link`

Object-local method `directory_read_link`.

| Property | Contract |
| --- | --- |
| Receiver | `directory: Borrow, kind=directory, rights=0x10` |
| Operation | 269 |
| Feature | `Core` |
| Arguments | `path: user_address`, `path_length: byte_count`, `output: user_address`, `capacity: byte_count` |
| Results | `actual: byte_count` |
| Capability effects | `directory: Borrow, kind=directory, rights=0x10` |
| User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=0`, `output: Write, len=capacity bytes, max-bytes=4096; order=1` |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `directory_canonicalize`

Object-local method `directory_canonicalize`.

| Property | Contract |
| --- | --- |
| Receiver | `directory: Borrow, kind=directory, rights=0x10` |
| Operation | 270 |
| Feature | `Core` |
| Arguments | `path: user_address`, `path_length: byte_count`, `output: user_address`, `capacity: byte_count` |
| Results | `actual: byte_count` |
| Capability effects | `directory: Borrow, kind=directory, rights=0x10` |
| User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=0`, `output: Write, len=capacity bytes, max-bytes=4096; order=1` |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `directory_remove_if`

Object-local method `directory_remove_if`.

| Property | Contract |
| --- | --- |
| Receiver | `directory: Borrow, kind=directory, rights=0x30` |
| Operation | 271 |
| Feature | `Core` |
| Arguments | `path: user_address`, `path_length: byte_count`, `options: u32`, `expected_node_id: u64` |
| Results | — |
| Capability effects | `directory: Borrow, kind=directory, rights=0x30` |
| User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=0` |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `directory_open_directory_nofollow`

Object-local method `directory_open_directory_nofollow`.

| Property | Contract |
| --- | --- |
| Receiver | `directory: Borrow, kind=directory, rights=0x10` |
| Operation | 272 |
| Feature | `Core` |
| Arguments | `path: user_address`, `path_length: byte_count`, `rights: rights` |
| Results | `directory: handle` |
| Capability effects | `directory: Borrow, kind=directory, rights=0x10`, `directory: produce, kind=directory, exact-from(rights), allowed=0x9800000bb, required-from=directory` |
| User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=0` |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `file_sync`

Object-local method `file_sync`.

| Property | Contract |
| --- | --- |
| Receiver | `file: Borrow, kind=file, rights=0x20` |
| Operation | 262 |
| Feature | `Core` |
| Arguments | `scope: u32` |
| Results | — |
| Capability effects | `file: Borrow, kind=file, rights=0x20` |
| User memory | — |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `file_lock`

Object-local method `file_lock`.

| Property | Contract |
| --- | --- |
| Receiver | `file: Borrow, kind=file, rights=0x100000000` |
| Operation | 263 |
| Feature | `Core` |
| Arguments | `mode: u32`, `deadline: u64` |
| Results | — |
| Capability effects | `file: Borrow, kind=file, rights=0x100000000` |
| User memory | — |
| Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `file_unlock`

Object-local method `file_unlock`.

| Property | Contract |
| --- | --- |
| Receiver | `file: Borrow, kind=file, rights=0x100000000` |
| Operation | 264 |
| Feature | `Core` |
| Arguments | — |
| Results | — |
| Capability effects | `file: Borrow, kind=file, rights=0x100000000` |
| User memory | — |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `clock_get_realtime`

Object-local method `clock_get_realtime`.

| Property | Contract |
| --- | --- |
| Receiver | `receiver: Borrow, kind=system_info, rights=0x10` |
| Operation | 258 |
| Feature | `Core` |
| Arguments | — |
| Results | `seconds: i64`, `nanoseconds: u32` |
| Capability effects | `receiver: Borrow, kind=system_info, rights=0x10` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `directory_open_file_with_options`

Object-local method `directory_open_file_with_options`.

| Property | Contract |
| --- | --- |
| Receiver | `directory: Borrow, kind=directory, rights=0x10` |
| Operation | 273 |
| Feature | `Core` |
| Arguments | `path: user_address`, `path_length: byte_count`, `rights: rights`, `options: u32`, `mode: u32` |
| Results | `file: handle` |
| Capability effects | `directory: Borrow, kind=directory, rights=0x10`, `file: produce, kind=file, exact-from(rights), allowed=0x9800000bb, required-from=directory` |
| User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=0` |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=Strict` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `virtual_machine_creation_lease_get_platform_info`

Object-local method `virtual_machine_creation_lease_get_platform_info`.

| Property | Contract |
| --- | --- |
| Receiver | `lease: Borrow, kind=virtual_machine_creation_lease, rights=0x8` |
| Operation | 257 |
| Feature | `Core` |
| Arguments | `platform_profile: u32`, `info: user_address`, `info_size: byte_count` |
| Results | `supported_size: byte_count` |
| Capability effects | `lease: Borrow, kind=virtual_machine_creation_lease, rights=0x8` |
| User memory | `info: Write, len=info_size bytes, max-bytes=4096, record=virtual_machine_platform_info; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Object` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `vmo_create_snapshot`

Object-local method `vmo_create_snapshot`.

| Property | Contract |
| --- | --- |
| Receiver | `vmo: Borrow, kind=vmo, rights=0x10` |
| Operation | 258 |
| Feature | `Core` |
| Arguments | — |
| Results | `vmo: handle`, `byte_size: byte_count` |
| Capability effects | `vmo: Borrow, kind=vmo, rights=0x10`, `vmo: produce, kind=vmo, fixed=0x5b` |
| User memory | — |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `vmar_map_private`

Object-local method `vmar_map_private`.

| Property | Contract |
| --- | --- |
| Receiver | `vmar: Borrow, kind=vmar, rights=0x40` |
| Operation | 261 |
| Feature | `Core` |
| Arguments | `vmo: handle`, `mapping: user_address`, `mapping_size: byte_count` |
| Results | — |
| Capability effects | `vmar: Borrow, kind=vmar, rights=0x40`, `vmo: Borrow, kind=vmo, rights=0x50` |
| User memory | `mapping: Read, len=mapping_size bytes, max-bytes=4096, record=private_mapping; order=0` |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `virtual_machine_get_power_request`

Object-local method `virtual_machine_get_power_request`.

| Property | Contract |
| --- | --- |
| Receiver | `virtual_machine: Borrow, kind=virtual_machine, rights=0x20` |
| Operation | 258 |
| Feature | `Core` |
| Arguments | `request: user_address`, `request_size: byte_count` |
| Results | `supported_size: byte_count` |
| Capability effects | `virtual_machine: Borrow, kind=virtual_machine, rights=0x20` |
| User memory | `request: Write, len=request_size bytes, max-bytes=4096, record=virtual_machine_power_request; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `virtual_machine_complete_power_request`

Object-local method `virtual_machine_complete_power_request`.

| Property | Contract |
| --- | --- |
| Receiver | `virtual_machine: Borrow, kind=virtual_machine, rights=0x20` |
| Operation | 259 |
| Feature | `Core` |
| Arguments | `request_id: u64`, `accept: u32` |
| Results | — |
| Capability effects | `virtual_machine: Borrow, kind=virtual_machine, rights=0x20` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `virtual_machine_open_vcpu`

Object-local method `virtual_machine_open_vcpu`.

| Property | Contract |
| --- | --- |
| Receiver | `virtual_machine: Borrow, kind=virtual_machine, rights=0x20` |
| Operation | 260 |
| Feature | `Core` |
| Arguments | `vcpu_id: u32` |
| Results | `virtual_cpu: handle` |
| Capability effects | `virtual_machine: Borrow, kind=virtual_machine, rights=0x20`, `virtual_cpu: produce, kind=virtual_cpu, fixed=0x1000000042e, inherited-from(virtual_machine), mask=0xe0000000000` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `virtual_machine_register_mmio`

Object-local method `virtual_machine_register_mmio`.

| Property | Contract |
| --- | --- |
| Receiver | `virtual_machine: Borrow, kind=virtual_machine, rights=0x20` |
| Operation | 261 |
| Feature | `Core` |
| Arguments | `base: u64`, `length: u64`, `device: u64` |
| Results | — |
| Capability effects | `virtual_machine: Borrow, kind=virtual_machine, rights=0x20` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `virtual_cpu_get_mmio_request`

Object-local method `virtual_cpu_get_mmio_request`.

| Property | Contract |
| --- | --- |
| Receiver | `virtual_cpu: Borrow, kind=virtual_cpu, rights=0x20` |
| Operation | 258 |
| Feature | `Core` |
| Arguments | `request: user_address`, `request_size: byte_count` |
| Results | `supported_size: byte_count` |
| Capability effects | `virtual_cpu: Borrow, kind=virtual_cpu, rights=0x20` |
| User memory | `request: Write, len=request_size bytes, max-bytes=4096, record=virtual_cpu_mmio_request; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `virtual_cpu_complete_mmio`

Object-local method `virtual_cpu_complete_mmio`.

| Property | Contract |
| --- | --- |
| Receiver | `virtual_cpu: Borrow, kind=virtual_cpu, rights=0x20` |
| Operation | 259 |
| Feature | `Core` |
| Arguments | `request_id: u64`, `operation: u32`, `value: u64` |
| Results | — |
| Capability effects | `virtual_cpu: Borrow, kind=virtual_cpu, rights=0x20` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `guest_memory_create`

Object-local method `guest_memory_create`.

| Property | Contract |
| --- | --- |
| Receiver | `vmo: Borrow, kind=vmo, rights=0x70` |
| Operation | 259 |
| Feature | `Core` |
| Arguments | — |
| Results | `memory: handle` |
| Capability effects | `vmo: Borrow, kind=vmo, rights=0x70`, `memory: produce, kind=guest_memory, fixed=0x7b` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `pending_virtual_machine_map_memory`

Object-local method `pending_virtual_machine_map_memory`.

| Property | Contract |
| --- | --- |
| Receiver | `pending_virtual_machine: Borrow, kind=pending_virtual_machine, rights=0x20` |
| Operation | 262 |
| Feature | `Core` |
| Arguments | `memory: handle`, `guest_offset: u64`, `source_offset: u64`, `length: u64` |
| Results | — |
| Capability effects | `pending_virtual_machine: Borrow, kind=pending_virtual_machine, rights=0x20`, `memory: Borrow, kind=guest_memory, rights=0x40` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `vmo_create_contiguous`

Object-local method `vmo_create_contiguous`.

| Property | Contract |
| --- | --- |
| Receiver | `receiver: Borrow, kind=object_factory, rights=0x8000000000` |
| Operation | 262 |
| Feature | `Core` |
| Arguments | `size: byte_count` |
| Results | `vmo: handle` |
| Capability effects | `receiver: Borrow, kind=object_factory, rights=0x8000000000`, `vmo: produce, kind=vmo, fixed=0x7b` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `device_claim`

Object-local method `device_claim`.

| Property | Contract |
| --- | --- |
| Receiver | `authority: Borrow, kind=device_assignment_authority, rights=0x8000` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | `index: u32` |
| Results | `physical_device: handle` |
| Capability effects | `authority: Borrow, kind=device_assignment_authority, rights=0x8000`, `physical_device: produce, kind=physical_device, fixed=0x800f` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `physical_device_info`

Object-local method `physical_device_info`.

| Property | Contract |
| --- | --- |
| Receiver | `device: Borrow, kind=physical_device, rights=0x8` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | `output: user_address`, `output_size: byte_count` |
| Results | `supported_size: byte_count` |
| Capability effects | `device: Borrow, kind=physical_device, rights=0x8` |
| User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=physical_device_info; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `vmo_get_dma_extent`

Object-local method `vmo_get_dma_extent`.

| Property | Contract |
| --- | --- |
| Receiver | `authority: Borrow, kind=device_assignment_authority, rights=0x10000` |
| Operation | 257 |
| Feature | `Core` |
| Arguments | `vmo: handle`, `offset: u64`, `length: u64`, `output: user_address`, `output_size: byte_count` |
| Results | `supported_size: byte_count` |
| Capability effects | `authority: Borrow, kind=device_assignment_authority, rights=0x10000`, `vmo: Borrow, kind=vmo, rights=0x50` |
| User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=dma_extent; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `pending_virtual_machine_assign_device`

Object-local method `pending_virtual_machine_assign_device`.

| Property | Contract |
| --- | --- |
| Receiver | `pending: Borrow, kind=pending_virtual_machine, rights=0x20` |
| Operation | 263 |
| Feature | `Core` |
| Arguments | `device: handle`, `guest_base: u64`, `guest_irq: u32` |
| Results | — |
| Capability effects | `pending: Borrow, kind=pending_virtual_machine, rights=0x20`, `device: Borrow, kind=physical_device, rights=0x8000` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `guest_mailbox_create`

Object-local method `guest_mailbox_create`.

| Property | Contract |
| --- | --- |
| Receiver | `machine: Borrow, kind=virtual_machine, rights=0x20` |
| Operation | 262 |
| Feature | `Core` |
| Arguments | `base: u64`, `irq: u32` |
| Results | `guest_mailbox: handle` |
| Capability effects | `machine: Borrow, kind=virtual_machine, rights=0x20`, `guest_mailbox: produce, kind=guest_mailbox, fixed=0x3e` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `guest_mailbox_send`

Object-local method `guest_mailbox_send`.

| Property | Contract |
| --- | --- |
| Receiver | `mailbox: Borrow, kind=guest_mailbox, rights=0x20` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | `bytes: user_address`, `length: byte_count` |
| Results | — |
| Capability effects | `mailbox: Borrow, kind=guest_mailbox, rights=0x20` |
| User memory | `bytes: Read, len=length bytes, max-bytes=256; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `guest_mailbox_receive`

Object-local method `guest_mailbox_receive`.

| Property | Contract |
| --- | --- |
| Receiver | `mailbox: Borrow, kind=guest_mailbox, rights=0x10` |
| Operation | 257 |
| Feature | `Core` |
| Arguments | `bytes: user_address`, `capacity: byte_count` |
| Results | `bytes: byte_count` |
| Capability effects | `mailbox: Borrow, kind=guest_mailbox, rights=0x10` |
| User memory | `bytes: Write, len=capacity bytes, max-bytes=256; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `guest_notification_create`

Object-local method `guest_notification_create`.

| Property | Contract |
| --- | --- |
| Receiver | `frontend: Borrow, kind=virtual_machine, rights=0x20` |
| Operation | 263 |
| Feature | `Core` |
| Arguments | `backend: handle`, `frontend_base: u64`, `backend_base: u64`, `frontend_irq: u32`, `backend_irq: u32` |
| Results | `guest_notification: handle` |
| Capability effects | `frontend: Borrow, kind=virtual_machine, rights=0x20`, `backend: Borrow, kind=virtual_machine, rights=0x20`, `guest_notification: produce, kind=guest_notification, fixed=0x2e` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `guest_notification_control`

Object-local method `guest_notification_control`.

| Property | Contract |
| --- | --- |
| Receiver | `notification: Borrow, kind=guest_notification, rights=0x20` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | `operation: u32` |
| Results | `epoch: u32` |
| Capability effects | `notification: Borrow, kind=guest_notification, rights=0x20` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `native_block_create`

Object-local method `native_block_create`.

| Property | Contract |
| --- | --- |
| Receiver | `memory: Borrow, kind=guest_memory, rights=0x40` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | `backend: handle`, `guest_base: u64`, `notification_base: u64`, `notification_irq: u32` |
| Results | `block: handle` |
| Capability effects | `memory: Borrow, kind=guest_memory, rights=0x40`, `backend: Borrow, kind=virtual_machine, rights=0x20`, `block: produce, kind=native_block, fixed=0x7e` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `native_block_activate`

Object-local method `native_block_activate`.

| Property | Contract |
| --- | --- |
| Receiver | `block: Borrow, kind=native_block, rights=0x20` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | `readonly: u32` |
| Results | `sectors: u64`, `readonly: u32` |
| Capability effects | `block: Borrow, kind=native_block, rights=0x20` |
| User memory | — |
| Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `filesystem_mount`

Object-local method `filesystem_mount`.

| Property | Contract |
| --- | --- |
| Receiver | `directory: Borrow, kind=directory, rights=0xa0` |
| Operation | 274 |
| Feature | `Core` |
| Arguments | `channel: handle`, `buffer: handle`, `path: user_address`, `path_length: byte_count`, `read_only: u64` |
| Results | — |
| Capability effects | `directory: Borrow, kind=directory, rights=0xa0`, `channel: Borrow, kind=byte_channel, rights=0x34`, `buffer: Borrow, kind=vmo, rights=0x70` |
| User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=0` |
| Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `guest_mapping_create`

Object-local method `guest_mapping_create`.

| Property | Contract |
| --- | --- |
| Receiver | `backend: Borrow, kind=virtual_machine, rights=0x20` |
| Operation | 264 |
| Feature | `Core` |
| Arguments | `memory: handle`, `frontend_base: u64` |
| Results | `mapping: handle`, `token: u64` |
| Capability effects | `backend: Borrow, kind=virtual_machine, rights=0x20`, `memory: Borrow, kind=guest_memory, rights=0x40`, `mapping: produce, kind=guest_mapping, fixed=0x2a` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `guest_mapping_release`

Object-local method `guest_mapping_release`.

| Property | Contract |
| --- | --- |
| Receiver | `mapping: Borrow, kind=guest_mapping, rights=0x20` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | — |
| Results | — |
| Capability effects | `mapping: Borrow, kind=guest_mapping, rights=0x20` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `device_claim_matching`

Object-local method `device_claim_matching`.

| Property | Contract |
| --- | --- |
| Receiver | `authority: Borrow, kind=device_assignment_authority, rights=0x8000` |
| Operation | 258 |
| Feature | `Core` |
| Arguments | `profile: u32`, `identity_kind: u32`, `identity: user_address`, `length: byte_count` |
| Results | `physical_device: handle` |
| Capability effects | `authority: Borrow, kind=device_assignment_authority, rights=0x8000`, `physical_device: produce, kind=physical_device, fixed=0x800f` |
| User memory | `identity: Read, len=length bytes, max-bytes=512; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `device_profile_info`

Object-local method `device_profile_info`.

| Property | Contract |
| --- | --- |
| Receiver | `device: Borrow, kind=physical_device, rights=0x8` |
| Operation | 257 |
| Feature | `Core` |
| Arguments | `output: user_address`, `output_size: byte_count` |
| Results | `supported_size: byte_count` |
| Capability effects | `device: Borrow, kind=physical_device, rights=0x8` |
| User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=device_profile_info; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `device_resource_info`

Object-local method `device_resource_info`.

| Property | Contract |
| --- | --- |
| Receiver | `device: Borrow, kind=physical_device, rights=0x8` |
| Operation | 258 |
| Feature | `Core` |
| Arguments | `index: u32`, `output: user_address`, `output_size: byte_count` |
| Results | `supported_size: byte_count` |
| Capability effects | `device: Borrow, kind=physical_device, rights=0x8` |
| User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=device_resource_info; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `device_firmware_read`

Object-local method `device_firmware_read`.

| Property | Contract |
| --- | --- |
| Receiver | `authority: Borrow, kind=device_assignment_authority, rights=0x8` |
| Operation | 259 |
| Feature | `Core` |
| Arguments | `query: user_address`, `output: user_address`, `capacity: byte_count` |
| Results | `value: u64` |
| Capability effects | `authority: Borrow, kind=device_assignment_authority, rights=0x8` |
| User memory | `query: Read, fixed-bytes=24, record=device_firmware_query; order=0`, `output: Write, len=capacity bytes, max-bytes=65536; order=1` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `device_claim_bundle`

Object-local method `device_claim_bundle`.

| Property | Contract |
| --- | --- |
| Receiver | `authority: Borrow, kind=device_assignment_authority, rights=0x8000` |
| Operation | 260 |
| Feature | `Core` |
| Arguments | `entries: user_address`, `count: element_count`, `irq_node: u32` |
| Results | `physical_device: handle` |
| Capability effects | `authority: Borrow, kind=device_assignment_authority, rights=0x8000`, `physical_device: produce, kind=physical_device, fixed=0x2803f` |
| User memory | `entries: Read, len=count elements, max-elements=8, element-size=16, record=device_bundle_entry; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `device_mmio`

Object-local method `device_mmio`.

| Property | Contract |
| --- | --- |
| Receiver | `device: Borrow, kind=physical_device, rights=0x0` |
| Operation | 259 |
| Feature | `Core` |
| Arguments | `offset: u64`, `width: u32`, `operation: u32`, `value: u64` |
| Results | `value: u64` |
| Capability effects | `device: Borrow, kind=physical_device, rights=0x0` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `device_irq_pending`

Object-local method `device_irq_pending`.

| Property | Contract |
| --- | --- |
| Receiver | `device: Borrow, kind=physical_device, rights=0x4` |
| Operation | 260 |
| Feature | `Core` |
| Arguments | — |
| Results | `value: u64` |
| Capability effects | `device: Borrow, kind=physical_device, rights=0x4` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `device_irq_complete`

Object-local method `device_irq_complete`.

| Property | Contract |
| --- | --- |
| Receiver | `device: Borrow, kind=physical_device, rights=0x20000` |
| Operation | 261 |
| Feature | `Core` |
| Arguments | `sequence: u64`, `asserted: u32` |
| Results | — |
| Capability effects | `device: Borrow, kind=physical_device, rights=0x20000` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `virtual_cpu_set_affinity`

Object-local method `virtual_cpu_set_affinity`.

| Property | Contract |
| --- | --- |
| Receiver | `virtual_cpu: Borrow, kind=virtual_cpu, rights=0x20` |
| Operation | 260 |
| Feature | `Core` |
| Arguments | `affinity_words: user_address`, `word_count: element_count` |
| Results | — |
| Capability effects | `virtual_cpu: Borrow, kind=virtual_cpu, rights=0x20` |
| User memory | `affinity_words: Read, len=word_count elements, max-elements=4, element-size=8; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `system_config`

Object-local method `system_config`.

| Property | Contract |
| --- | --- |
| Receiver | `receiver: Borrow, kind=system_info, rights=0x10` |
| Operation | 259 |
| Feature | `Core` |
| Arguments | `key: u64` |
| Results | `value: u64` |
| Capability effects | `receiver: Borrow, kind=system_info, rights=0x10` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Abi` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `object_inspector_read_details`

Object-local method `object_inspector_read_details`.

| Property | Contract |
| --- | --- |
| Receiver | `inspector: Borrow, kind=object_inspector, rights=0x200000008` |
| Operation | 261 |
| Feature | `Core` |
| Arguments | `process_koid: u64`, `target: u64`, `cursor: u64`, `record: user_address`, `record_size: byte_count` |
| Results | `supported_size: byte_count` |
| Capability effects | `inspector: Borrow, kind=object_inspector, rights=0x200000008` |
| User memory | `record: Write, len=record_size bytes, max-bytes=4096, record=object_details; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Object` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `filesystem_unmount`

Object-local method `filesystem_unmount`.

| Property | Contract |
| --- | --- |
| Receiver | `directory: Borrow, kind=directory, rights=0xa0` |
| Operation | 275 |
| Feature | `Core` |
| Arguments | `path: user_address`, `path_length: byte_count` |
| Results | — |
| Capability effects | `directory: Borrow, kind=directory, rights=0xa0` |
| User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=0` |
| Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `filesystem_mount_ramfs`

Object-local method `filesystem_mount_ramfs`.

| Property | Contract |
| --- | --- |
| Receiver | `directory: Borrow, kind=directory, rights=0x4000000a0` |
| Operation | 276 |
| Feature | `Core` |
| Arguments | `path: user_address`, `path_length: byte_count` |
| Results | — |
| Capability effects | `directory: Borrow, kind=directory, rights=0x4000000a0` |
| User memory | `path: Read, len=path_length bytes, max-bytes=4096; order=0` |
| Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `filesystem_switch_root`

Object-local method `filesystem_switch_root`.

| Property | Contract |
| --- | --- |
| Receiver | `directory: Borrow, kind=directory, rights=0x4000000a0` |
| Operation | 277 |
| Feature | `Core` |
| Arguments | `request: user_address` |
| Results | — |
| Capability effects | `directory: Borrow, kind=directory, rights=0x4000000a0` |
| User memory | `request: Read, fixed-bytes=48, record=filesystem_root_switch; order=0` |
| Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `file_protect`

Object-local method `file_protect`.

| Property | Contract |
| --- | --- |
| Receiver | `file: Borrow, kind=file, rights=0x800000000` |
| Operation | 265 |
| Feature | `Core` |
| Arguments | — |
| Results | `protection: handle`, `byte_size: byte_count` |
| Capability effects | `file: Borrow, kind=file, rights=0x800000000`, `protection: produce, kind=file_protection, fixed=0x48` |
| User memory | — |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `vmar_map_file`

Object-local method `vmar_map_file`.

| Property | Contract |
| --- | --- |
| Receiver | `vmar: Borrow, kind=vmar, rights=0x40` |
| Operation | 262 |
| Feature | `Core` |
| Arguments | `file: handle`, `protection: handle`, `mapping: user_address`, `mapping_size: byte_count` |
| Results | — |
| Capability effects | `vmar: Borrow, kind=vmar, rights=0x40`, `file: Borrow, kind=file, rights=0x10`, `protection: Borrow, kind=file_protection, rights=0x40` |
| User memory | `mapping: Read, len=mapping_size bytes, max-bytes=4096, record=private_mapping; order=0` |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `directory_begin_read`

Object-local method `directory_begin_read`.

| Property | Contract |
| --- | --- |
| Receiver | `directory: Borrow, kind=directory, rights=0x10` |
| Operation | 278 |
| Feature | `Core` |
| Arguments | — |
| Results | `stream: handle` |
| Capability effects | `directory: Borrow, kind=directory, rights=0x10`, `stream: produce, kind=directory_stream, inherited-from(directory), mask=0x1b` |
| User memory | — |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `process_derive_observer`

Object-local method `process_derive_observer`.

| Property | Contract |
| --- | --- |
| Receiver | `process: Borrow, kind=process, rights=0x806` |
| Operation | 258 |
| Feature | `Core` |
| Arguments | — |
| Results | `observer: handle` |
| Capability effects | `process: Borrow, kind=process, rights=0x806`, `observer: produce, kind=process, fixed=0x6` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Task` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `task_group_request_stop`

Object-local method `task_group_request_stop`.

| Property | Contract |
| --- | --- |
| Receiver | `task_group: Borrow, kind=task_group, rights=0x800` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | — |
| Results | — |
| Capability effects | `task_group: Borrow, kind=task_group, rights=0x800` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Task` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `console_discard_input`

Object-local method `console_discard_input`.

| Property | Contract |
| --- | --- |
| Receiver | `console: Borrow, kind=console, rights=0x10` |
| Operation | 258 |
| Feature | `Core` |
| Arguments | — |
| Results | `discarded: byte_count` |
| Capability effects | `console: Borrow, kind=console, rights=0x10` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `guest_notification_configure`

Object-local method `guest_notification_configure`.

| Property | Contract |
| --- | --- |
| Receiver | `machine: Borrow, kind=virtual_machine, rights=0x20` |
| Operation | 265 |
| Feature | `Core` |
| Arguments | `original_base: u64`, `base: u64`, `flags: u32` |
| Results | — |
| Capability effects | `machine: Borrow, kind=virtual_machine, rights=0x20` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `guest_notification_access`

Object-local method `guest_notification_access`.

| Property | Contract |
| --- | --- |
| Receiver | `machine: Borrow, kind=virtual_machine, rights=0x20` |
| Operation | 266 |
| Feature | `Core` |
| Arguments | `original_base: u64`, `operation: u32`, `value: u32` |
| Results | `value: u32` |
| Capability effects | `machine: Borrow, kind=virtual_machine, rights=0x20` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `vmar_lock_memory`

Object-local method `vmar_lock_memory`.

| Property | Contract |
| --- | --- |
| Receiver | `vmar: Borrow, kind=vmar, rights=0x1000000000` |
| Operation | 263 |
| Feature | `Core` |
| Arguments | `address: u64`, `size: byte_count`, `flags: u32` |
| Results | — |
| Capability effects | `vmar: Borrow, kind=vmar, rights=0x1000000000` |
| User memory | — |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `vmar_unlock_memory`

Object-local method `vmar_unlock_memory`.

| Property | Contract |
| --- | --- |
| Receiver | `vmar: Borrow, kind=vmar, rights=0x1000000000` |
| Operation | 264 |
| Feature | `Core` |
| Arguments | `address: u64`, `size: byte_count`, `flags: u32` |
| Results | — |
| Capability effects | `vmar: Borrow, kind=vmar, rights=0x1000000000` |
| User memory | — |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `process_builder_set_memory_lock`

Object-local method `process_builder_set_memory_lock`.

| Property | Contract |
| --- | --- |
| Receiver | `builder: Borrow, kind=process_builder, rights=0x1000000020` |
| Operation | 263 |
| Feature | `Core` |
| Arguments | `enabled: u32` |
| Results | — |
| Capability effects | `builder: Borrow, kind=process_builder, rights=0x1000000020` |
| User memory | — |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `filesystem_begin_root_switch`

Object-local method `filesystem_begin_root_switch`.

| Property | Contract |
| --- | --- |
| Receiver | `directory: Borrow, kind=directory, rights=0x4000000a0` |
| Operation | 279 |
| Feature | `Core` |
| Arguments | `request: user_address` |
| Results | `transition: handle` |
| Capability effects | `directory: Borrow, kind=directory, rights=0x4000000a0`, `transition: produce, kind=root_transition, fixed=0x2a` |
| User memory | `request: Read, fixed-bytes=48, record=filesystem_root_switch; order=0` |
| Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `filesystem_finish_root_switch`

Object-local method `filesystem_finish_root_switch`.

| Property | Contract |
| --- | --- |
| Receiver | `transition: Borrow, kind=root_transition, rights=0x20` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | `commit: u32` |
| Results | — |
| Capability effects | `transition: Borrow, kind=root_transition, rights=0x20` |
| User memory | — |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `guest_memory_transfer`

Object-local method `guest_memory_transfer`.

| Property | Contract |
| --- | --- |
| Receiver | `memory: Borrow, kind=guest_memory, rights=0x0` |
| Operation | 257 |
| Feature | `Core` |
| Arguments | `operation: u32`, `offset: u64`, `buffer: user_address`, `length: byte_count` |
| Results | — |
| Capability effects | `memory: Borrow, kind=guest_memory, rights=0x0` |
| User memory | `buffer: ReadWrite, len=length bytes, max-bytes=4096; order=0` |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `guest_memory_scalar`

Object-local method `guest_memory_scalar`.

| Property | Contract |
| --- | --- |
| Receiver | `memory: Borrow, kind=guest_memory, rights=0x0` |
| Operation | 258 |
| Feature | `Core` |
| Arguments | `operation: u32`, `offset: u64`, `width: u32`, `value: u64` |
| Results | `value: u64` |
| Capability effects | `memory: Borrow, kind=guest_memory, rights=0x0` |
| User memory | — |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `guest_interrupt_create`

Object-local method `guest_interrupt_create`.

| Property | Contract |
| --- | --- |
| Receiver | `machine: Borrow, kind=virtual_machine, rights=0x20` |
| Operation | 267 |
| Feature | `Core` |
| Arguments | `irq: u32` |
| Results | `interrupt: handle` |
| Capability effects | `machine: Borrow, kind=virtual_machine, rights=0x20`, `interrupt: produce, kind=guest_interrupt, fixed=0x2a` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `guest_interrupt_set_level`

Object-local method `guest_interrupt_set_level`.

| Property | Contract |
| --- | --- |
| Receiver | `interrupt: Borrow, kind=guest_interrupt, rights=0x20` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | `asserted: u32` |
| Results | — |
| Capability effects | `interrupt: Borrow, kind=guest_interrupt, rights=0x20` |
| User memory | — |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `memory_inspector_read_statistics`

Object-local method `memory_inspector_read_statistics`.

| Property | Contract |
| --- | --- |
| Receiver | `inspector: Borrow, kind=memory_inspector, rights=0x8` |
| Operation | 257 |
| Feature | `Core` |
| Arguments | `topic: u32`, `index: u32`, `statistics: user_address`, `statistics_size: byte_count` |
| Results | `supported_size: byte_count` |
| Capability effects | `inspector: Borrow, kind=memory_inspector, rights=0x8` |
| User memory | `statistics: Write, len=statistics_size bytes, max-bytes=4096; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Object` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `process_builder_retain`

Object-local method `process_builder_retain`.

| Property | Contract |
| --- | --- |
| Receiver | `builder: Borrow, kind=process_builder, rights=0x2000000020` |
| Operation | 264 |
| Feature | `Core` |
| Arguments | `max_bytes: byte_count`, `timeout_ns: u64` |
| Results | `handle: handle` |
| Capability effects | `builder: Borrow, kind=process_builder, rights=0x2000000020`, `handle: produce, kind=process_retirement, fixed=0x420000000e` |
| User memory | — |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `process_retirement_get_info`

Object-local method `process_retirement_get_info`.

| Property | Contract |
| --- | --- |
| Receiver | `object: Borrow, kind=process_retirement, rights=0x8` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | `output: user_address`, `output_size: byte_count` |
| Results | `supported_size: byte_count` |
| Capability effects | `object: Borrow, kind=process_retirement, rights=0x8` |
| User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=process_retirement_info; order=0` |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `process_retirement_open_view`

Object-local method `process_retirement_open_view`.

| Property | Contract |
| --- | --- |
| Receiver | `object: Borrow, kind=process_retirement, rights=0x200000000` |
| Operation | 257 |
| Feature | `Core` |
| Arguments | — |
| Results | `handle: handle` |
| Capability effects | `object: Borrow, kind=process_retirement, rights=0x200000000`, `handle: produce, kind=crash_view, fixed=0x1b` |
| User memory | — |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `process_retirement_release`

Object-local method `process_retirement_release`.

| Property | Contract |
| --- | --- |
| Receiver | `object: Borrow, kind=process_retirement, rights=0x4000000000` |
| Operation | 258 |
| Feature | `Core` |
| Arguments | — |
| Results | — |
| Capability effects | `object: Borrow, kind=process_retirement, rights=0x4000000000` |
| User memory | — |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `crash_view_get_info`

Object-local method `crash_view_get_info`.

| Property | Contract |
| --- | --- |
| Receiver | `object: Borrow, kind=crash_view, rights=0x8` |
| Operation | 256 |
| Feature | `Core` |
| Arguments | `output: user_address`, `output_size: byte_count` |
| Results | `supported_size: byte_count` |
| Capability effects | `object: Borrow, kind=crash_view, rights=0x8` |
| User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=process_retirement_info; order=0` |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `crash_view_read`

Object-local method `crash_view_read`.

| Property | Contract |
| --- | --- |
| Receiver | `object: Borrow, kind=crash_view, rights=0x10` |
| Operation | 257 |
| Feature | `Core` |
| Arguments | `address: u64`, `output: user_address`, `length: byte_count` |
| Results | `actual_bytes: byte_count` |
| Capability effects | `object: Borrow, kind=crash_view, rights=0x10` |
| User memory | `output: Write, len=length bytes, max-bytes=65536; order=0` |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `crash_view_thread`

Object-local method `crash_view_thread`.

| Property | Contract |
| --- | --- |
| Receiver | `object: Borrow, kind=crash_view, rights=0x10` |
| Operation | 258 |
| Feature | `Core` |
| Arguments | `index: u64`, `output: user_address`, `output_size: byte_count` |
| Results | `supported_size: byte_count`, `next_index: u64` |
| Capability effects | `object: Borrow, kind=crash_view, rights=0x10` |
| User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=crash_thread; order=0` |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `crash_view_mapping`

Object-local method `crash_view_mapping`.

| Property | Contract |
| --- | --- |
| Receiver | `object: Borrow, kind=crash_view, rights=0x10` |
| Operation | 259 |
| Feature | `Core` |
| Arguments | `index: u64`, `output: user_address`, `output_size: byte_count` |
| Results | `supported_size: byte_count`, `next_index: u64` |
| Capability effects | `object: Borrow, kind=crash_view, rights=0x10` |
| User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=crash_mapping; order=0` |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `crash_view_handle`

Object-local method `crash_view_handle`.

| Property | Contract |
| --- | --- |
| Receiver | `object: Borrow, kind=crash_view, rights=0x10` |
| Operation | 260 |
| Feature | `Core` |
| Arguments | `index: u64`, `output: user_address`, `output_size: byte_count` |
| Results | `supported_size: byte_count`, `next_index: u64` |
| Capability effects | `object: Borrow, kind=crash_view, rights=0x10` |
| User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=crash_handle; order=0` |
| Execution | `blocking=MayBlock, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `thread_set_current_name`

Object-local method `thread_set_current_name`.

| Property | Contract |
| --- | --- |
| Receiver | `receiver: Borrow, kind=execution_context, rights=0x20` |
| Operation | 265 |
| Feature | `Core` |
| Arguments | `name: user_address`, `name_size: byte_count` |
| Results | — |
| Capability effects | `receiver: Borrow, kind=execution_context, rights=0x20` |
| User memory | `name: Read, len=name_size bytes, max-bytes=64; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Task` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `device_dma_fault_info`

Object-local method `device_dma_fault_info`.

| Property | Contract |
| --- | --- |
| Receiver | `device: Borrow, kind=physical_device, rights=0x8008` |
| Operation | 262 |
| Feature | `Core` |
| Arguments | `output: user_address`, `output_size: byte_count` |
| Results | `supported_size: byte_count` |
| Capability effects | `device: Borrow, kind=physical_device, rights=0x8008` |
| User memory | `output: Write, len=output_size bytes, max-bytes=4096, record=device_dma_fault_info; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `native_block_transfer`

Object-local method `native_block_transfer`.

| Property | Contract |
| --- | --- |
| Receiver | `block: Borrow, kind=native_block, rights=0x0` |
| Operation | 257 |
| Feature | `Core` |
| Arguments | `operation: u32`, `first_sector: u64`, `buffer: user_address`, `length: byte_count` |
| Results | — |
| Capability effects | `block: Borrow, kind=native_block, rights=0x0` |
| User memory | `buffer: ReadWrite, len=length bytes, max-bytes=524352; order=0` |
| Execution | `blocking=MayBlock, cancellation=Explicit, restart=Never, completion=Returns, flags=None` |
| Audit | `Capability` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `thread_get_current_scheduling`

Object-local method `thread_get_current_scheduling`.

| Property | Contract |
| --- | --- |
| Receiver | `receiver: Borrow, kind=execution_context, rights=0x8` |
| Operation | 266 |
| Feature | `Core` |
| Arguments | `configuration: user_address` |
| Results | — |
| Capability effects | `receiver: Borrow, kind=execution_context, rights=0x8` |
| User memory | `configuration: Write, fixed-bytes=64, record=scheduling_parameters; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Task` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `thread_set_current_scheduling`

Object-local method `thread_set_current_scheduling`.

| Property | Contract |
| --- | --- |
| Receiver | `receiver: Borrow, kind=execution_context, rights=0x0` |
| Operation | 267 |
| Feature | `Core` |
| Arguments | `configuration: user_address` |
| Results | — |
| Capability effects | `receiver: Borrow, kind=execution_context, rights=0x0` |
| User memory | `configuration: Read, fixed-bytes=64, record=scheduling_parameters; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Task` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `thread_get_scheduling`

Object-local method `thread_get_scheduling`.

| Property | Contract |
| --- | --- |
| Receiver | `receiver: Borrow, kind=thread, rights=0x8` |
| Operation | 258 |
| Feature | `Core` |
| Arguments | `configuration: user_address` |
| Results | — |
| Capability effects | `receiver: Borrow, kind=thread, rights=0x8` |
| User memory | `configuration: Write, fixed-bytes=64, record=scheduling_parameters; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Task` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `thread_set_scheduling`

Object-local method `thread_set_scheduling`.

| Property | Contract |
| --- | --- |
| Receiver | `receiver: Borrow, kind=thread, rights=0x0` |
| Operation | 259 |
| Feature | `Core` |
| Arguments | `configuration: user_address` |
| Results | — |
| Capability effects | `receiver: Borrow, kind=thread, rights=0x0` |
| User memory | `configuration: Read, fixed-bytes=64, record=scheduling_parameters; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Task` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `virtual_cpu_get_scheduling`

Object-local method `virtual_cpu_get_scheduling`.

| Property | Contract |
| --- | --- |
| Receiver | `receiver: Borrow, kind=virtual_cpu, rights=0x8` |
| Operation | 261 |
| Feature | `Core` |
| Arguments | `configuration: user_address` |
| Results | — |
| Capability effects | `receiver: Borrow, kind=virtual_cpu, rights=0x8` |
| User memory | `configuration: Write, fixed-bytes=64, record=scheduling_parameters; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Task` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `virtual_cpu_set_scheduling`

Object-local method `virtual_cpu_set_scheduling`.

| Property | Contract |
| --- | --- |
| Receiver | `receiver: Borrow, kind=virtual_cpu, rights=0x0` |
| Operation | 262 |
| Feature | `Core` |
| Arguments | `configuration: user_address` |
| Results | — |
| Capability effects | `receiver: Borrow, kind=virtual_cpu, rights=0x0` |
| User memory | `configuration: Read, fixed-bytes=64, record=scheduling_parameters; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Task` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

### `process_builder_set_scheduling`

Object-local method `process_builder_set_scheduling`.

| Property | Contract |
| --- | --- |
| Receiver | `receiver: Borrow, kind=process_builder, rights=0x0` |
| Operation | 265 |
| Feature | `Core` |
| Arguments | `configuration: user_address`, `delegated_rights: rights` |
| Results | — |
| Capability effects | `receiver: Borrow, kind=process_builder, rights=0x0` |
| User memory | `configuration: Read, fixed-bytes=64, record=scheduling_parameters; order=0` |
| Execution | `blocking=Never, cancellation=None, restart=Never, completion=Returns, flags=None` |
| Audit | `Task` |

Arguments and auxiliary results follow the listed register order; neither list includes the receiver or signed status.

Auxiliary results are valid only on `ok`, except for the explicitly listed failure statuses.

## Public records

### `trace_config`

Minimum prefix: 56 bytes; size: 56 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `categories` | `u64` |
| 8 | `duration_ms` | `u64` |
| 16 | `capacity` | `u64` |
| 24 | `cpu_mask` | `bytes[32]` |

### `trace_info`

Minimum prefix: 160 bytes; size: 160 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `generation` | `u64` |
| 8 | `started_ns` | `u64` |
| 16 | `deadline_ns` | `u64` |
| 24 | `categories` | `u64` |
| 32 | `cpu_count` | `u64` |
| 40 | `capacity` | `u64` |
| 48 | `maximum_capacity` | `u64` |
| 56 | `running` | `u64` |
| 64 | `cpu_mask` | `bytes[32]` |
| 96 | `revision` | `bytes[64]` |

### `trace_record`

Minimum prefix: 328 bytes; size: 328 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `sequence` | `u64` |
| 8 | `timestamp_ns` | `u64` |
| 16 | `category` | `u64` |
| 24 | `field_count` | `u64` |
| 32 | `values` | `bytes[32]` |
| 64 | `name` | `bytes[64]` |
| 128 | `fields` | `bytes[96]` |
| 224 | `source` | `bytes[96]` |
| 320 | `line` | `u64` |

### `trace_batch`

Minimum prefix: 2656 bytes; size: 2656 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `count` | `u64` |
| 8 | `next` | `u64` |
| 16 | `overwritten` | `u64` |
| 24 | `incomplete` | `u64` |
| 32 | `records` | `bytes[2624]` |

### `trace_thread`

Minimum prefix: 72 bytes; size: 72 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `thread` | `u64` |
| 8 | `name` | `bytes[64]` |

### `scheduling_parameters`

Minimum prefix: 64 bytes; size: 64 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `policy` | `u32` |
| 4 | `band` | `u32` |
| 8 | `role` | `u32` |
| 12 | `weight` | `u32` |
| 16 | `fifo_priority` | `u32` |
| 20 | `fair_backend` | `u32` |
| 24 | `quantum_ns` | `u64` |
| 32 | `budget_ns` | `u64` |
| 40 | `period_ns` | `u64` |
| 48 | `burst_ns` | `u64` |
| 56 | `reserved` | `u64` |

### `cpu_profile_info`

Minimum prefix: 144 bytes; size: 144 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `generation` | `u64` |
| 8 | `period` | `u64` |
| 16 | `deadline_ns` | `u64` |
| 24 | `cpu_count` | `u64` |
| 32 | `running_generation` | `u64` |
| 40 | `armed_mask` | `u64` |
| 48 | `capacity` | `u64` |
| 56 | `depth` | `u64` |
| 64 | `active_cpus` | `u64` |
| 72 | `dropped` | `u64` |
| 80 | `revision` | `bytes[64]` |

### `cpu_profile_sample`

Minimum prefix: 192 bytes; size: 192 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `sequence` | `u64` |
| 8 | `timestamp_ns` | `u64` |
| 16 | `cpu` | `u64` |
| 24 | `world` | `u64` |
| 32 | `depth` | `u64` |
| 40 | `flags` | `u64` |
| 48 | `dropped` | `u64` |
| 56 | `period` | `u64` |
| 64 | `frames` | `bytes[128]` |

### `process_retirement_info`

Minimum prefix: 104 bytes; size: 104 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `size` | `u32` |
| 4 | `phase` | `u32` |
| 8 | `flags` | `u32` |
| 12 | `release_reason` | `u32` |
| 16 | `process_koid` | `u64` |
| 24 | `deadline_ns` | `u64` |
| 32 | `retained_bytes` | `u64` |
| 40 | `memory_limit` | `u64` |
| 48 | `terminal_reason` | `u32` |
| 52 | `fault_class` | `u32` |
| 56 | `exit_status` | `i64` |
| 64 | `fault_pc` | `u64` |
| 72 | `fault_address` | `u64` |
| 80 | `fault_syndrome` | `u64` |
| 88 | `thread_count` | `u32` |
| 92 | `mapping_count` | `u32` |
| 96 | `handle_count` | `u32` |
| 100 | `reserved` | `u32` |

### `crash_thread`

Minimum prefix: 840 bytes; size: 840 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `size` | `u32` |
| 4 | `machine` | `u32` |
| 8 | `flags` | `u32` |
| 12 | `reserved` | `u32` |
| 16 | `thread_koid` | `u64` |
| 24 | `pc` | `u64` |
| 32 | `sp` | `u64` |
| 40 | `tls` | `u64` |
| 48 | `processor_state` | `u64` |
| 56 | `general` | `bytes[256]` |
| 312 | `vector` | `bytes[512]` |
| 824 | `fp_control` | `bytes[16]` |

### `crash_mapping`

Minimum prefix: 48 bytes; size: 48 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `size` | `u32` |
| 4 | `flags` | `u32` |
| 8 | `base` | `u64` |
| 16 | `length` | `u64` |
| 24 | `object_offset` | `u64` |
| 32 | `object_koid` | `u64` |
| 40 | `permissions` | `u32` |
| 44 | `kind` | `u32` |

### `crash_handle`

Minimum prefix: 40 bytes; size: 40 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `size` | `u32` |
| 4 | `kind` | `u32` |
| 8 | `value` | `u64` |
| 16 | `koid` | `u64` |
| 24 | `rights` | `u64` |
| 32 | `flags` | `u32` |
| 36 | `reserved` | `u32` |

### `memory_heap_statistics`

Minimum prefix: 176 bytes; size: 176 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `captured_at_ns` | `u64` |
| 8 | `live_objects` | `u64` |
| 16 | `live_slab_objects` | `u64` |
| 24 | `live_large_objects` | `u64` |
| 32 | `requested_bytes` | `u64` |
| 40 | `peak_objects` | `u64` |
| 48 | `peak_requested_bytes` | `u64` |
| 56 | `allocations` | `u64` |
| 64 | `failures` | `u64` |
| 72 | `cached_objects` | `u64` |
| 80 | `cache_hits` | `u64` |
| 88 | `cache_misses` | `u64` |
| 96 | `cache_refills` | `u64` |
| 104 | `cache_drains` | `u64` |
| 112 | `pressure_reclaims` | `u64` |
| 120 | `reclaimed_objects` | `u64` |
| 128 | `cache_cpus` | `u64` |
| 136 | `profiling_enabled` | `u64` |
| 144 | `page_allocations` | `u64` |
| 152 | `page_failures` | `u64` |
| 160 | `page_deallocations` | `u64` |
| 168 | `peak_allocated_pages` | `u64` |

### `memory_buddy_statistics`

Minimum prefix: 24 bytes; size: 24 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `captured_at_ns` | `u64` |
| 8 | `order` | `u64` |
| 16 | `free_blocks` | `u64` |

### `memory_owner_statistics`

Minimum prefix: 48 bytes; size: 48 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `captured_at_ns` | `u64` |
| 8 | `resident_pages` | `u64` |
| 16 | `peak_pages` | `u64` |
| 24 | `allocations` | `u64` |
| 32 | `failures` | `u64` |
| 40 | `deallocations` | `u64` |

### `memory_file_cache_statistics`

Minimum prefix: 120 bytes; size: 120 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `captured_at_ns` | `u64` |
| 8 | `payload_pages` | `u64` |
| 16 | `metadata_slab_pages` | `u64` |
| 24 | `metadata_large_pages` | `u64` |
| 32 | `index_capacity` | `u64` |
| 40 | `clean_entries` | `u64` |
| 48 | `loading_entries` | `u64` |
| 56 | `live_payloads` | `u64` |
| 64 | `metadata_budget_pages` | `u64` |
| 72 | `table_present` | `u64` |
| 80 | `admission_enabled` | `u64` |
| 88 | `admission_paused` | `u64` |
| 96 | `maintenance_pending` | `u64` |
| 104 | `maintenance_running` | `u64` |
| 112 | `growth_requested` | `u64` |

### `memory_heap_class_statistics`

Minimum prefix: 88 bytes; size: 88 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `captured_at_ns` | `u64` |
| 8 | `pool` | `u64` |
| 16 | `object_size` | `u64` |
| 24 | `pages` | `u64` |
| 32 | `capacity` | `u64` |
| 40 | `reserved` | `u64` |
| 48 | `cached` | `u64` |
| 56 | `live` | `u64` |
| 64 | `requested_bytes` | `u64` |
| 72 | `allocations` | `u64` |
| 80 | `deallocations` | `u64` |

### `filesystem_root_switch`

Minimum prefix: 48 bytes; size: 48 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `new_root` | `u64` |
| 8 | `new_root_length` | `u64` |
| 16 | `put_old` | `u64` |
| 24 | `put_old_length` | `u64` |
| 32 | `moves` | `u64` |
| 40 | `move_count` | `u64` |

### `filesystem_mount_move`

Minimum prefix: 32 bytes; size: 32 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `source` | `u64` |
| 8 | `source_length` | `u64` |
| 16 | `target` | `u64` |
| 24 | `target_length` | `u64` |

### `object_details`

Minimum prefix: 88 bytes; size: 88 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `koid` | `u64` |
| 8 | `object_kind` | `u32` |
| 12 | `record_kind` | `u32` |
| 16 | `next_cursor` | `u64` |
| 24 | `payload` | `bytes[64]` |

### `device_dma_fault_info`

Minimum prefix: 40 bytes; size: 40 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `timestamp_ns` | `u64` |
| 8 | `address` | `u64` |
| 16 | `stream_id` | `u32` |
| 20 | `source` | `u32` |
| 24 | `scope` | `u32` |
| 28 | `code` | `u32` |
| 32 | `flags` | `u32` |
| 36 | `reserved` | `u32` |

### `device_profile_info`

Minimum prefix: 32 bytes; size: 32 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `profile` | `u32` |
| 4 | `interrupt_count` | `u32` |
| 8 | `resource_count` | `u32` |
| 12 | `pci_identity` | `u32` |
| 16 | `dma_bus_offset` | `u64` |
| 24 | `aperture_size` | `u64` |

### `device_firmware_query`

Minimum prefix: 24 bytes; size: 24 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `node` | `u32` |
| 4 | `field` | `u32` |
| 8 | `name_address` | `u64` |
| 16 | `name_length` | `u64` |

### `device_firmware_info`

Minimum prefix: 32 bytes; size: 32 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `flags` | `u32` |
| 4 | `register_count` | `u32` |
| 8 | `irq_number` | `u32` |
| 12 | `irq_trigger` | `u32` |
| 16 | `reserved0` | `u64` |
| 24 | `reserved1` | `u64` |

### `device_bundle_entry`

Minimum prefix: 16 bytes; size: 16 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `node` | `u32` |
| 4 | `resource` | `u32` |
| 8 | `offset` | `u64` |

### `device_resource_info`

Minimum prefix: 32 bytes; size: 32 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `kind` | `u32` |
| 4 | `flags` | `u32` |
| 8 | `offset` | `u64` |
| 16 | `length` | `u64` |
| 24 | `bus_address` | `u64` |

### `physical_device_info`

Minimum prefix: 16 bytes; size: 16 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `device_id` | `u32` |
| 4 | `transport_version` | `u32` |
| 8 | `mmio_size` | `u64` |

### `dma_extent`

Minimum prefix: 16 bytes; size: 16 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `physical_base` | `u64` |
| 8 | `length` | `u64` |

### `virtual_cpu_mmio_request`

Minimum prefix: 48 bytes; size: 48 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `id` | `u64` |
| 8 | `device` | `u64` |
| 16 | `address` | `u64` |
| 24 | `value` | `u64` |
| 32 | `operation` | `u32` |
| 36 | `width` | `u32` |
| 40 | `reserved` | `u64` |

### `private_mapping`

Minimum prefix: 48 bytes; size: 48 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `source_offset` | `u64` |
| 8 | `source_length` | `u64` |
| 16 | `address` | `u64` |
| 24 | `size` | `u64` |
| 32 | `data_offset` | `u64` |
| 40 | `permissions` | `u32` |
| 44 | `mode` | `u32` |

### `virtual_machine_platform_info`

Minimum prefix: 32 bytes; size: 32 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `architecture` | `u32` |
| 4 | `platform_profile` | `u32` |
| 8 | `counter_frequency_hz` | `u64` |
| 16 | `riscv_isa` | `u64` |
| 24 | `aarch64_gic_version` | `u64` |

### `file_metadata`

Minimum prefix: 112 bytes; size: 112 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `filesystem_id` | `u64` |
| 8 | `mount_id` | `u64` |
| 16 | `node_id` | `u64` |
| 24 | `size` | `u64` |
| 32 | `mode` | `u32` |
| 36 | `kind` | `u32` |
| 40 | `valid_times` | `u32` |
| 44 | `reserved` | `u32` |
| 48 | `accessed_seconds` | `i64` |
| 56 | `accessed_nanoseconds` | `u32` |
| 60 | `accessed_reserved` | `u32` |
| 64 | `modified_seconds` | `i64` |
| 72 | `modified_nanoseconds` | `u32` |
| 76 | `modified_reserved` | `u32` |
| 80 | `created_seconds` | `i64` |
| 88 | `created_nanoseconds` | `u32` |
| 92 | `created_reserved` | `u32` |
| 96 | `changed_seconds` | `i64` |
| 104 | `changed_nanoseconds` | `u32` |
| 108 | `changed_reserved` | `u32` |

### `file_metadata_update`

Minimum prefix: 40 bytes; size: 40 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `mask` | `u32` |
| 4 | `mode` | `u32` |
| 8 | `accessed_seconds` | `i64` |
| 16 | `accessed_nanoseconds` | `u32` |
| 20 | `accessed_reserved` | `u32` |
| 24 | `modified_seconds` | `i64` |
| 32 | `modified_nanoseconds` | `u32` |
| 36 | `modified_reserved` | `u32` |

### `wait_set_event`

Minimum prefix: 24 bytes; size: 24 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `registration` | `u64` |
| 8 | `signals` | `u64` |
| 16 | `sequence` | `u64` |

### `handle_info`

Minimum prefix: 16 bytes; size: 16 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `object_kind` | `u32` |
| 4 | `flags` | `u32` |
| 8 | `rights` | `u64` |

### `object_basic_info`

Minimum prefix: 16 bytes; size: 16 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `koid` | `u64` |
| 8 | `object_kind` | `u32` |
| 12 | `reserved` | `u32` |

### `object_wait_item`

Minimum prefix: 16 bytes; size: 16 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `handle` | `u64` |
| 8 | `signals` | `u64` |

### `process_info`

Minimum prefix: 32 bytes; size: 32 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `phase` | `u32` |
| 4 | `terminal_reason` | `u32` |
| 8 | `detail0` | `u64` |
| 16 | `detail1` | `u64` |
| 24 | `reserved` | `u64` |

### `capability_disposition`

Minimum prefix: 24 bytes; size: 24 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `handle` | `u64` |
| 8 | `rights` | `u64` |
| 16 | `expected_kind` | `u32` |
| 20 | `operation` | `u32` |

### `capability_receive_slot`

Minimum prefix: 24 bytes; size: 24 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `handle` | `u64` |
| 8 | `rights` | `u64` |
| 16 | `expected_kind` | `u32` |
| 20 | `flags` | `u32` |

### `loader_startup`

Minimum prefix: 96 bytes; size: 96 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `size` | `u32` |
| 4 | `handle_count` | `u32` |
| 8 | `data_size` | `u32` |
| 12 | `flags` | `u32` |
| 16 | `executable` | `u64` |
| 24 | `root_vmar` | `u64` |
| 32 | `stack_vmar` | `u64` |
| 40 | `stack_base` | `u64` |
| 48 | `stack_size` | `u64` |
| 56 | `loader_base` | `u64` |
| 64 | `loader_size` | `u64` |
| 72 | `execution_context` | `u64` |
| 80 | `object_factory` | `u64` |
| 88 | `system_info` | `u64` |

### `startup_handle`

Minimum prefix: 16 bytes; size: 16 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `purpose` | `u32` |
| 4 | `flags` | `u32` |
| 8 | `handle` | `u64` |

### `task_process`

Minimum prefix: 96 bytes; size: 96 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `koid` | `u64` |
| 8 | `phase` | `u32` |
| 12 | `terminal_reason` | `u32` |
| 16 | `pending_threads` | `u32` |
| 20 | `active_threads` | `u32` |
| 24 | `name_length` | `u32` |
| 28 | `reserved` | `u32` |
| 32 | `name` | `bytes[64]` |

### `task_thread`

Minimum prefix: 104 bytes; size: 104 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `koid` | `u64` |
| 8 | `process_koid` | `u64` |
| 16 | `role` | `u32` |
| 20 | `registry_phase` | `u32` |
| 24 | `name_length` | `u32` |
| 28 | `reserved` | `u32` |
| 32 | `runtime_ns` | `u64` |
| 40 | `name` | `bytes[64]` |

### `memory_observation`

Minimum prefix: 144 bytes; size: 144 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `captured_at_ns` | `u64` |
| 8 | `page_size` | `u64` |
| 16 | `total_bytes` | `u64` |
| 24 | `managed_bytes` | `u64` |
| 32 | `free_bytes` | `u64` |
| 40 | `kernel_bytes` | `u64` |
| 48 | `slab_bytes` | `u64` |
| 56 | `large_heap_bytes` | `u64` |
| 64 | `page_table_bytes` | `u64` |
| 72 | `vmo_bytes` | `u64` |
| 80 | `guest_loaned_bytes` | `u64` |
| 88 | `guest_private_bytes` | `u64` |
| 96 | `file_cache_bytes` | `u64` |
| 104 | `slab_reclaimable_bytes` | `u64` |
| 112 | `slab_sample_complete` | `u64` |
| 120 | `file_cache_reclaimable_bytes` | `u64` |
| 128 | `file_cache_sample_complete` | `u64` |
| 136 | `ramfs_capacity_bytes` | `u64` |

### `cpu_observation`

Minimum prefix: 64 bytes; size: 64 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `captured_at_ns` | `u64` |
| 8 | `reserved_clock` | `u64` |
| 16 | `online_cpus` | `u64` |
| 24 | `idle_ns` | `u64` |
| 32 | `kernel_thread_ns` | `u64` |
| 40 | `user_thread_ns` | `u64` |
| 48 | `vcpu_ns` | `u64` |
| 56 | `reserved` | `u64` |

### `object_inspection`

Minimum prefix: 104 bytes; size: 104 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `koid` | `u64` |
| 8 | `object_kind` | `u32` |
| 12 | `handle_state` | `u32` |
| 16 | `active_handles` | `u64` |
| 24 | `supported_rights` | `u64` |
| 32 | `strong_references` | `u64` |
| 40 | `kernel_service_references` | `u64` |
| 48 | `scheduler_references` | `u64` |
| 56 | `operation_references` | `u64` |
| 64 | `user_authority_references` | `u64` |
| 72 | `publication_references` | `u64` |
| 80 | `diagnostic_references` | `u64` |
| 88 | `retirement_references` | `u64` |
| 96 | `vm_device_binding_references` | `u64` |

### `handle_inspection`

Minimum prefix: 40 bytes; size: 40 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `process_koid` | `u64` |
| 8 | `handle` | `u64` |
| 16 | `object_koid` | `u64` |
| 24 | `rights` | `u64` |
| 32 | `object_kind` | `u32` |
| 36 | `flags` | `u32` |

### `directory_entry`

Minimum prefix: 280 bytes; size: 280 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `size` | `u64` |
| 8 | `mode` | `u32` |
| 12 | `kind` | `u32` |
| 16 | `name_length` | `u32` |
| 20 | `reserved` | `u32` |
| 24 | `name` | `bytes[256]` |

### `file_info`

Minimum prefix: 40 bytes; size: 40 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `filesystem_id` | `u64` |
| 8 | `mount_id` | `u64` |
| 16 | `node_id` | `u64` |
| 24 | `size` | `u64` |
| 32 | `mode` | `u32` |
| 36 | `reserved` | `u32` |

### `directory_info`

Minimum prefix: 32 bytes; size: 32 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `filesystem_id` | `u64` |
| 8 | `mount_id` | `u64` |
| 16 | `node_id` | `u64` |
| 24 | `mode` | `u32` |
| 28 | `reserved` | `u32` |

### `virtual_machine_configuration`

Minimum prefix: 32 bytes; size: 32 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `guest_physical_base` | `u64` |
| 8 | `memory_size` | `u64` |
| 16 | `vcpu_count` | `u32` |
| 20 | `architecture` | `u32` |
| 24 | `platform_profile` | `u32` |
| 28 | `flags` | `u32` |

### `virtual_cpu_bootstrap`

Minimum prefix: 64 bytes; size: 64 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `entry` | `u64` |
| 8 | `stack` | `u64` |
| 16 | `argument0` | `u64` |
| 24 | `argument1` | `u64` |
| 32 | `argument2` | `u64` |
| 40 | `argument3` | `u64` |
| 48 | `vcpu_id` | `u32` |
| 52 | `flags` | `u32` |
| 56 | `reserved` | `u64` |

### `virtual_machine_power_request`

Minimum prefix: 40 bytes; size: 40 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `id` | `u64` |
| 8 | `vcpu` | `u32` |
| 12 | `operation` | `u32` |
| 16 | `target` | `u32` |
| 20 | `reserved` | `u32` |
| 24 | `entry` | `u64` |
| 32 | `context` | `u64` |

### `virtual_machine_info`

Minimum prefix: 32 bytes; size: 40 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `phase` | `u32` |
| 4 | `vcpu_count` | `u32` |
| 8 | `guest_physical_base` | `u64` |
| 16 | `memory_size` | `u64` |
| 24 | `architecture` | `u32` |
| 28 | `platform_profile` | `u32` |
| 32 | `resident_memory_bytes` | `u64` |

### `virtual_cpu_info`

Minimum prefix: 24 bytes; size: 32 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `vcpu_id` | `u32` |
| 4 | `phase` | `u32` |
| 8 | `scheduler_thread_id` | `u64` |
| 16 | `terminal_reason` | `u32` |
| 20 | `reserved` | `u32` |
| 24 | `host_cpu` | `u32` |
| 28 | `migration_target` | `u32` |

### `resource_limits`

Minimum prefix: 160 bytes; size: 160 bytes; alignment: 8 bytes.

| Offset | Field | Type |
| ---: | --- | --- |
| 0 | `kernel_memory_bytes` | `u64` |
| 8 | `processes` | `u64` |
| 16 | `threads` | `u64` |
| 24 | `handles` | `u64` |
| 32 | `kernel_objects` | `u64` |
| 40 | `committed_pages` | `u64` |
| 48 | `pinned_pages` | `u64` |
| 56 | `guest_pages` | `u64` |
| 64 | `ipc_messages` | `u64` |
| 72 | `ipc_bytes` | `u64` |
| 80 | `ipc_handles` | `u64` |
| 88 | `subscriptions` | `u64` |
| 96 | `timers` | `u64` |
| 104 | `virtual_machines` | `u64` |
| 112 | `virtual_cpus` | `u64` |
| 120 | `device_leases` | `u64` |
| 128 | `dma_mappings` | `u64` |
| 136 | `user_address_spaces` | `u64` |
| 144 | `user_mappings` | `u64` |
| 152 | `reserved` | `u64` |
