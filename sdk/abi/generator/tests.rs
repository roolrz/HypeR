// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Schema rejection cases and generated-output contracts.

use super::{Error, generate, schema, validate};
use schema::{
    AbiSchema, CapabilityCommit, FieldKind, HandleDisposition, HandleOperation, IndirectHandles,
    MemoryDirection, MemoryLength, ObjectConstraint, OperationDisposition, ProducedObject,
    ProducedRights, TransferClass, ValueKind,
};

#[test]
fn production_schema_is_valid_and_deterministic() {
    assert!(validate(&schema::NATIVE_ABI).is_ok());
    let first = generate(&schema::NATIVE_ABI);
    let second = generate(&schema::NATIVE_ABI);
    assert!(first.is_ok(), "first generation failed: {first:?}");
    assert!(second.is_ok(), "second generation failed: {second:?}");
    let (Ok(first), Ok(second)) = (first, second) else {
        return;
    };
    assert_eq!(first.rust, second.rust);
    assert_eq!(first.c, second.c);
    assert_eq!(first.reference, second.reference);
}

#[test]
fn generated_bit_constants_preserve_schema_bit_positions() {
    let generated = generate(&schema::NATIVE_ABI);
    assert!(generated.is_ok());
    let Ok(generated) = generated else {
        return;
    };
    assert!(
        generated
            .rust
            .contains("HYPER_NATIVE_FEATURE_CORE: u64 = 1_u64 << 0;")
    );
    assert!(
        generated
            .rust
            .contains("HYPER_NATIVE_RIGHT_DERIVE: u64 = 1_u64 << 28;")
    );
    assert!(
        generated
            .rust
            .contains("HYPER_NATIVE_RIGHT_CREATE_VIRTUAL_MACHINE: u64 = 1_u64 << 29;")
    );
    for (name, bit) in [
        ("SET_ATTRIBUTES", 31),
        ("LOCK_FILE", 32),
        ("INSPECT_DETAILS", 33),
    ] {
        assert!(
            generated
                .rust
                .contains(&format!("HYPER_NATIVE_RIGHT_{name}: u64 = 1_u64 << {bit};"))
        );
        assert!(
            generated
                .c
                .contains(&format!("HYPER_NATIVE_RIGHT_{name} (UINT64_C(1) << {bit})"))
        );
    }
    assert!(
        generated
            .rust
            .contains("HYPER_NATIVE_RIGHTS_MASK: u64 = 0x3ffffffff;")
    );
    assert!(
        generated
            .c
            .contains("HYPER_NATIVE_RIGHT_DERIVE (UINT64_C(1) << 28)")
    );
    assert!(
        generated
            .c
            .contains("HYPER_NATIVE_RIGHT_CREATE_VIRTUAL_MACHINE (UINT64_C(1) << 29)")
    );
    assert!(
        generated
            .c
            .contains("HYPER_NATIVE_RIGHTS_MASK UINT64_C(0x3ffffffff)")
    );
}

#[test]
fn rejects_record_minimum_size_outside_current_layout() {
    let mut records = schema::RECORDS.to_vec();
    records[0].minimum_size = records[0].size + 1;
    let candidate = AbiSchema {
        records: Box::leak(records.into_boxed_slice()),
        ..schema::NATIVE_ABI
    };
    assert!(
        matches!(validate(&candidate), Err(Error::InvalidSchema(message)) if message.contains("minimum size"))
    );
}

#[test]
fn rejects_record_minimum_size_inside_a_field() {
    let mut records = schema::RECORDS.to_vec();
    records[0].minimum_size = 1;
    let candidate = AbiSchema {
        records: Box::leak(records.into_boxed_slice()),
        ..schema::NATIVE_ABI
    };
    assert!(
        matches!(validate(&candidate), Err(Error::InvalidSchema(message)) if message.contains("minimum size splits field"))
    );
}

