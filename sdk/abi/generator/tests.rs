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
        ("MANAGE_NAMESPACE", 34),
        ("PROTECT", 35),
        ("LOCK_MEMORY", 36),
        ("RETAIN_PROCESS", 37),
        ("RETIRE", 38),
        ("SCHED_LOWER", 40),
        ("SCHED_PRIORITY", 41),
        ("SCHED_CLASS", 42),
        ("SCHED_BUDGET", 43),
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
            .contains("HYPER_NATIVE_RIGHTS_MASK: u64 = 0xfffffffffff;")
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
            .contains("HYPER_NATIVE_RIGHTS_MASK UINT64_C(0xfffffffffff)")
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
        let mut calls = schema::INVOCATIONS.to_vec();
        let selected = calls
            .iter_mut()
            .find(|call| call.name == "device_firmware_read");
        assert!(selected.is_some());
        let Some(call) = selected else {
            return;
        };
        let mut arguments = call.arguments.to_vec();
        if let Some(memory) = &mut arguments[0].memory {
            memory.length = MemoryLength::FixedBytes { bytes };
        }
        call.arguments = Box::leak(arguments.into_boxed_slice());
        let candidate = AbiSchema {
            invocations: Box::leak(calls.into_boxed_slice()),
            ..schema::NATIVE_ABI
        };
        assert_eq!(validate(&candidate).is_ok(), bytes == 24);
    }
}

#[test]
fn every_info_record_call_reports_its_supported_size() {
    for invocation in schema::INVOCATIONS {
        let has_info_output = invocation.arguments.iter().any(|argument| {
            argument.memory.is_some_and(|memory| {
                memory.direction == MemoryDirection::Write
                    && memory
                        .record
                        .is_some_and(|record| record != "wait_set_event")
                    && matches!(memory.length, MemoryLength::Bytes { .. })
            })
        });
        if has_info_output {
            let indexed = matches!(
                invocation.name,
                "crash_view_thread" | "crash_view_mapping" | "crash_view_handle"
            );
            assert_eq!(
                invocation.results.len(),
                if indexed { 2 } else { 1 },
                "{}",
                invocation.name
            );
            if indexed {
                assert_eq!(invocation.results[1].name, "next_index");
                assert_eq!(invocation.results[1].kind, ValueKind::U64);
            }
            assert_eq!(
                invocation.results[0].name, "supported_size",
                "{}",
                invocation.name
            );
            assert_eq!(
                invocation.results[0].kind,
                ValueKind::ByteCount,
                "{}",
                invocation.name
            );
        }
    }
}

#[test]
fn rejects_duplicate_operations_within_receiver_kind() {
    let mut duplicate = schema::INVOCATIONS.to_vec();
    duplicate[2].operation = duplicate[1].operation;
    let duplicate = Box::leak(duplicate.into_boxed_slice());
    let candidate = AbiSchema {
        invocations: duplicate,
        ..schema::NATIVE_ABI
    };
    assert!(
        matches!(validate(&candidate), Err(Error::InvalidSchema(message)) if message.contains("operation"))
    );
}

#[test]
fn receiver_namespace_allows_repeated_local_operations() {
    let file = schema::INVOCATIONS
        .iter()
        .find(|call| call.name == "file_read_at");
    let console = schema::INVOCATIONS
        .iter()
        .find(|call| call.name == "console_read");
    assert!(
        file.is_some() && console.is_some(),
        "required receiver interfaces are absent"
    );
    let (Some(file), Some(console)) = (file, console) else {
        return;
    };
    assert_eq!(file.operation, console.operation);
    assert_ne!(file.receiver.handle, console.receiver.handle);
    assert!(validate(&schema::NATIVE_ABI).is_ok());
}

