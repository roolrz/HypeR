// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Deterministic reference output for a validated ABI schema.

use super::super::schema;
use schema::{
    AbiSchema, FieldKind, HandleDisposition, HandleOperation, IndirectHandles, MemoryLength,
    ObjectConstraint, OperationDisposition, ProducedObject, ProducedRights, TransferClass,
    ValueKind,
};
use std::fmt::Write as _;

const fn transfer_class_name(class: TransferClass) -> &'static str {
    match class {
        TransferClass::Forbidden => "forbidden",
        TransferClass::General => "general",
        TransferClass::RendezvousOnly => "rendezvous_only",
    }
}

pub(in super::super) fn render_reference(schema: &AbiSchema) -> String {
    let mut output = String::from(
        "<!--\n\
         SPDX-FileCopyrightText: 2026 roolrz\n\
         SPDX-License-Identifier: Apache-2.0\n\
         -->\n\n\
         # HypeR Native ABI reference\n\n\
         This file is generated from `schema/native.rs`. Do not edit it directly.\n\n",
    );
    let _ = writeln!(output, "ABI revision: `{}`.\n", schema.revision);
    output.push_str("## Status values\n\n| Value | Name |\n| ---: | --- |\n");
    for status in schema.statuses {
        let _ = writeln!(output, "| {} | `{}` |", status.value, status.name);
    }
    output.push('\n');
    output.push_str("## Object kinds\n\n| Value | Name | Transfer |\n| ---: | --- | --- |\n");
    for object in schema.object_kinds {
        let _ = writeln!(
            output,
            "| {} | `{}` | `{}` |",
            object.value,
            object.name,
            transfer_class_name(object.transfer)
        );
    }
    output.push('\n');
    output.push_str("## Object signals\n\n| Object | Bit | Name |\n| --- | ---: | --- |\n");
    for signal in schema.signals {
        let _ = writeln!(
            output,
            "| `{}` | {} | `{}` |",
            signal.object, signal.bit, signal.name
        );
    }
    output.push('\n');
    output.push_str("## Constants\n\n| Name | Value |\n| --- | ---: |\n");
    for constant in schema.constants {
        let _ = writeln!(output, "| `{}` | `{}` |", constant.name, constant.value);
    }
    output.push('\n');
    output.push_str("## Semantic rules\n\n");
    for rule in schema.semantic_rules {
        let _ = writeln!(output, "- {rule}");
    }
    output.push('\n');
    output.push_str(
        "## Syscalls\n\n\
         Auxiliary result registers are defined only for `ok` unless a result is annotated with\n\
         `also-on=<status>`. Element-count memory ranges are checked as count times the declared\n\
         element size before any user-memory access.\n\n\
         | Number | Name | Arguments | Results | Capability effects | User memory | Execution | Audit |\n\
         | ---: | --- | --- | --- | --- | --- | --- | --- |\n",
    );
    for syscall in schema.syscalls {
        let arguments = joined_values(
            syscall
                .arguments
                .iter()
                .map(|value| format!("`{}: {}`", value.name, value_kind_name(value.kind))),
        );
        let results = joined_values(syscall.results.iter().map(|value| {
            let failures = joined_failure_statuses(syscall, value.name);
            format!(
                "`{}: {}{}`",
                value.name,
                value_kind_name(value.kind),
                failures
            )
        }));
        let capability_effects = joined_values(
            syscall
                .arguments
                .iter()
                .filter_map(|argument| {
                    argument
                        .handle
                        .map(|handle| describe_handle_argument(argument.name, handle))
                })
                .chain(syscall.results.iter().filter_map(|result| {
                    result
                        .handle
                        .map(|handle| describe_produced_handle(result.name, handle))
                })),
        );
        let user_memory = joined_values(syscall.arguments.iter().filter_map(|argument| {
            argument
                .memory
                .map(|memory| describe_user_memory(argument.name, memory))
        }));
        let execution = format!(
            "`blocking={:?}, cancellation={:?}, restart={:?}, completion={:?}, flags={:?}`",
            syscall.blocking,
            syscall.cancellation,
            syscall.restart,
            syscall.completion,
            syscall.flags
        );
        let _ = writeln!(
            output,
            "| {} | `{}` | {} | {} | {} | {} | {} | `{:?}` |",
            syscall.number,
            syscall.name,
            arguments,
            results,
            capability_effects,
            user_memory,
            execution,
            syscall.audit
        );
    }
    output.push_str("\n## Public records\n\n| Name | Minimum prefix | Size | Alignment | Fields |\n| --- | ---: | ---: | ---: | --- |\n");
    for record in schema.records {
        let fields = joined_values(record.fields.iter().map(|field| {
            format!(
                "`{}: {} @ {}`",
                field.name,
                field_kind_name(field.kind),
                field.offset
            )
        }));
        let _ = writeln!(
            output,
            "| `{}` | {} | {} | {} | {} |",
            record.name, record.minimum_size, record.size, record.alignment, fields
        );
    }
    output
}

fn joined_values(values: impl Iterator<Item = String>) -> String {
    let values: Vec<_> = values.collect();
    if values.is_empty() {
        String::from("—")
    } else {
        values.join(", ")
    }
}