#[test]
fn accepts_trailing_fields_after_the_minimum_prefix() {
    let mut records = schema::RECORDS.to_vec();
    let index = records
        .iter()
        .position(|record| record.name == "object_basic_info");
    assert!(index.is_some());
    let Some(index) = index else {
        return;
    };
    let mut fields = records[index].fields.to_vec();
    fields.push(schema::Field {
        name: "extension",
        kind: FieldKind::U64,
        offset: records[index].size,
    });
    records[index].fields = Box::leak(fields.into_boxed_slice());
    records[index].size += 8;
    let candidate = AbiSchema {
        records: Box::leak(records.into_boxed_slice()),
        ..schema::NATIVE_ABI
    };
    assert!(validate(&candidate).is_ok());
}

#[test]
fn fixed_query_memory_requires_a_nonzero_complete_record() {
    for bytes in [0, 16, 24] {
        let mut calls = schema::SYSCALLS.to_vec();
        let selected = calls
            .iter_mut()
            .find(|call| call.name == "device_firmware_read");
        assert!(selected.is_some());
        let Some(call) = selected else {
            return;
        };
        let mut arguments = call.arguments.to_vec();
        if let Some(memory) = &mut arguments[1].memory {
            memory.length = MemoryLength::FixedBytes { bytes };
        }
        call.arguments = Box::leak(arguments.into_boxed_slice());
        let candidate = AbiSchema {
            syscalls: Box::leak(calls.into_boxed_slice()),
            ..schema::NATIVE_ABI
        };
        assert_eq!(validate(&candidate).is_ok(), bytes == 24);
    }
}

#[test]
fn every_info_record_call_reports_its_supported_size() {
    for syscall in schema::SYSCALLS {
        let has_info_output = syscall.arguments.iter().any(|argument| {
            argument.memory.is_some_and(|memory| {
                memory.direction == MemoryDirection::Write
                    && memory
                        .record
                        .is_some_and(|record| record != "wait_set_event")
                    && matches!(memory.length, MemoryLength::Bytes { .. })
            })
        });
        if has_info_output {
            assert_eq!(syscall.results.len(), 1, "{}", syscall.name);
            assert_eq!(
                syscall.results[0].name, "supported_size",
                "{}",
                syscall.name
            );
            assert_eq!(
                syscall.results[0].kind,
                ValueKind::ByteCount,
                "{}",
                syscall.name
            );
        }
    }
}

#[test]
fn rejects_duplicate_permanent_numbers() {
    let mut duplicate = schema::SYSCALLS.to_vec();
    duplicate[1].number = duplicate[0].number;
    let duplicate = Box::leak(duplicate.into_boxed_slice());
    let candidate = AbiSchema {
        syscalls: duplicate,
        ..schema::NATIVE_ABI
    };
    assert!(
        matches!(validate(&candidate), Err(Error::InvalidSchema(message)) if message.contains("number"))
    );
}

#[test]
fn rejects_unlinked_user_memory() {
    let mut calls = schema::SYSCALLS.to_vec();
    let mut arguments = calls[4].arguments.to_vec();
    if let Some(memory) = arguments[1].memory.as_mut()
        && let MemoryLength::Bytes { argument, .. } = &mut memory.length
    {
        *argument = "missing";
    }
    calls[4].arguments = Box::leak(arguments.into_boxed_slice());
    let calls = Box::leak(calls.into_boxed_slice());
    let candidate = AbiSchema {
        syscalls: calls,
        ..schema::NATIVE_ABI
    };
    assert!(
        matches!(validate(&candidate), Err(Error::InvalidSchema(message)) if message.contains("matching length"))
    );
}

#[test]
fn rejects_element_memory_with_byte_count_length() {
    let mut calls = schema::SYSCALLS.to_vec();
    let mut arguments = calls[21].arguments.to_vec();
    arguments[5].kind = ValueKind::ByteCount;
    calls[21].arguments = Box::leak(arguments.into_boxed_slice());
    let calls = Box::leak(calls.into_boxed_slice());
    let candidate = AbiSchema {
        syscalls: calls,
        ..schema::NATIVE_ABI
    };
    assert!(
        matches!(validate(&candidate), Err(Error::InvalidSchema(message)) if message.contains("matching length"))
    );
}