#[test]
fn receiver_and_business_arguments_have_distinct_register_ownership() {
    for call in schema::INVOCATIONS {
        assert_eq!(call.receiver.kind, ValueKind::Handle, "{}", call.name);
        assert!(call.receiver.handle.is_some(), "{}", call.name);
        assert!(
            call.arguments
                .iter()
                .all(|argument| argument.name != call.receiver.name),
            "{} duplicates its receiver",
            call.name
        );
        assert!(call.arguments.len() <= schema::INVOCATION_ARGUMENT_REGISTERS);
    }
    let create_thread = schema::INVOCATIONS
        .iter()
        .find(|call| call.name == "thread_create");
    assert!(matches!(create_thread, Some(call) if call.arguments.len() == 6));
    let mount = schema::INVOCATIONS
        .iter()
        .find(|call| call.name == "filesystem_mount");
    assert!(
        matches!(mount, Some(call) if call.receiver.name == "directory"
        && call.arguments[0].name == "channel" && call.arguments[1].name == "buffer")
    );
}

#[test]
fn rejects_receiver_without_handle_metadata_and_local_common_overlap() {
    for missing_receiver in [true, false] {
        let mut calls = schema::INVOCATIONS.to_vec();
        if missing_receiver {
            calls[0].receiver.handle = None;
        } else {
            calls[0].operation = 0;
        }
        let candidate = AbiSchema {
            invocations: Box::leak(calls.into_boxed_slice()),
            ..schema::NATIVE_ABI
        };
        assert!(validate(&candidate).is_err());
    }
}

#[test]
fn rejects_unlinked_user_memory() {
    let mut calls = schema::INVOCATIONS.to_vec();
    let mut arguments = calls[4].arguments.to_vec();
    if let Some(memory) = arguments[0].memory.as_mut()
        && let MemoryLength::Bytes { argument, .. } = &mut memory.length
    {
        *argument = "missing";
    }
    calls[4].arguments = Box::leak(arguments.into_boxed_slice());
    let calls = Box::leak(calls.into_boxed_slice());
    let candidate = AbiSchema {
        invocations: calls,
        ..schema::NATIVE_ABI
    };
    assert!(
        matches!(validate(&candidate), Err(Error::InvalidSchema(message)) if message.contains("matching length"))
    );
}

#[test]
fn rejects_element_memory_with_byte_count_length() {
    let mut calls = schema::INVOCATIONS.to_vec();
    let mut arguments = calls[21].arguments.to_vec();
    arguments[4].kind = ValueKind::ByteCount;
    calls[21].arguments = Box::leak(arguments.into_boxed_slice());
    let calls = Box::leak(calls.into_boxed_slice());
    let candidate = AbiSchema {
        invocations: calls,
        ..schema::NATIVE_ABI
    };
    assert!(
        matches!(validate(&candidate), Err(Error::InvalidSchema(message)) if message.contains("matching length"))
    );
}

#[test]
fn rejects_element_stride_which_disagrees_with_record() {
    let mut calls = schema::INVOCATIONS.to_vec();
    let mut arguments = calls[21].arguments.to_vec();
    if let Some(memory) = arguments[3].memory.as_mut()
        && let MemoryLength::Elements { element_size, .. } = &mut memory.length
    {
        *element_size = 8;
    }
    calls[21].arguments = Box::leak(arguments.into_boxed_slice());
    let calls = Box::leak(calls.into_boxed_slice());
    let candidate = AbiSchema {
        invocations: calls,
        ..schema::NATIVE_ABI
    };
    assert!(
        matches!(validate(&candidate), Err(Error::InvalidSchema(message)) if message.contains("element size does not match"))
    );
}

