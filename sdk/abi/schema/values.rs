// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Grouped scalar values used by the Native wire contract.
//!
//! `enum` groups require unique discriminants, `flags` require independent bits,
//! and `constants` admit limits and sentinels with repeated numeric values.
//! These declarations create metadata, not Rust wire enums. Consumers must still
//! validate raw integers; group membership is not a granted capability.
//!
//! Group/member `///` text inside `constant_groups!` is captured as metadata.
//! Ordinary `//` comments explain maintenance or representation without changing
//! generated bindings or reference text.

use super::capabilities::{CAPABILITY_OPERATION_DUPLICATE, CAPABILITY_OPERATION_MOVE};
use super::model::{AbiConstant, ConstantGroup, ConstantGroupKind};

/// Maximum borrowed wait records in one wait-many operation.
pub(super) const OBJECT_WAIT_MANY_MAX_ITEMS: u32 = 64;

// Shared bounds keep signatures and public scalar exports in agreement.
// Sizes are bytes unless the identifier explicitly counts records/items.
pub(super) const CONSOLE_MAX_TRANSFER_BYTES: u32 = 4 * 1024;
pub(super) const VIRTUAL_SERIAL_MAX_TRANSFER_BYTES: u32 = 4 * 1024;
pub(super) const EXTENSIBLE_RECORD_MAX_BYTES: u32 = 4 * 1024;
pub(super) const DIRECTORY_ENTRY_PAGE_CAPACITY: u32 = 4;
/// Wire storage includes room for termination; the public name-length limit is 255.
pub(super) const DIRECTORY_ENTRY_NAME_CAPACITY: u32 = 256;
/// Fixed entry header plus byte-name storage; keep aligned with records.rs.
pub(super) const DIRECTORY_ENTRY_RECORD_SIZE: u16 = 24 + DIRECTORY_ENTRY_NAME_CAPACITY as u16;

// These declarations expand only to schema metadata. Enum domains use unique
// discriminants, flags use independent bits, and constants carry unrestricted
// numeric parameters. Public C/Rust names and integer widths remain unchanged.
macro_rules! constant_group_kind {
    (enum) => {
        ConstantGroupKind::Enumeration
    };
    (flags) => {
        ConstantGroupKind::Flags
    };
    (constants) => {
        ConstantGroupKind::Constants
    };
}

