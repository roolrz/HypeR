// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Syscall argument, result, and completion contracts.

use super::handles::{
    require_object_kind, validate_handle_operations, validate_indirect_handles,
    validate_transfer_operations,
};
use super::{Error, invalid, schema, validate_identifier};
use schema::{
    AbiSchema, CompletionClass, FeatureGate, HandleDisposition, MemoryLength, ObjectConstraint,
    ProducedObject, ProducedRights, ValueKind,
};
use std::collections::BTreeSet;

pub(super) fn validate_syscalls(schema: &AbiSchema, supported_rights: u64) -> Result<(), Error> {
    let mut names = BTreeSet::new();
    let mut numbers = BTreeSet::new();
    let mut previous_number = None;
    for syscall in schema.syscalls {
        validate_identifier("syscall", syscall.name)?;
        if !names.insert(syscall.name) {
            return invalid(format!(
                "syscall {} is declared more than once",
                syscall.name
            ));
        }
        if !numbers.insert(syscall.number) {
            return invalid(format!(
                "syscall number {} is declared more than once",
                syscall.number
            ));
        }
        if previous_number.is_some_and(|previous| syscall.number <= previous) {
            return invalid("syscall declarations must be ordered by number");
        }
        previous_number = Some(syscall.number);
        match syscall.feature {
            FeatureGate::Core => {}
            FeatureGate::Named(name)
                if schema.features.iter().any(|feature| feature.name == name) => {}
            FeatureGate::Named(name) => {
                return invalid(format!(
                    "syscall {} names unknown feature {name}",
                    syscall.name
                ));
            }
        }
        if syscall.arguments.len() > schema::SYSCALL_ARGUMENT_REGISTERS {
            return invalid(format!(
                "syscall {} exceeds the machine argument registers",
                syscall.name
            ));
        }
        if syscall.results.len() > schema::SYSCALL_RESULT_REGISTERS {
            return invalid(format!(
                "syscall {} exceeds the machine result registers",
                syscall.name
            ));
        }
        if syscall.completion == CompletionClass::NoReturn && !syscall.results.is_empty() {
            return invalid(format!(
                "no-return syscall {} declares results",
                syscall.name
            ));
        }
        validate_arguments(schema, syscall, supported_rights)?;
        validate_results(schema, syscall, supported_rights)?;
    }
    Ok(())
}