fn describe_handle_argument(name: &str, handle: schema::HandleArgument) -> String {
    let object = match handle.object {
        ObjectConstraint::Any => String::from("any"),
        ObjectConstraint::Kind(kind) => format!("kind={kind}"),
    };
    match handle.disposition {
        HandleDisposition::Borrow => format!(
            "`{name}: Borrow, {object}, rights=0x{:x}`",
            handle.required_rights
        ),
        HandleDisposition::ConsumeOnCommit => format!(
            "`{name}: ConsumeOnCommit, {object}, rights=0x{:x}`",
            handle.required_rights
        ),
        HandleDisposition::ByOperation {
            argument,
            operations,
        } => format!(
            "`{name}: ByOperation({argument}: {}), {object}, common-rights=0x{:x}`",
            describe_operations(operations, handle.required_rights),
            handle.required_rights
        ),
    }
}

fn describe_produced_handle(name: &str, handle: schema::ProducedHandle) -> String {
    let object = match handle.object {
        ProducedObject::SameAsArgument(argument) => format!("same-as({argument})"),
        ProducedObject::Kind(kind) => format!("kind={kind}"),
    };
    let rights = match handle.rights {
        ProducedRights::RequestedSubsetOf(argument) => format!("subset-from({argument})"),
        ProducedRights::ExactRequested {
            argument,
            allowed_rights,
            authority_source,
        } => {
            let authority = authority_source
                .map_or_else(String::new, |source| format!(", required-from={source}"));
            format!("exact-from({argument}), allowed=0x{allowed_rights:x}{authority}")
        }
        ProducedRights::Fixed(mask) => format!("fixed=0x{mask:x}"),
    };
    format!("`{name}: produce, {object}, {rights}`")
}

fn describe_user_memory(name: &str, memory: schema::UserMemory) -> String {
    let record = memory
        .record
        .map_or_else(String::new, |record| format!(", record={record}"));
    let length = match memory.length {
        MemoryLength::FixedBytes { bytes } => format!("fixed-bytes={bytes}"),
        MemoryLength::Bytes {
            argument,
            maximum_bytes,
        } => format!("len={argument} bytes, max-bytes={maximum_bytes}"),
        MemoryLength::Elements {
            argument,
            maximum_elements,
            element_size,
        } => format!(
            "len={argument} elements, max-elements={maximum_elements}, element-size={element_size}"
        ),
    };
    let handles = match memory.handles {
        None => String::new(),
        Some(IndirectHandles::BorrowRecords {
            handle_field,
            required_rights,
        }) => format!(", borrowed-handles=({handle_field}), required-rights=0x{required_rights:x}"),
        Some(IndirectHandles::ConsumeRecords {
            handle_field,
            rights_field,
            expected_kind_field,
            operation_field,
            common_rights,
            operations,
            commit,
        }) => format!(
            ", transactional-handles=({handle_field}, {rights_field}, {expected_kind_field}, {operation_field}), common-rights=0x{common_rights:x}, operations=[{}], commit={commit:?}",
            describe_operations(operations, common_rights)
        ),
        Some(IndirectHandles::ProduceTransferred {
            handle_field,
            rights_field,
            expected_kind_field,
            flags_field,
            commit,
        }) => format!(
            ", typed-receive-slots=({handle_field}, {rights_field}, {expected_kind_field}, {flags_field}), produce-transferred-handles, commit={commit:?}"
        ),
    };
    format!(
        "`{name}: {:?}, {}{}{}; order={}`",
        memory.direction, length, record, handles, memory.validation_order
    )
}

fn describe_operations(operations: &[HandleOperation], common_rights: u64) -> String {
    operations
        .iter()
        .map(|operation| {
            let disposition = match operation.disposition {
                OperationDisposition::Borrow => "Borrow",
                OperationDisposition::ConsumeOnCommit => "ConsumeOnCommit",
            };
            format!(
                "{}={}:{disposition}/rights=0x{:x}",
                operation.name,
                operation.value,
                common_rights | operation.additional_rights
            )
        })
        .collect::<Vec<_>>()
        .join("+")
}

fn joined_failure_statuses(syscall: &schema::Syscall, result_name: &str) -> String {
    let statuses: Vec<_> = syscall
        .failure_results
        .iter()
        .filter(|failure| failure.results.contains(&result_name))
        .map(|failure| failure.status)
        .collect();
    if statuses.is_empty() {
        String::new()
    } else {
        format!("; also-on={}", statuses.join("+"))
    }
}

fn value_kind_name(kind: ValueKind) -> &'static str {
    match kind {
        ValueKind::U32 => "u32",
        ValueKind::U64 => "u64",
        ValueKind::I64 => "i64",
        ValueKind::Handle => "handle",
        ValueKind::UserAddress => "user_address",
        ValueKind::ByteCount => "byte_count",
        ValueKind::ElementCount => "element_count",
        ValueKind::Rights => "rights",
    }
}

fn field_kind_name(kind: FieldKind) -> String {
    match kind {
        FieldKind::I64 => String::from("i64"),
        FieldKind::U32 => String::from("u32"),
        FieldKind::U64 => String::from("u64"),
        FieldKind::Bytes(size) => format!("bytes[{size}]"),
    }
}