#[test]
fn rejects_element_stride_which_disagrees_with_record() {
    let mut calls = schema::SYSCALLS.to_vec();
    let mut arguments = calls[21].arguments.to_vec();
    if let Some(memory) = arguments[4].memory.as_mut()
        && let MemoryLength::Elements { element_size, .. } = &mut memory.length
    {
        *element_size = 8;
    }
    calls[21].arguments = Box::leak(arguments.into_boxed_slice());
    let calls = Box::leak(calls.into_boxed_slice());
    let candidate = AbiSchema {
        syscalls: calls,
        ..schema::NATIVE_ABI
    };
    assert!(
        matches!(validate(&candidate), Err(Error::InvalidSchema(message)) if message.contains("element size does not match"))
    );
}

#[test]
fn rejects_unknown_indirect_handle_record_field() {
    let mut calls = schema::SYSCALLS.to_vec();
    let syscall = calls
        .iter_mut()
        .find(|syscall| syscall.name == "capability_channel_try_send");
    assert!(syscall.is_some());
    let Some(syscall) = syscall else {
        return;
    };
    let mut arguments = syscall.arguments.to_vec();
    if let Some(memory) = arguments[4].memory.as_mut() {
        memory.handles = Some(IndirectHandles::ConsumeRecords {
            handle_field: "missing",
            rights_field: "rights",
            expected_kind_field: "expected_kind",
            operation_field: "operation",
            common_rights: schema::RIGHT_TRANSFER,
            operations: schema::CAPABILITY_OPERATIONS,
            commit: CapabilityCommit::AtomicOnOk,
        });
    }
    syscall.arguments = Box::leak(arguments.into_boxed_slice());
    let calls = Box::leak(calls.into_boxed_slice());
    let candidate = AbiSchema {
        syscalls: calls,
        ..schema::NATIVE_ABI
    };
    assert!(
        matches!(validate(&candidate), Err(Error::InvalidSchema(message)) if message.contains("invalid capability_disposition field"))
    );
}

#[test]
fn rejects_failure_results_for_unknown_status() {
    let mut calls = schema::SYSCALLS.to_vec();
    let failures = Box::leak(
        vec![schema::FailureResults {
            status: "missing",
            results: &["actual_bytes"],
        }]
        .into_boxed_slice(),
    );
    calls[14].failure_results = failures;
    let calls = Box::leak(calls.into_boxed_slice());
    let candidate = AbiSchema {
        syscalls: calls,
        ..schema::NATIVE_ABI
    };
    assert!(
        matches!(validate(&candidate), Err(Error::InvalidSchema(message)) if message.contains("unknown failure-result status"))
    );
}

#[test]
fn byte_channel_read_declares_buffer_size_result_on_failure() {
    let syscall = &schema::SYSCALLS[14];
    assert_eq!(syscall.name, "byte_channel_read");
    assert_eq!(syscall.failure_results.len(), 1);
    assert_eq!(syscall.failure_results[0].status, "buffer_too_small");
    assert_eq!(syscall.failure_results[0].results, &["actual_bytes"]);
}

#[test]
fn capability_receive_is_flattened_and_transactionally_typed() {
    let syscall = schema::SYSCALLS
        .iter()
        .find(|syscall| syscall.name == "capability_channel_receive");
    assert!(syscall.is_some());
    let Some(syscall) = syscall else {
        return;
    };
    assert_eq!(syscall.arguments.len(), schema::SYSCALL_ARGUMENT_REGISTERS);
    let slots = syscall
        .arguments
        .iter()
        .find(|argument| argument.name == "capability_slots")
        .and_then(|argument| argument.memory);
    assert!(slots.is_some());
    let Some(slots) = slots else {
        return;
    };
    assert_eq!(slots.direction, MemoryDirection::ReadWrite);
    assert_eq!(slots.record, Some("capability_receive_slot"));
    assert!(matches!(
        slots.handles,
        Some(IndirectHandles::ProduceTransferred {
            handle_field: "handle",
            rights_field: "rights",
            expected_kind_field: "expected_kind",
            flags_field: "flags",
            commit: CapabilityCommit::AtomicOnOk,
        })
    ));
}