fn validate_arguments(
    schema: &AbiSchema,
    syscall: &schema::Syscall,
    supported_rights: u64,
) -> Result<(), Error> {
    let mut names = BTreeSet::new();
    let mut memory_orders = BTreeSet::new();
    let mut memory_count = 0usize;
    for argument in syscall.arguments {
        validate_identifier("argument", argument.name)?;
        if !names.insert(argument.name) {
            return invalid(format!(
                "syscall {} repeats argument {}",
                syscall.name, argument.name
            ));
        }
        if argument.handle.is_some() != (argument.kind == ValueKind::Handle) {
            return invalid(format!(
                "syscall {} argument {} has inconsistent handle metadata",
                syscall.name, argument.name
            ));
        }
        if argument.memory.is_some() != (argument.kind == ValueKind::UserAddress) {
            return invalid(format!(
                "syscall {} argument {} has inconsistent memory metadata",
                syscall.name, argument.name
            ));
        }
        if let Some(handle) = argument.handle {
            if handle.required_rights & !supported_rights != 0 {
                return invalid(format!(
                    "syscall {} argument {} requires undeclared rights",
                    syscall.name, argument.name
                ));
            }
            if let ObjectConstraint::Kind(kind) = handle.object {
                require_object_kind(schema, syscall.name, kind)?;
            }
            match handle.disposition {
                HandleDisposition::Borrow | HandleDisposition::ConsumeOnCommit => {}
                HandleDisposition::ByOperation {
                    argument: operation_argument,
                    operations,
                } => {
                    if !syscall.arguments.iter().any(|candidate| {
                        candidate.name == operation_argument && candidate.kind == ValueKind::U32
                    }) {
                        return invalid(format!(
                            "syscall {} handle {} names invalid operation argument {operation_argument}",
                            syscall.name, argument.name
                        ));
                    }
                    validate_handle_operations(
                        syscall.name,
                        argument.name,
                        operations,
                        supported_rights,
                    )?;
                    validate_transfer_operations(
                        syscall,
                        argument,
                        handle.required_rights,
                        operations,
                    )?;
                }
            }
        }
        if let Some(memory) = argument.memory {
            memory_count += 1;
            if !memory_orders.insert(memory.validation_order) {
                return invalid(format!(
                    "syscall {} repeats memory validation order {}",
                    syscall.name, memory.validation_order
                ));
            }
            let (length_argument, expected_length_kind, maximum_bytes, element_size) = match memory
                .length
            {
                MemoryLength::FixedBytes { bytes } => {
                    if bytes == 0 {
                        return invalid("fixed memory size cannot be zero");
                    }
                    ("", ValueKind::ByteCount, bytes, None)
                }
                MemoryLength::Bytes {
                    argument: length_name,
                    maximum_bytes,
                } => {
                    if maximum_bytes == 0 {
                        return invalid(format!(
                            "syscall {} argument {} has an unbounded zero byte maximum",
                            syscall.name, argument.name
                        ));
                    }
                    (length_name, ValueKind::ByteCount, maximum_bytes, None)
                }
                MemoryLength::Elements {
                    argument: length_name,
                    maximum_elements,
                    element_size,
                } => {
                    if maximum_elements == 0 || element_size == 0 {
                        return invalid(format!(
                            "syscall {} argument {} has an invalid element bound",
                            syscall.name, argument.name
                        ));
                    }
                    let Some(maximum_bytes) = maximum_elements.checked_mul(u32::from(element_size))
                    else {
                        return invalid(format!(
                            "syscall {} argument {} element range overflows",
                            syscall.name, argument.name
                        ));
                    };
                    (
                        length_name,
                        ValueKind::ElementCount,
                        maximum_bytes,
                        Some(element_size),
                    )
                }
            };
            let length = syscall
                .arguments
                .iter()
                .find(|candidate| candidate.name == length_argument);
            if !matches!(memory.length, MemoryLength::FixedBytes { .. })
                && !matches!(length, Some(candidate) if candidate.kind == expected_length_kind)
            {
                return invalid(format!(
                    "syscall {} memory argument {} has no matching length argument",
                    syscall.name, argument.name,
                ));
            }
            let mut selected_record = None;
            if let Some(record_name) = memory.record {
                let Some(record) = schema
                    .records
                    .iter()
                    .find(|record| record.name == record_name)
                else {
                    return invalid(format!(
                        "syscall {} memory argument {} names unknown record {record_name}",
                        syscall.name, argument.name
                    ));
                };
                if u32::from(record.size) > maximum_bytes {
                    return invalid(format!(
                        "syscall {} memory argument {} cannot contain record {record_name}",
                        syscall.name, argument.name
                    ));
                }
                if element_size.is_some_and(|size| size != record.size) {
                    return invalid(format!(
                        "syscall {} memory argument {} element size does not match record {record_name}",
                        syscall.name, argument.name
                    ));
                }
                selected_record = Some(record);
            }
            if u32::from(element_size.unwrap_or(1)) > maximum_bytes {
                return invalid(format!(
                    "syscall {} memory argument {} cannot contain one element",
                    syscall.name, argument.name
                ));
            }
            if let Some(handles) = memory.handles {
                validate_indirect_handles(
                    syscall,
                    argument,
                    memory,
                    handles,
                    selected_record,
                    supported_rights,
                )?;
            }
        }
    }
    if !(0..memory_count).all(|order| memory_orders.contains(&(order as u8))) {
        return invalid(format!(
            "syscall {} memory validation order is not contiguous",
            syscall.name
        ));
    }
    Ok(())
}