#[test]
fn rejects_unknown_indirect_handle_record_field() {
    let mut calls = schema::INVOCATIONS.to_vec();
    let invocation = calls
        .iter_mut()
        .find(|invocation| invocation.name == "capability_channel_try_send");
    assert!(invocation.is_some());
    let Some(invocation) = invocation else {
        return;
    };
    let mut arguments = invocation.arguments.to_vec();
    if let Some(memory) = arguments[3].memory.as_mut() {
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
    invocation.arguments = Box::leak(arguments.into_boxed_slice());
    let calls = Box::leak(calls.into_boxed_slice());
    let candidate = AbiSchema {
        invocations: calls,
        ..schema::NATIVE_ABI
    };
    assert!(
        matches!(validate(&candidate), Err(Error::InvalidSchema(message)) if message.contains("invalid capability_disposition field"))
    );
}

#[test]
fn rejects_failure_results_for_unknown_status() {
    let mut calls = schema::INVOCATIONS.to_vec();
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
        invocations: calls,
        ..schema::NATIVE_ABI
    };
    assert!(
        matches!(validate(&candidate), Err(Error::InvalidSchema(message)) if message.contains("unknown failure-result status"))
    );
}

#[test]
fn common_failure_results_accept_only_declared_receiver_kinds()
-> Result<(), Box<dyn std::error::Error>> {
    use std::fmt::Write as _;
    use std::process::Command;

    let mut calls = schema::INVOCATIONS.to_vec();
    let call = calls
        .iter_mut()
        .find(|call| call.name == "handle_get_info")
        .ok_or("missing common information method")?;
    call.failure_results = &[schema::FailureResults {
        status: "buffer_too_small",
        results: &["supported_size"],
    }];
    let candidate = AbiSchema {
        invocations: Box::leak(calls.into_boxed_slice()),
        ..schema::NATIVE_ABI
    };
    let generated = generate(&candidate)?;
    let mut source = String::from("#include \"native.h\"\n\nint main(void) {\n");
    for object in candidate.object_kinds {
        let expected = u64::from(object.value != 0);
        writeln!(
            source,
            "    if (hyper_native_failure_result_mask(UINT32_C({}), HYPER_NATIVE_OP_HANDLE_GET_INFO, HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL) != UINT64_C({expected})) return 1;",
            object.value
        )?;
    }
    source.push_str(
        r#"
    if (hyper_native_failure_result_mask(UINT32_MAX, HYPER_NATIVE_OP_HANDLE_GET_INFO, HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL)) return 2;
    if (hyper_native_failure_result_mask(HYPER_NATIVE_OBJECT_FILE, HYPER_NATIVE_OP_HANDLE_GET_INFO, HYPER_NATIVE_STATUS_OK)) return 3;
    if (hyper_native_failure_result_mask(HYPER_NATIVE_OBJECT_FILE, HYPER_NATIVE_OP_HANDLE_CLOSE, HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL)) return 4;
    return 0;
}
"#,
    );
    let scratch =
        std::env::temp_dir().join(format!("hyper-abi-common-result-{}", std::process::id()));
    std::fs::create_dir_all(&scratch)?;
    let input = scratch.join("common-result.c");
    let executable = scratch.join("common-result");
    std::fs::write(scratch.join("native.h"), generated.c)?;
    std::fs::write(&input, source)?;
    // This fixture executes on the host. CLANG selects the cross toolchain and
    // may carry a different sysroot, so use the same HOST_CC contract as C tests.
    let compiler = std::env::var_os("HOST_CC").unwrap_or_else(|| {
        if cfg!(target_os = "macos") {
            "/usr/bin/clang".into()
        } else {
            "clang".into()
        }
    });
    let compiled = Command::new(compiler)
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror"])
        .arg(&input)
        .arg("-o")
        .arg(&executable)
        .output()?;
    assert!(
        compiled.status.success(),
        "common failure-result C fixture did not compile: {}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let executed = Command::new(&executable).output()?;
    assert!(
        executed.status.success(),
        "common failure-result C fixture failed: {executed:?}"
    );
    std::fs::remove_dir_all(scratch)?;
    Ok(())
}

#[test]
fn byte_channel_read_declares_buffer_size_result_on_failure() {
    let invocation = &schema::INVOCATIONS[14];
    assert_eq!(invocation.name, "byte_channel_read");
    assert_eq!(invocation.failure_results.len(), 1);
    assert_eq!(invocation.failure_results[0].status, "buffer_too_small");
    assert_eq!(invocation.failure_results[0].results, &["actual_bytes"]);
}