#[test]
fn object_wait_many_declares_borrowed_wait_authority() {
    let syscall = schema::SYSCALLS
        .iter()
        .find(|syscall| syscall.name == "object_wait_many");
    assert!(syscall.is_some());
    let Some(syscall) = syscall else {
        return;
    };
    let items = syscall
        .arguments
        .iter()
        .find(|argument| argument.name == "items")
        .and_then(|argument| argument.memory);
    assert!(items.is_some());
    let Some(items) = items else {
        return;
    };
    assert_eq!(items.direction, MemoryDirection::Read);
    assert_eq!(items.record, Some("object_wait_item"));
    assert!(matches!(
        items.handles,
        Some(IndirectHandles::BorrowRecords {
            handle_field: "handle",
            required_rights: schema::RIGHT_WAIT,
        })
    ));
}

#[test]
fn console_write_borrows_but_vm_binding_consumes_device_authority() {
    let console_write = schema::SYSCALLS
        .iter()
        .find(|syscall| syscall.name == "console_write")
        .and_then(|syscall| syscall.arguments.first())
        .and_then(|argument| argument.handle);
    assert_eq!(
        console_write,
        Some(schema::HandleArgument {
            object: schema::ObjectConstraint::Kind("console"),
            required_rights: schema::RIGHT_WRITE,
            disposition: schema::HandleDisposition::Borrow,
        })
    );

    let vm_serial = schema::SYSCALLS
        .iter()
        .find(|syscall| syscall.name == "pending_virtual_machine_set_virtual_serial")
        .and_then(|syscall| syscall.arguments.get(1))
        .and_then(|argument| argument.handle);
    assert_eq!(
        vm_serial,
        Some(schema::HandleArgument {
            object: schema::ObjectConstraint::Kind("virtual_serial"),
            required_rights: schema::RIGHT_TRANSFER | schema::RIGHT_ASSIGN_DEVICE,
            disposition: schema::HandleDisposition::ConsumeOnCommit,
        })
    );
}

#[test]
fn rejects_write_only_transferred_capability_slots() {
    let mut calls = schema::SYSCALLS.to_vec();
    let syscall = calls
        .iter_mut()
        .find(|syscall| syscall.name == "capability_channel_receive");
    assert!(syscall.is_some());
    let Some(syscall) = syscall else {
        return;
    };
    let mut arguments = syscall.arguments.to_vec();
    let slots = arguments
        .iter_mut()
        .find(|argument| argument.name == "capability_slots")
        .and_then(|argument| argument.memory.as_mut());
    assert!(slots.is_some());
    let Some(slots) = slots else {
        return;
    };
    slots.direction = MemoryDirection::Write;
    syscall.arguments = Box::leak(arguments.into_boxed_slice());
    let candidate = AbiSchema {
        syscalls: Box::leak(calls.into_boxed_slice()),
        ..schema::NATIVE_ABI
    };
    assert!(
        matches!(validate(&candidate), Err(Error::InvalidSchema(message)) if message.contains("must be in/out memory"))
    );
}