// Parsing remains compiler-checked Rust. The generator validates names, bit
// independence and enum uniqueness after these declarations have expanded.
macro_rules! constant_groups {
    ($(#[doc = $description:literal] $kind:ident $group:ident {
        $($(#[doc = $member_doc:literal])* $name:ident = $value:expr,)*
    })*) => {
        pub const CONSTANT_GROUPS: &[ConstantGroup] = &[
            $(ConstantGroup {
                name: stringify!($group),
                description: $description,
                kind: constant_group_kind!($kind),
                values: &[$(AbiConstant {
                    name: stringify!($name),
                    value: $value,
                    description: concat!($($member_doc, "\n",)*),
                },)*],
            },)*
        ];
    };
}

constant_groups! {
    /// Handle value classes; the remaining bits are opaque process-local data.
    constants handle_encoding {
        /// No handle is present. This value never identifies a live handle.
        handle_invalid = 0,
        /// Selects the most significant byte of a wire handle.
        handle_prefix_mask = 0xff_u64 << 56,
        /// Live handles have prefix 0x0F; all other prefixes are reserved.
        handle_prefix = 0x0f_u64 << 56,
    }

    /// Page geometry, record bounds, identity encoding and wait limits.
    constants core_limits {
        /// Native memory-management granule in bytes.
        page_size = 4096,
        /// Upper bound on a caller-supplied extensible record, including its tail.
        extensible_record_max_bytes = EXTENSIBLE_RECORD_MAX_BYTES as u64,
        /// Low slot bits of a KOID; the generation is required for identity.
        koid_slot_mask = u32::MAX as u64,
        /// Wait without a deadline; finite deadlines use absolute monotonic nanoseconds.
        deadline_infinite = u64::MAX,
        object_wait_many_max_items = OBJECT_WAIT_MANY_MAX_ITEMS as u64,
    }

    /// Selectors for system configuration queries.
    enum system_config {
        system_config_page_size = 1,
        system_config_application_address_limit = 2,
    }

    /// Executable identifiers and kernel-to-loader transport bounds.
    constants startup_limits {
        /// `HypeR`'s ELF OSABI byte; this is not a Linux or generic System V image.
        elf_osabi = 63,
        elf_abi_version = 0,
        process_startup_data_max_bytes = 16 * 1024,
        startup_max_handles = 256,
        /// Maximum UTF-8 boot-argument payload, excluding its length prefix.
        boot_command_line_max_bytes = 512,
    }

    /// Kernel-to-loader startup message tags; payload semantics belong to userspace.
    enum loader_startup {
        loader_startup_reply = 1,
    }

    /// Purposes assigned to capabilities in the process startup table.
    enum startup_handle_purpose {
        startup_handle_purpose_resource_domain = 1,
        startup_handle_purpose_task_group = 2,
        startup_handle_purpose_task_factory = 3,
        startup_handle_purpose_executable_authority = 4,
        startup_handle_purpose_root_vmar = 5,
        startup_handle_purpose_console = 6,
        startup_handle_purpose_root_directory = 7,
        startup_handle_purpose_task_inspector = 8,
        startup_handle_purpose_object_inspector = 9,
        startup_handle_purpose_dynamic_library_directory = 10,
        startup_handle_purpose_memory_inspector = 11,
        startup_handle_purpose_cpu_inspector = 12,
        startup_handle_purpose_virtual_machine_creation_authority = 13,
        startup_handle_purpose_device_assignment_authority = 14,
        startup_handle_purpose_initial_stack_vmar = 15,
        startup_handle_purpose_execution_context = 16,
        startup_handle_purpose_object_factory = 17,
        startup_handle_purpose_system_info = 18,
        /// Unconsumed boot arguments: immutable VMO with READ | INSPECT rights.
        /// Contains a little-endian u64 byte length followed by that many UTF-8
        /// bytes (no NUL), bounded by `BOOT_COMMAND_LINE_MAX_BYTES`. The kernel
        /// passes unrecognized arguments to its initial process. Each userspace
        /// boot stage may pass its own remainder to the successor under this purpose.
        startup_handle_purpose_boot_command_line = 19,
    }

    /// Process lifecycle phases.
    enum process_phase {
        process_phase_prepared = 0,
        process_phase_created = 1,
        process_phase_running = 2,
        process_phase_stopping = 3,
        process_phase_stopped = 4,
        process_phase_retiring = 5,
        process_phase_retired = 6,
    }

    /// Reasons a process stopped.
    enum process_terminal {
        process_terminal_none = 0,
        process_terminal_requested = 1,
        process_terminal_thread_exited = 2,
        process_terminal_process_exited = 3,
        process_terminal_last_thread_exited = 4,
        process_terminal_fault = 5,
        process_terminal_task_group_stop = 6,
    }

    /// Scheduler thread roles reported by inspection.
    enum thread_role {
        thread_role_bootstrap = 1,
        thread_role_idle = 2,
        thread_role_kernel = 3,
        thread_role_user = 4,
        thread_role_vcpu = 5,
    }

    /// Scheduling algorithms; these do not identify the thread execution payload.
    enum scheduling_policy {
        scheduling_policy_fair = 1,
        scheduling_policy_budget = 2,
        scheduling_policy_fifo = 3,
    }

    /// Eligibility for CPU-local priority and minimum-progress allowances.
    enum scheduling_role {
        scheduling_role_ordinary = 0,
        scheduling_role_priority = 1,
        scheduling_role_protected = 2,
    }

    /// Fair queue implementations supported by the selected kernel image.
    enum scheduling_fair_backend {
        scheduling_fair_backend_stride = 0,
        scheduling_fair_backend_round_robin = 1,
        scheduling_fair_backend_default = 2,
    }

    /// Thread registry membership states.
    enum thread_registry {
        thread_registry_resident = 1,
        thread_registry_retiring = 2,
    }

    /// Task names, affinity bounds and inspection page sizes.
    constants task_limits {
        /// UTF-8 bytes excluding any NUL terminator; control characters are rejected.
        thread_name_max_bytes = 64,
        process_name_max_bytes = 64,
        process_affinity_max_words = 4,
        process_affinity_max_cpus = 256,
        task_inspector_process_page_capacity = 8,
        task_inspector_thread_page_capacity = 8,
        object_inspector_object_page_capacity = 8,
        object_inspector_handle_page_capacity = 8,
    }

    /// Payload tags for detailed object inspection.
    enum object_detail {
        object_detail_empty = 0,
        object_detail_thread = 1,
        object_detail_vmar = 2,
        object_detail_mapping = 3,
        object_detail_channel = 4,
        object_detail_device = 5,
        object_detail_device_resource = 6,
    }

    /// Userspace handle publication states.
    enum object_handle_state {
        object_handle_state_unpublished = 1,
        object_handle_state_active = 2,
        object_handle_state_retired = 3,
    }

    /// Retained process lifecycle phases.
    enum process_retirement {
        process_retirement_prepared = 0,
        process_retirement_running = 1,
        process_retirement_stopping = 2,
        process_retirement_ready = 3,
        process_retirement_released = 4,
    }

    /// Reasons retained process state was released.
    enum process_retirement_release {
        process_retirement_release_none = 0,
        process_retirement_release_explicit = 1,
        process_retirement_release_owner_lost = 2,
        process_retirement_release_deadline = 3,
        process_retirement_release_aborted = 4,
    }

    /// Incomplete sections of retained crash state.
    flags crash_partial {
        crash_partial_memory = 1,
        crash_partial_threads = 2,
        crash_partial_handles = 4,
        crash_partial_mappings = 8,
    }

    /// Retention and exclusion flags for a crash mapping.
    flags crash_mapping_status {
        crash_mapping_retained = 1,
        crash_mapping_partial = 2,
        crash_mapping_excluded_shared = 4,
        crash_mapping_excluded_file = 8,
    }

    /// Backing kinds reported for a crash mapping.
    enum crash_mapping_kind {
        crash_mapping_anonymous = 1,
        crash_mapping_private_file = 2,
        crash_mapping_shared = 3,
    }

    /// Maximum bytes returned by one crash-memory read.
    constants crash_limits {
        crash_read_max_bytes = 65536,
    }

    /// Selectors for detailed memory statistics.
    enum memory_statistics {
        memory_statistics_heap = 1,
        memory_statistics_buddy = 2,
        memory_statistics_owner = 3,
        memory_statistics_file_cache = 4,
        memory_statistics_heap_class = 5,
    }

    /// Dimensions of memory statistics records.
    constants memory_statistics_dimensions {
        memory_statistics_buddy_order_count = 19,
        memory_statistics_owner_count = 5,
        memory_statistics_heap_class_count = 18,
    }

    /// Private mapping population policies.
    enum private_mapping {
        /// Share source pages until the mapping is written; modifications remain private.
        private_mapping_copy_on_write = 0,
        /// Populate private pages immediately instead of deferring copying to write faults.
        private_mapping_eager = 1,
    }

    /// Permissions which may be combined for a mapping.
    flags vmar_permission {
        vmar_permission_read = 1 << 0,
        vmar_permission_write = 1 << 1,
        vmar_permission_execute = 1 << 2,
    }

    /// Options for address-space allocation.
    flags vmar_allocate {
        vmar_allocate_exact = 1,
    }

    /// Options for locking currently mapped memory.
    flags vmar_memory {
        /// Lock mappings that exist now; future mappings do not inherit the lock.
        /// Unmapping and process teardown still release the corresponding residency.
        vmar_memory_current = 1,
    }

    /// VMO size and transfer bounds.
    constants vmo_limits {
        vmo_max_size_bytes = 4 * 1024 * 1024 * 1024,
        vmo_max_transfer_bytes = 64 * 1024,
        vmo_max_contiguous_size_bytes = 128 * 1024 * 1024,
    }

    /// Ownership operations for a transferred capability.
    enum capability_disposition {
        /// Transfer ownership only when the operation commits; failures preserve the source.
        capability_disposition_move = CAPABILITY_OPERATION_MOVE as u64,
        /// Keep the source handle; requires DUPLICATE in addition to transfer authority.
        capability_disposition_duplicate = CAPABILITY_OPERATION_DUPLICATE as u64,
    }

    /// Channel message, queue and capability-transfer bounds.
    constants channel_limits {
        byte_channel_max_message_bytes = 64 * 1024,
        byte_channel_max_queued_messages = 16,
        byte_channel_max_queued_bytes = 16 * 64 * 1024,
        capability_channel_max_message_bytes = 4 * 1024,
        capability_channel_max_handles = 64,
        /// Request the source's current rights, not every right defined by the ABI.
        capability_disposition_same_rights = u64::MAX,
    }

    /// Console transfer bounds and virtual serial shared-memory layout.
    constants console_limits {
        console_max_transfer_bytes = CONSOLE_MAX_TRANSFER_BYTES as u64,
        virtual_serial_max_transfer_bytes = VIRTUAL_SERIAL_MAX_TRANSFER_BYTES as u64,
        /// Byte slots in the producer ring; unread bytes are never overwritten.
        virtual_serial_output_capacity = 65536,
        /// Byte offset of ring data after its shared atomic counter header.
        virtual_serial_output_header_bytes = 4096,
        /// Exact VMO size required by `virtual_serial_register_output`.
        virtual_serial_output_bytes = 4096 + 65536,
    }

    /// Kinds reported by directory enumeration.
    enum directory_entry_kind {
        directory_entry_kind_file = 1,
        directory_entry_kind_directory = 2,
        directory_entry_kind_symlink = 3,
        directory_entry_kind_other = 4,
    }

    /// Path, directory-page, transfer and root-switch bounds.
    constants filesystem_limits {
        directory_entry_page_capacity = DIRECTORY_ENTRY_PAGE_CAPACITY as u64,
        directory_entry_name_max_bytes = (DIRECTORY_ENTRY_NAME_CAPACITY - 1) as u64,
        directory_max_path_bytes = 4096,
        file_max_read_bytes = 2 * 1024 * 1024,
        filesystem_switch_root_max_moves = 16,
    }

    /// Physical-device assignment profiles.
    enum device_profile {
        device_profile_virtio_mmio_scsi = 1,
        device_profile_userspace = 2,
        device_profile_virtio_mmio_net = 3,
        device_profile_pci_function = 4,
    }

    /// Selectors for device identity matching.
    enum device_identity {
        device_identity_compatible = 1,
        device_identity_fdt_path = 2,
        device_identity_pci_id = 3,
    }

    /// Fields exposed by firmware inspection.
    enum device_firmware_field {
        device_firmware_field_info = 0,
        device_firmware_field_path = 1,
        device_firmware_field_compatible = 2,
        device_firmware_field_registers = 3,
        device_firmware_field_property = 4,
        device_firmware_field_property_names = 5,
    }

    /// PCI resource identifiers; BAR0 is the base of the BAR index range.
    constants device_resource_ids {
        device_resource_pci_ecam = 0x100,
        device_resource_pci_msi = 0x101,
        /// Add the zero-based BAR index to identify a PCI BAR resource.
        device_resource_pci_bar0 = 0x200,
        device_resource_pci_intx = 0x102,
    }

    /// PCI resource attributes.
    flags device_resource_flags {
        device_resource_memory_64 = 1,
        device_resource_prefetchable = 2,
    }

    /// Device discovery, assignment and PCI aperture bounds.
    constants device_limits {
        device_firmware_max_bytes = 65536,
        device_firmware_name_max_bytes = 128,
        device_bundle_max_entries = 8,
        device_pci_aperture_size = 0x80_0000,
        device_assignment_max_devices = 4,
    }

    /// DMA isolation mechanisms reporting a fault.
    enum dma_fault_source {
        dma_fault_source_smmuv3 = 1,
    }

    /// Scope of a DMA fault report.
    enum dma_fault_scope {
        dma_fault_scope_stream = 1,
        dma_fault_scope_controller = 2,
    }

    /// Valid-address and access-direction flags in DMA fault reports.
    flags dma_fault_flags {
        /// The reported address is meaningful; a controller-wide report may omit it.
        dma_fault_address_valid = 1,
        dma_fault_read = 2,
        dma_fault_write = 4,
    }

    /// Shared-memory geometry and transfer bounds for the Native block initiator.
    constants native_block_layout {
        native_block_transfer_bytes = 524288,
        native_block_batch_max = 4,
        native_block_batch_record_bytes = 16,
        native_block_transfer_frame_bytes = 524352,
        native_block_memory_bytes = 1048576,
        native_block_queue_count = 6,
        native_block_queue_size = 8,
        native_block_queue_stride = 4096,
        native_block_available_offset = 256,
        native_block_used_offset = 512,
    }

    /// Asynchronous block submission directions. A flush carries no ranges or payload.
    enum block_queue_operation {
        block_queue_read = 0,
        block_queue_write = 1,
        block_queue_flush = 2,
    }

    /// Bounded submission and completion storage. Completed entries remain charged until reaped.
    constants block_queue_limits {
        block_queue_batch_max = 16,
        block_queue_range_bytes = 16,
        block_queue_transfer_bytes = 524288,
        block_queue_frame_bytes = 524544,
        block_queue_capacity_max = 32,
        block_queue_byte_limit_max = 8388608,
        block_queue_inflight_max = 4,
        /// Zero selects any completed ticket in block_queue_wait; it is never a valid ticket.
        block_queue_any_ticket = 0,
    }

    /// Guest notification control operations.
    enum guest_notification {
        guest_notification_disable = 0,
        guest_notification_enable = 1,
        guest_notification_raise_config = 2,
        guest_notification_disconnect = 3,
    }

    /// I/O client, mailbox and guest alias-window bounds.
    constants io_transport_limits {
        io_max_clients = 9,
        guest_mailbox_max_message_bytes = 256,
        guest_dynamic_alias_offset = 64 << 30,
        guest_dynamic_physical_limit = 256 << 30,
    }

    /// Architecture identifiers for guest construction.
    enum virtual_machine_architecture {
        virtual_machine_architecture_aarch64 = 1,
        virtual_machine_architecture_riscv64 = 2,
        virtual_machine_architecture_x86_64 = 3,
    }

    /// Guest power-management requests.
    enum virtual_machine_power {
        virtual_machine_power_cpu_on = 1,
        virtual_machine_power_cpu_off = 2,
        virtual_machine_power_system_off = 3,
        virtual_machine_power_system_reset = 4,
    }

    /// Installed guest lifecycle phases.
    enum virtual_machine_phase {
        virtual_machine_phase_installed = 1,
        virtual_machine_phase_running = 2,
        virtual_machine_phase_stopping = 3,
        virtual_machine_phase_stopped = 4,
    }

    /// Virtual CPU lifecycle phases.
    enum virtual_cpu_phase {
        virtual_cpu_phase_dormant = 1,
        virtual_cpu_phase_started = 2,
        virtual_cpu_phase_stopped = 3,
    }

    /// Virtual CPU termination reasons.
    enum virtual_cpu_terminal {
        virtual_cpu_terminal_none = 0,
        virtual_cpu_terminal_memory_fault = 1,
        virtual_cpu_terminal_mmio = 2,
        virtual_cpu_terminal_synchronous = 3,
        virtual_cpu_terminal_administrative = 4,
    }

    /// Sentinel for an unknown host CPU assignment.
    constants virtual_cpu_sentinels {
        /// No host CPU assignment is available; do not treat this as a CPU index.
        vcpu_host_cpu_unknown = u32::MAX as u64,
    }

    /// Reference platform identifiers.
    enum virtual_platform {
        virtual_platform_aarch64_reference = 1,
        virtual_platform_riscv64_reference = 2,
    }

    /// Immutable `AArch64` reference platform geometry shared by kernel and userspace.
    constants virtual_platform_aarch64_layout {
        virtual_platform_aarch64_reference_max_vcpus = 8,
        virtual_platform_aarch64_reference_guest_ram_base = 0x4000_0000,
        virtual_platform_aarch64_reference_dtb_offset = 0x0001_0000,
        virtual_platform_aarch64_reference_gic_distributor_base = 0x0800_0000,
        virtual_platform_aarch64_reference_gic_distributor_size = 0x0001_0000,
        virtual_platform_aarch64_reference_gic_redistributor_base = 0x080a_0000,
        virtual_platform_aarch64_reference_gic_redistributor_size = 0x0002_0000,
        virtual_platform_aarch64_reference_gicv2_distributor_size = 0x1000,
        virtual_platform_aarch64_reference_gicv2_cpu_base = 0x0801_0000,
        virtual_platform_aarch64_reference_gicv2_cpu_size = 0x2000,
        virtual_platform_aarch64_reference_uart_base = 0x0900_0000,
        virtual_platform_aarch64_reference_user_mmio_base = 0x0a00_0000,
        virtual_platform_aarch64_reference_user_mmio_size = 0x0100_0000,
        virtual_platform_aarch64_reference_uart_size = 0x1000,
        virtual_platform_aarch64_reference_uart_baud = 115_200,
        virtual_platform_aarch64_reference_uart_interrupt = 33,
        virtual_platform_aarch64_reference_timer_interrupt = 27,
        virtual_platform_aarch64_reference_interrupt_count = 256,
        virtual_platform_aarch64_reference_device_interrupt_base = 40,
    }

    /// Immutable RISC-V reference platform geometry shared by kernel and userspace.
    constants virtual_platform_riscv64_layout {
        virtual_platform_riscv64_reference_guest_ram_base = 2147483648,
        virtual_platform_riscv64_reference_dtb_offset = 65536,
        virtual_platform_riscv64_reference_uart_base = 268435456,
        virtual_platform_riscv64_reference_uart_size = 4096,
        virtual_platform_riscv64_reference_uart_interrupt = 10,
        virtual_platform_riscv64_reference_uart_clock_frequency = 3686400,
        virtual_platform_riscv64_reference_plic_base = 201326592,
        virtual_platform_riscv64_reference_plic_size = 4194304,
        virtual_platform_riscv64_reference_plic_num_sources = 31,
        virtual_platform_riscv64_reference_plic_supervisor_context = 1,
    }

    /// RISC-V ISA extensions accepted in guest configuration.
    flags riscv_isa {
        riscv_isa_i = 1,
        riscv_isa_m = 2,
        riscv_isa_a = 4,
        riscv_isa_f = 8,
        riscv_isa_d = 16,
        riscv_isa_c = 32,
        riscv_isa_zicsr = 64,
        riscv_isa_zifencei = 128,
        riscv_isa_sstc = 256,
    }
}