#[test]
fn capability_receive_is_flattened_and_transactionally_typed() {
    let invocation = schema::INVOCATIONS
        .iter()
        .find(|invocation| invocation.name == "capability_channel_receive");
    assert!(invocation.is_some());
    let Some(invocation) = invocation else {
        return;
    };
    assert_eq!(
        invocation.arguments.len() + 1,
        schema::INVOCATION_ARGUMENT_REGISTERS
    );
    let slots = invocation
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
    let invocation = schema::INVOCATIONS
        .iter()
        .find(|invocation| invocation.name == "object_wait_many");
    assert!(invocation.is_some());
    let Some(invocation) = invocation else {
        return;
    };
    let items = invocation
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
    let console_write = schema::INVOCATIONS
        .iter()
        .find(|invocation| invocation.name == "console_write")
        .map(|invocation| &invocation.receiver)
        .and_then(|argument| argument.handle);
    assert_eq!(
        console_write,
        Some(schema::HandleArgument {
            object: schema::ObjectConstraint::Kind("console"),
            required_rights: schema::RIGHT_WRITE,
            disposition: schema::HandleDisposition::Borrow,
        })
    );

    let vm_serial = schema::INVOCATIONS
        .iter()
        .find(|invocation| invocation.name == "pending_virtual_machine_set_virtual_serial")
        .and_then(|invocation| invocation.arguments.first())
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
    let mut calls = schema::INVOCATIONS.to_vec();
    let invocation = calls
        .iter_mut()
        .find(|invocation| invocation.name == "capability_channel_receive");
    assert!(invocation.is_some());
    let Some(invocation) = invocation else {
        return;
    };
    let mut arguments = invocation.arguments.to_vec();
    let slots = arguments
        .iter_mut()
        .find(|argument| argument.name == "capability_slots")
        .and_then(|argument| argument.memory.as_mut());
    assert!(slots.is_some());
    let Some(slots) = slots else {
        return;
    };
    slots.direction = MemoryDirection::Write;
    invocation.arguments = Box::leak(arguments.into_boxed_slice());
    let candidate = AbiSchema {
        invocations: Box::leak(calls.into_boxed_slice()),
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
    let mut calls = schema::INVOCATIONS.to_vec();
    let invocation = calls
        .iter_mut()
        .find(|invocation| invocation.name == "capability_channel_try_send");
    assert!(invocation.is_some());
    let Some(invocation) = invocation else {
        return;
    };
    let mut arguments = invocation.arguments.to_vec();
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
    invocation.arguments = Box::leak(arguments.into_boxed_slice());
    let candidate = AbiSchema {
        invocations: Box::leak(calls.into_boxed_slice()),
        ..schema::NATIVE_ABI
    };
    assert!(
        matches!(validate(&candidate), Err(Error::InvalidSchema(message)) if message.contains("invalid duplicate operation"))
    );
}

#[test]
fn directory_open_file_grants_exact_bounded_rights() {
    let invocation = schema::INVOCATIONS
        .iter()
        .find(|invocation| invocation.name == "directory_open_file");
    assert!(invocation.is_some());
    let Some(invocation) = invocation else {
        return;
    };
    assert!(matches!(
        invocation.results[0].handle,
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
    let mut calls = schema::INVOCATIONS.to_vec();
    let invocation = calls
        .iter_mut()
        .find(|invocation| invocation.name == "directory_open_file");
    assert!(invocation.is_some());
    let Some(invocation) = invocation else {
        return;
    };
    let mut results = invocation.results.to_vec();
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
    invocation.results = Box::leak(results.into_boxed_slice());
    let candidate = AbiSchema {
        invocations: Box::leak(calls.into_boxed_slice()),
        ..schema::NATIVE_ABI
    };
    assert!(
        matches!(validate(&candidate), Err(Error::InvalidSchema(message)) if message.contains("undeclared or empty object rights"))
    );
}

#[test]
fn rejects_an_unknown_exact_rights_authority_source() {
    let mut calls = schema::INVOCATIONS.to_vec();
    let invocation = calls
        .iter_mut()
        .find(|invocation| invocation.name == "directory_open_file");
    assert!(invocation.is_some());
    let Some(invocation) = invocation else {
        return;
    };
    let mut results = invocation.results.to_vec();
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
    invocation.results = Box::leak(results.into_boxed_slice());
    let candidate = AbiSchema {
        invocations: Box::leak(calls.into_boxed_slice()),
        ..schema::NATIVE_ABI
    };
    assert!(
        matches!(validate(&candidate), Err(Error::InvalidSchema(message)) if message.contains("invalid authority source handle"))
    );
}

#[test]
fn process_builder_is_linear_and_uses_staged_invocations() {
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
    let calls: Vec<_> = schema::INVOCATIONS
        .iter()
        .filter(|invocation| invocation.name.starts_with("process_builder_"))
        .map(|invocation| (invocation.operation, invocation.name))
        .collect();
    assert_eq!(
        calls,
        vec![
            (256, "process_builder_create"),
            (256, "process_builder_set_name"),
            (257, "process_builder_set_data"),
            (258, "process_builder_set_affinity"),
            (259, "process_builder_add_handle"),
            (260, "process_builder_seal"),
            (261, "process_builder_start"),
            (262, "process_builder_abort"),
            (263, "process_builder_set_memory_lock"),
            (264, "process_builder_retain"),
            (265, "process_builder_set_scheduling"),
        ]
    );
    for name in ["process_builder_start", "process_builder_abort"] {
        let invocation = schema::INVOCATIONS
            .iter()
            .find(|invocation| invocation.name == name);
        assert!(invocation.is_some());
        let Some(invocation) = invocation else {
            return;
        };
        assert!(matches!(
            invocation.receiver.handle,
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
    let clock = schema::INVOCATIONS
        .iter()
        .find(|invocation| invocation.name == "clock_get_monotonic");
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
        let invocation = schema::INVOCATIONS
            .iter()
            .find(|invocation| invocation.name == name);
        assert!(invocation.is_some());
        let Some(invocation) = invocation else {
            continue;
        };
        assert!(matches!(
            invocation.receiver.handle,
            Some(schema::HandleArgument {
                object: ObjectConstraint::Kind(found),
                required_rights,
                disposition: HandleDisposition::Borrow,
            }) if found == object && required_rights == rights
        ));
        assert!(matches!(
            invocation.results[0].handle,
            Some(schema::ProducedHandle {
                object: ProducedObject::Kind(found),
                rights: ProducedRights::Fixed(produced),
            }) if found == object && produced == rights
        ));
    }
}

#[test]
fn inspector_scan_limits_match_their_published_page_capacities() {
    for (invocation_name, constant_name, expected) in [
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
        let invocation = schema::INVOCATIONS
            .iter()
            .find(|invocation| invocation.name == invocation_name);
        let constant = schema::NATIVE_ABI
            .constants()
            .find(|constant| constant.name == constant_name);
        assert!(matches!(constant, Some(value) if value.value == expected));
        assert!(matches!(
            invocation.and_then(|value| value
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
        ("virtual_serial", TransferClass::RendezvousOnly),
        ("wait_set", TransferClass::Forbidden),
        ("guest_memory", TransferClass::RendezvousOnly),
        ("device_assignment_authority", TransferClass::General),
        ("physical_device", TransferClass::RendezvousOnly),
        ("guest_mailbox", TransferClass::RendezvousOnly),
        ("guest_notification", TransferClass::RendezvousOnly),
        ("native_block", TransferClass::RendezvousOnly),
        ("guest_mapping", TransferClass::RendezvousOnly),
        ("file_protection", TransferClass::Forbidden),
        ("directory_stream", TransferClass::General),
        ("root_transition", TransferClass::RendezvousOnly),
        ("guest_interrupt", TransferClass::RendezvousOnly),
        ("process_retirement", TransferClass::RendezvousOnly),
        ("crash_view", TransferClass::General),
        ("backend_memory_lease", TransferClass::Forbidden),
        ("execution_context", TransferClass::Forbidden),
        ("object_factory", TransferClass::Forbidden),
        ("system_info", TransferClass::General),
        ("block_queue", TransferClass::RendezvousOnly),
    ];
    assert_eq!(schema::OBJECT_KINDS.len(), expected.len());
    for (kind, expected) in schema::OBJECT_KINDS.iter().zip(expected) {
        assert_eq!((kind.name, kind.transfer), expected);
    }
}

#[test]
fn rejects_rights_not_declared_by_the_abi() {
    let mut calls = schema::INVOCATIONS.to_vec();
    if let Some(handle) = calls[2].receiver.handle.as_mut() {
        handle.required_rights = 1u64 << 63;
    }
    let calls = Box::leak(calls.into_boxed_slice());
    let candidate = AbiSchema {
        invocations: calls,
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
                description: "",
                name: "duplicate",
                value: 1,
            },
            schema::AbiConstant {
                description: "",
                name: "duplicate",
                value: 2,
            },
        ]
        .into_boxed_slice(),
    );
    let candidate = AbiSchema {
        constant_groups: test_constant_groups(schema::ConstantGroupKind::Constants, constants),
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
            description: "",
            name: "op_abi_query",
            value: 99,
        }]
        .into_boxed_slice(),
    );
    let candidate = AbiSchema {
        constant_groups: test_constant_groups(schema::ConstantGroupKind::Constants, constants),
        ..schema::NATIVE_ABI
    };
    assert!(
        matches!(validate(&candidate), Err(Error::InvalidSchema(message)) if message.contains("HYPER_NATIVE_OP_ABI_QUERY"))
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

#[test]
fn directory_stream_derivation_requires_read_only_and_separates_cursor_authority() {
    let begin = schema::INVOCATIONS
        .iter()
        .find(|call| call.name == "directory_begin_read");
    assert!(matches!(
        begin
            .map(|call| &call.receiver)
            .and_then(|argument| argument.handle),
        Some(schema::HandleArgument {
            object: schema::ObjectConstraint::Kind("directory"),
            required_rights: schema::RIGHT_READ,
            disposition: schema::HandleDisposition::Borrow
        })
    ));
    assert!(
        matches!(begin.and_then(|call| call.results.first()).and_then(|result| result.handle),
        Some(schema::ProducedHandle {
            object: ProducedObject::Kind("directory_stream"),
            rights: ProducedRights::InheritedSubset {
                authority_source: "directory", allowed_rights
            }
        }) if allowed_rights == schema::DIRECTORY_STREAM_RIGHTS)
    );
    let read = schema::INVOCATIONS
        .iter()
        .find(|call| call.name == "directory_read");
    assert!(matches!(
        read.map(|call| &call.receiver)
            .and_then(|argument| argument.handle),
        Some(schema::HandleArgument {
            object: schema::ObjectConstraint::Kind("directory_stream"),
            required_rights: schema::RIGHT_READ,
            disposition: schema::HandleDisposition::Borrow
        })
    ));
}

#[test]
fn residency_authority_is_inherited_without_becoming_a_default_grant() {
    for (name, source, inherited) in [
        (
            "process_builder_create",
            "factory",
            schema::RIGHT_LOCK_MEMORY
                | schema::RIGHT_RETAIN_PROCESS
                | schema::SCHEDULING_ELEVATION_RIGHTS,
        ),
        ("vmar_allocate", "vmar", schema::RIGHT_LOCK_MEMORY),
    ] {
        let call = schema::INVOCATIONS.iter().find(|call| call.name == name);
        assert!(
            matches!(call.and_then(|call| call.results.first()).and_then(|result| result.handle),
            Some(schema::ProducedHandle { rights: ProducedRights::FixedAndInherited {
                fixed_rights, authority_source, inherited_rights }, .. })
            if fixed_rights & inherited == 0
                && authority_source == source && inherited_rights == inherited)
        );
    }
}

#[test]
fn mixed_produced_rights_reject_undeclared_source_and_overlapping_masks() {
    for (source, inherited) in [
        ("missing", schema::RIGHT_LOCK_MEMORY),
        ("factory", schema::RIGHT_WRITE),
    ] {
        let mut calls = schema::INVOCATIONS.to_vec();
        let Some(call) = calls
            .iter_mut()
            .find(|call| call.name == "process_builder_create")
        else {
            return;
        };
        let mut results = call.results.to_vec();
        let Some(handle) = results[0].handle.as_mut() else {
            return;
        };
        handle.rights = ProducedRights::FixedAndInherited {
            fixed_rights: schema::PROCESS_BUILDER_RIGHTS,
            authority_source: source,
            inherited_rights: inherited,
        };
        call.results = Box::leak(results.into_boxed_slice());
        let candidate = AbiSchema {
            invocations: Box::leak(calls.into_boxed_slice()),
            ..schema::NATIVE_ABI
        };
        assert!(validate(&candidate).is_err());
    }
}

#[test]
fn execution_context_waits_require_execution_control_not_waitable_object_rights() {
    for name in ["object_wait_many", "atomic_wait", "thread_sleep"] {
        let invocation = schema::INVOCATIONS.iter().find(|call| call.name == name);
        assert!(invocation.is_some(), "missing {name}");
        let Some(invocation) = invocation else {
            continue;
        };
        let receiver = invocation.receiver.handle;
        assert!(
            matches!(receiver, Some(handle) if handle.object == ObjectConstraint::Kind("execution_context") && handle.required_rights == schema::RIGHT_WRITE),
            "{name}"
        );
    }
}

fn test_constant_groups(
    kind: schema::ConstantGroupKind,
    values: &'static [schema::AbiConstant],
) -> &'static [schema::ConstantGroup] {
    Box::leak(
        vec![schema::ConstantGroup {
            name: "test_values",
            description: "Schema validation fixture.",
            kind,
            values,
        }]
        .into_boxed_slice(),
    )
}

#[test]
fn scalar_domains_validate_enumerations_and_flags() {
    use schema::{AbiConstant, ConstantGroupKind};
    for (kind, values, accepted) in [
        (ConstantGroupKind::Enumeration, [0, 1], true),
        (ConstantGroupKind::Enumeration, [1, 1], false),
        (ConstantGroupKind::Flags, [1, 1 << 63], true),
        (ConstantGroupKind::Flags, [0, 1], false),
        (ConstantGroupKind::Flags, [1, 3], false),
        (ConstantGroupKind::Flags, [2, 2], false),
        (ConstantGroupKind::Constants, [u64::MAX, u64::MAX], true),
    ] {
        let values = Box::leak(
            vec![
                AbiConstant {
                    description: "",
                    name: "first",
                    value: values[0],
                },
                AbiConstant {
                    description: "",
                    name: "second",
                    value: values[1],
                },
            ]
            .into_boxed_slice(),
        );
        let candidate = AbiSchema {
            constant_groups: test_constant_groups(kind, values),
            ..schema::NATIVE_ABI
        };
        let result = validate(&candidate);
        assert_eq!(result.is_ok(), accepted, "{kind:?}: {result:?}");
    }
}

#[test]
fn rejects_duplicate_or_empty_constant_groups() {
    for groups in [
        vec![schema::CONSTANT_GROUPS[0], schema::CONSTANT_GROUPS[0]],
        vec![schema::ConstantGroup {
            values: &[],
            ..schema::CONSTANT_GROUPS[0]
        }],
        vec![schema::ConstantGroup {
            description: "",
            ..schema::CONSTANT_GROUPS[0]
        }],
        vec![schema::ConstantGroup {
            description: "escape */",
            ..schema::CONSTANT_GROUPS[0]
        }],
    ] {
        let candidate = AbiSchema {
            constant_groups: Box::leak(groups.into_boxed_slice()),
            ..schema::NATIVE_ABI
        };
        assert!(matches!(validate(&candidate), Err(Error::InvalidSchema(_))));
    }
}

#[test]
fn rejects_duplicate_constant_names_across_groups() {
    let group = schema::CONSTANT_GROUPS[0];
    let groups = Box::leak(
        vec![
            group,
            schema::ConstantGroup {
                name: "another_group",
                ..group
            },
        ]
        .into_boxed_slice(),
    );
    let candidate = AbiSchema {
        constant_groups: groups,
        ..schema::NATIVE_ABI
    };
    assert!(
        matches!(validate(&candidate), Err(Error::InvalidSchema(message)) if message.contains("declared more than once"))
    );
}

#[test]
fn block_queues_bound_frames_and_separate_completion_from_wait_failure() -> Result<(), String> {
    let call = |name| {
        schema::INVOCATIONS
            .iter()
            .find(|call| call.name == name)
            .ok_or_else(|| format!("missing operation {name}"))
    };
    let create = call("block_queue_create")?;
    assert_eq!(
        create.results[0].handle.map(|handle| handle.rights),
        Some(ProducedRights::FixedAndInherited {
            fixed_rights: schema::RIGHT_DUPLICATE,
            authority_source: "receiver",
            inherited_rights: schema::RIGHT_READ
                | schema::RIGHT_WRITE
                | schema::RIGHT_WAIT
                | schema::RIGHT_INSPECT
                | schema::RIGHT_TRANSFER,
        })
    );
    let submit = call("block_queue_submit")?;
    assert_eq!(submit.results.len(), 1);
    assert_eq!(submit.results[0].name, "ticket");
    let memory = submit.arguments[2].memory.ok_or("missing frame contract")?;
    assert_eq!(memory.direction, MemoryDirection::Read);
    assert_eq!(
        memory.length,
        MemoryLength::Bytes {
            argument: "frame_bytes",
            maximum_bytes: 524544
        }
    );
    let wait = call("block_queue_wait")?;
    assert_eq!(
        wait.receiver.handle.map(|handle| handle.required_rights),
        Some(schema::RIGHT_WAIT)
    );
    assert_eq!(wait.results[1].kind, ValueKind::I64);
    assert!(wait.failure_results.is_empty());
    let space = call("block_queue_wait_space")?;
    assert_eq!(space.operation, 260);
    assert_eq!(
        space.receiver.handle.map(|handle| handle.required_rights),
        Some(schema::RIGHT_WAIT)
    );
    assert!(space.results.is_empty());
    assert_eq!(space.arguments[0].kind, ValueKind::ByteCount);
    assert_eq!(space.cancellation, schema::CancellationClass::Explicit);
    let reap = call("block_queue_reap")?;
    assert_eq!(reap.results[0].kind, ValueKind::I64);
    assert_eq!(
        reap.arguments[1].memory.map(|memory| memory.direction),
        Some(MemoryDirection::Write)
    );
    let window = call("block_queue_create_window")?;
    assert_eq!(
        window.receiver.handle.map(|handle| handle.required_rights),
        Some(schema::RIGHT_READ)
    );
    assert_eq!(
        window.results[0].handle.map(|handle| handle.rights),
        Some(ProducedRights::Fixed(
            schema::RIGHT_READ
                | schema::RIGHT_MAP
                | schema::RIGHT_INSPECT
                | schema::RIGHT_DUPLICATE
                | schema::RIGHT_TRANSFER
        ))
    );
    let submit_window = call("block_queue_submit_window")?;
    assert_eq!(
        submit_window
            .receiver
            .handle
            .map(|handle| handle.required_rights),
        Some(schema::RIGHT_WRITE)
    );
    assert_eq!(
        submit_window.arguments[2]
            .memory
            .map(|memory| memory.length),
        Some(MemoryLength::Bytes {
            argument: "frame_bytes",
            maximum_bytes: 384,
        })
    );
    Ok(())
}