#[test]
fn rejects_transfer_operations_without_duplicate_authority() {
    const INVALID_OPERATIONS: &[HandleOperation] = &[
        HandleOperation {
            name: "move",
            value: 0,
            disposition: OperationDisposition::ConsumeOnCommit,
            additional_rights: 0,
        },
        HandleOperation {
            name: "duplicate",
            value: 1,
            disposition: OperationDisposition::Borrow,
            additional_rights: 0,
        },
    ];
    let mut calls = schema::SYSCALLS.to_vec();
    let syscall = calls
        .iter_mut()
        .find(|syscall| syscall.name == "capability_channel_try_send");
    assert!(syscall.is_some());
    let Some(syscall) = syscall else {
        return;
    };
    let mut arguments = syscall.arguments.to_vec();
    let dispositions = arguments
        .iter_mut()
        .find(|argument| argument.name == "dispositions")
        .and_then(|argument| argument.memory.as_mut());
    assert!(dispositions.is_some());
    let Some(dispositions) = dispositions else {
        return;
    };
    assert!(matches!(
        dispositions.handles,
        Some(IndirectHandles::ConsumeRecords { .. })
    ));
    let Some(IndirectHandles::ConsumeRecords { operations, .. }) = dispositions.handles.as_mut()
    else {
        return;
    };
    *operations = INVALID_OPERATIONS;
    syscall.arguments = Box::leak(arguments.into_boxed_slice());
    let candidate = AbiSchema {
        syscalls: Box::leak(calls.into_boxed_slice()),
        ..schema::NATIVE_ABI
    };
    assert!(
        matches!(validate(&candidate), Err(Error::InvalidSchema(message)) if message.contains("invalid duplicate operation"))
    );
}

#[test]
fn directory_open_file_grants_exact_bounded_rights() {
    let syscall = schema::SYSCALLS
        .iter()
        .find(|syscall| syscall.name == "directory_open_file");
    assert!(syscall.is_some());
    let Some(syscall) = syscall else {
        return;
    };
    assert!(matches!(
        syscall.results[0].handle,
        Some(schema::ProducedHandle {
            object: ProducedObject::Kind("file"),
            rights: ProducedRights::ExactRequested {
                argument: "requested_rights",
                allowed_rights: schema::FILE_RIGHTS,
                authority_source: Some("directory"),
            },
        })
    ));
}

#[test]
fn rejects_exact_rights_outside_the_object_allowlist() {
    let mut calls = schema::SYSCALLS.to_vec();
    let syscall = calls
        .iter_mut()
        .find(|syscall| syscall.name == "directory_open_file");
    assert!(syscall.is_some());
    let Some(syscall) = syscall else {
        return;
    };
    let mut results = syscall.results.to_vec();
    let handle = results[0].handle.as_mut();
    assert!(handle.is_some());
    let Some(handle) = handle else {
        return;
    };
    handle.rights = ProducedRights::ExactRequested {
        argument: "requested_rights",
        allowed_rights: 1u64 << 63,
        authority_source: Some("directory"),
    };
    syscall.results = Box::leak(results.into_boxed_slice());
    let candidate = AbiSchema {
        syscalls: Box::leak(calls.into_boxed_slice()),
        ..schema::NATIVE_ABI
    };
    assert!(
        matches!(validate(&candidate), Err(Error::InvalidSchema(message)) if message.contains("undeclared or empty object rights"))
    );
}

#[test]
fn rejects_an_unknown_exact_rights_authority_source() {
    let mut calls = schema::SYSCALLS.to_vec();
    let syscall = calls
        .iter_mut()
        .find(|syscall| syscall.name == "directory_open_file");
    assert!(syscall.is_some());
    let Some(syscall) = syscall else {
        return;
    };
    let mut results = syscall.results.to_vec();
    let handle = results[0].handle.as_mut();
    assert!(handle.is_some());
    let Some(handle) = handle else {
        return;
    };
    handle.rights = ProducedRights::ExactRequested {
        argument: "requested_rights",
        allowed_rights: schema::FILE_RIGHTS,
        authority_source: Some("missing_source"),
    };
    syscall.results = Box::leak(results.into_boxed_slice());
    let candidate = AbiSchema {
        syscalls: Box::leak(calls.into_boxed_slice()),
        ..schema::NATIVE_ABI
    };
    assert!(
        matches!(validate(&candidate), Err(Error::InvalidSchema(message)) if message.contains("invalid authority source handle"))
    );
}