fn validate_results(
    schema: &AbiSchema,
    syscall: &schema::Syscall,
    supported_rights: u64,
) -> Result<(), Error> {
    let mut names = BTreeSet::new();
    for result in syscall.results {
        validate_identifier("result", result.name)?;
        if !names.insert(result.name) {
            return invalid(format!(
                "syscall {} repeats result {}",
                syscall.name, result.name
            ));
        }
        if result.handle.is_some() != (result.kind == ValueKind::Handle) {
            return invalid(format!(
                "syscall {} result {} has inconsistent handle metadata",
                syscall.name, result.name
            ));
        }
        let Some(handle) = result.handle else {
            continue;
        };
        match handle.object {
            ProducedObject::SameAsArgument(argument) => {
                if !syscall.arguments.iter().any(|candidate| {
                    candidate.name == argument && candidate.kind == ValueKind::Handle
                }) {
                    return invalid(format!(
                        "syscall {} result {} names invalid source handle {argument}",
                        syscall.name, result.name
                    ));
                }
            }
            ProducedObject::Kind(kind) => require_object_kind(schema, syscall.name, kind)?,
        }
        match handle.rights {
            ProducedRights::RequestedSubsetOf(argument) => {
                if !syscall.arguments.iter().any(|candidate| {
                    candidate.name == argument && candidate.kind == ValueKind::Rights
                }) {
                    return invalid(format!(
                        "syscall {} result {} names invalid rights argument {argument}",
                        syscall.name, result.name
                    ));
                }
            }
            ProducedRights::ExactRequested {
                argument,
                allowed_rights,
                authority_source,
            } => {
                if !syscall.arguments.iter().any(|candidate| {
                    candidate.name == argument && candidate.kind == ValueKind::Rights
                }) {
                    return invalid(format!(
                        "syscall {} result {} names invalid exact-rights argument {argument}",
                        syscall.name, result.name
                    ));
                }
                if allowed_rights == 0 || allowed_rights & !supported_rights != 0 {
                    return invalid(format!(
                        "syscall {} result {} allows undeclared or empty object rights",
                        syscall.name, result.name
                    ));
                }
                if let Some(source) = authority_source
                    && !syscall.arguments.iter().any(|candidate| {
                        candidate.name == source && candidate.kind == ValueKind::Handle
                    })
                {
                    return invalid(format!(
                        "syscall {} result {} names invalid authority source handle {source}",
                        syscall.name, result.name
                    ));
                }
            }
            ProducedRights::Fixed(rights) if rights & !supported_rights != 0 => {
                return invalid(format!(
                    "syscall {} result {} produces undeclared rights",
                    syscall.name, result.name
                ));
            }
            ProducedRights::Fixed(_) => {}
        }
    }
    let mut failure_statuses = BTreeSet::new();
    for failure in syscall.failure_results {
        validate_identifier("failure-result status", failure.status)?;
        if !schema
            .statuses
            .iter()
            .any(|status| status.name == failure.status && status.value != 0)
        {
            return invalid(format!(
                "syscall {} names unknown failure-result status {}",
                syscall.name, failure.status
            ));
        }
        if !failure_statuses.insert(failure.status) {
            return invalid(format!(
                "syscall {} repeats failure-result status {}",
                syscall.name, failure.status
            ));
        }
        if failure.results.is_empty() {
            return invalid(format!(
                "syscall {} exposes no results for failure status {}",
                syscall.name, failure.status
            ));
        }
        let mut result_names = BTreeSet::new();
        for name in failure.results {
            if !result_names.insert(*name)
                || !syscall.results.iter().any(|result| result.name == *name)
            {
                return invalid(format!(
                    "syscall {} names invalid failure result {}",
                    syscall.name, name
                ));
            }
        }
    }
    Ok(())
}