#[test]
fn process_builder_is_linear_and_uses_staged_syscalls() {
    let builder = schema::OBJECT_KINDS
        .iter()
        .find(|object| object.name == "process_builder");
    assert!(builder.is_some());
    let Some(builder) = builder else {
        return;
    };
    assert_eq!(builder.value, 15);
    assert_eq!(builder.transfer, TransferClass::RendezvousOnly);
    assert_eq!(schema::PROCESS_BUILDER_RIGHTS & schema::RIGHT_DUPLICATE, 0);
    let calls: Vec<_> = schema::SYSCALLS
        .iter()
        .filter(|syscall| syscall.name.starts_with("process_builder_"))
        .map(|syscall| (syscall.number, syscall.name))
        .collect();
    assert_eq!(
        calls,
        vec![
            (22, "process_builder_create"),
            (23, "process_builder_set_name"),
            (24, "process_builder_set_data"),
            (26, "process_builder_set_affinity"),
            (27, "process_builder_add_handle"),
            (28, "process_builder_seal"),
            (29, "process_builder_start"),
            (30, "process_builder_abort"),
        ]
    );
    for name in ["process_builder_start", "process_builder_abort"] {
        let syscall = schema::SYSCALLS.iter().find(|syscall| syscall.name == name);
        assert!(syscall.is_some());
        let Some(syscall) = syscall else {
            return;
        };
        assert!(matches!(
            syscall.arguments[0].handle,
            Some(schema::HandleArgument {
                object: ObjectConstraint::Kind("process_builder"),
                disposition: HandleDisposition::ConsumeOnCommit,
                ..
            })
        ));
    }
}

#[test]
fn monotonic_clock_has_the_declared_nonblocking_contract() {
    let clock = schema::SYSCALLS
        .iter()
        .find(|syscall| syscall.name == "clock_get_monotonic");
    assert!(clock.is_some());
    let Some(clock) = clock else {
        return;
    };
    assert!(clock.arguments.is_empty());
    assert_eq!(clock.results.len(), 1);
    assert_eq!(clock.results[0].name, "nanoseconds");
    assert_eq!(clock.results[0].kind, ValueKind::U64);
    assert_eq!(clock.blocking, schema::BlockingClass::Never);
    assert_eq!(clock.completion, schema::CompletionClass::Returns);
    assert_eq!(clock.audit, schema::AuditClass::Abi);
}

#[test]
fn inspector_derivation_cannot_amplify_handle_rights() {
    for (name, object, rights) in [
        (
            "task_inspector_derive_process",
            "task_inspector",
            schema::TASK_INSPECTOR_RIGHTS,
        ),
        (
            "task_inspector_derive_task_group",
            "task_inspector",
            schema::TASK_INSPECTOR_RIGHTS,
        ),
        (
            "task_inspector_derive_resource_domain",
            "task_inspector",
            schema::TASK_INSPECTOR_RIGHTS,
        ),
        (
            "object_inspector_derive_process",
            "object_inspector",
            schema::OBJECT_INSPECTOR_RIGHTS,
        ),
        (
            "object_inspector_derive_task_group",
            "object_inspector",
            schema::OBJECT_INSPECTOR_RIGHTS,
        ),
        (
            "object_inspector_derive_resource_domain",
            "object_inspector",
            schema::OBJECT_INSPECTOR_RIGHTS,
        ),
    ] {
        let syscall = schema::SYSCALLS.iter().find(|syscall| syscall.name == name);
        assert!(syscall.is_some());
        let Some(syscall) = syscall else {
            continue;
        };
        assert!(matches!(
            syscall.arguments[0].handle,
            Some(schema::HandleArgument {
                object: ObjectConstraint::Kind(found),
                required_rights,
                disposition: HandleDisposition::Borrow,
            }) if found == object && required_rights == rights
        ));
        assert!(matches!(
            syscall.results[0].handle,
            Some(schema::ProducedHandle {
                object: ProducedObject::Kind(found),
                rights: ProducedRights::Fixed(produced),
            }) if found == object && produced == rights
        ));
    }
}

#[test]
fn inspector_scan_limits_match_their_published_page_capacities() {
    for (syscall_name, constant_name, expected) in [
        (
            "task_inspector_scan_processes",
            "task_inspector_process_page_capacity",
            8,
        ),
        (
            "task_inspector_scan_threads",
            "task_inspector_thread_page_capacity",
            8,
        ),
        (
            "object_inspector_scan_objects",
            "object_inspector_object_page_capacity",
            8,
        ),
        (
            "object_inspector_scan_handles",
            "object_inspector_handle_page_capacity",
            8,
        ),
    ] {
        let syscall = schema::SYSCALLS
            .iter()
            .find(|syscall| syscall.name == syscall_name);
        let constant = schema::CONSTANTS
            .iter()
            .find(|constant| constant.name == constant_name);
        assert!(matches!(constant, Some(value) if value.value == expected));
        assert!(matches!(
            syscall.and_then(|value| value
                .arguments
                .iter()
                .find(|argument| argument.name == "records"))
                .and_then(|argument| argument.memory),
            Some(schema::UserMemory {
                length: MemoryLength::Elements {
                    maximum_elements,
                    ..
                },
                ..
            }) if u64::from(maximum_elements) == expected
        ));
    }
}

#[test]
fn object_transfer_classes_match_the_audited_contract() {
    let expected = [
        ("none", TransferClass::Forbidden),
        ("event", TransferClass::General),
        ("byte_channel", TransferClass::General),
        ("thread", TransferClass::RendezvousOnly),
        ("process", TransferClass::RendezvousOnly),
        ("task_group", TransferClass::RendezvousOnly),
        ("resource_domain", TransferClass::General),
        ("task_factory", TransferClass::General),
        ("executable_authority", TransferClass::General),
        ("vmo", TransferClass::General),
        ("vmar", TransferClass::RendezvousOnly),
        ("console", TransferClass::General),
        ("directory", TransferClass::General),
        ("file", TransferClass::General),
        ("capability_channel", TransferClass::RendezvousOnly),
        ("process_builder", TransferClass::RendezvousOnly),
        ("task_inspector", TransferClass::General),
        ("object_inspector", TransferClass::General),
        ("memory_inspector", TransferClass::General),
        ("cpu_inspector", TransferClass::General),
        ("virtual_machine_creation_authority", TransferClass::General),
        (
            "virtual_machine_creation_lease",
            TransferClass::RendezvousOnly,
        ),
        ("pending_virtual_machine", TransferClass::RendezvousOnly),
        ("virtual_machine", TransferClass::RendezvousOnly),
        ("virtual_cpu", TransferClass::RendezvousOnly),
        ("virtual_serial", TransferClass::Forbidden),
        ("wait_set", TransferClass::Forbidden),
        ("guest_memory", TransferClass::RendezvousOnly),
        ("device_assignment_authority", TransferClass::General),
        ("physical_device", TransferClass::RendezvousOnly),
        ("guest_mailbox", TransferClass::RendezvousOnly),
        ("guest_notification", TransferClass::RendezvousOnly),
        ("native_block", TransferClass::RendezvousOnly),
        ("guest_mapping", TransferClass::RendezvousOnly),
        ("backend_memory_lease", TransferClass::Forbidden),
    ];
    assert_eq!(schema::OBJECT_KINDS.len(), expected.len());
    for (kind, expected) in schema::OBJECT_KINDS.iter().zip(expected) {
        assert_eq!((kind.name, kind.transfer), expected);
    }
}

#[test]
fn rejects_rights_not_declared_by_the_abi() {
    let mut calls = schema::SYSCALLS.to_vec();
    let mut arguments = calls[2].arguments.to_vec();
    if let Some(handle) = arguments[0].handle.as_mut() {
        handle.required_rights = 1u64 << 63;
    }
    calls[2].arguments = Box::leak(arguments.into_boxed_slice());
    let calls = Box::leak(calls.into_boxed_slice());
    let candidate = AbiSchema {
        syscalls: calls,
        ..schema::NATIVE_ABI
    };
    assert!(
        matches!(validate(&candidate), Err(Error::InvalidSchema(message)) if message.contains("undeclared rights"))
    );
}

#[test]
fn rejects_unrepresentable_record_alignment() {
    let mut records = schema::RECORDS.to_vec();
    records[0].alignment = 4;
    let records = Box::leak(records.into_boxed_slice());
    let candidate = AbiSchema {
        records,
        ..schema::NATIVE_ABI
    };
    assert!(
        matches!(validate(&candidate), Err(Error::InvalidSchema(message)) if message.contains("render alignment"))
    );
}

#[test]
fn rejects_nonzero_revision_during_pre_release_development() {
    let candidate = AbiSchema {
        revision: 1,
        ..schema::NATIVE_ABI
    };
    assert!(
        matches!(validate(&candidate), Err(Error::InvalidSchema(message)) if message.contains("must remain zero"))
    );
}

#[test]
fn reserves_zero_for_the_none_object_kind() {
    let kinds = Box::leak(
        vec![schema::ObjectKind {
            value: 1,
            name: "none",
            transfer: TransferClass::Forbidden,
        }]
        .into_boxed_slice(),
    );
    let candidate = AbiSchema {
        object_kinds: kinds,
        ..schema::NATIVE_ABI
    };
    assert!(
        matches!(validate(&candidate), Err(Error::InvalidSchema(message)) if message.contains("reserved value zero"))
    );
}

#[test]
fn rejects_signals_for_unknown_object_kinds() {
    let signals = Box::leak(
        vec![schema::Signal {
            object: "missing",
            bit: 0,
            name: "ready",
        }]
        .into_boxed_slice(),
    );
    let candidate = AbiSchema {
        signals,
        ..schema::NATIVE_ABI
    };
    assert!(
        matches!(validate(&candidate), Err(Error::InvalidSchema(message)) if message.contains("unknown object kind"))
    );
}

#[test]
fn rejects_duplicate_public_constants() {
    let constants = Box::leak(
        vec![
            schema::AbiConstant {
                name: "duplicate",
                value: 1,
            },
            schema::AbiConstant {
                name: "duplicate",
                value: 2,
            },
        ]
        .into_boxed_slice(),
    );
    let candidate = AbiSchema {
        constants,
        ..schema::NATIVE_ABI
    };
    assert!(
        matches!(validate(&candidate), Err(Error::InvalidSchema(message)) if message.contains("declared more than once"))
    );
}

#[test]
fn rejects_cross_category_generated_constant_collisions() {
    let constants = Box::leak(
        vec![schema::AbiConstant {
            name: "sys_abi_query",
            value: 99,
        }]
        .into_boxed_slice(),
    );
    let candidate = AbiSchema {
        constants,
        ..schema::NATIVE_ABI
    };
    assert!(
        matches!(validate(&candidate), Err(Error::InvalidSchema(message)) if message.contains("HYPER_NATIVE_SYS_ABI_QUERY"))
    );
}

#[test]
fn reserves_zero_for_success_and_rejects_positive_statuses() {
    let statuses = Box::leak(
        vec![
            schema::Status {
                value: 0,
                name: "ok",
            },
            schema::Status {
                value: 1,
                name: "invalid",
            },
        ]
        .into_boxed_slice(),
    );
    let candidate = AbiSchema {
        statuses,
        ..schema::NATIVE_ABI
    };
    assert!(
        matches!(validate(&candidate), Err(Error::InvalidSchema(message)) if message.contains("positive value"))
    );
}
