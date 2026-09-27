// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Validation and deterministic rendering for the `HypeR` Native ABI schema.

mod names;
mod render;
mod validation;

use names::upper_snake;
use render::{render_c, render_reference, render_rust};
use validation::{require_record_field, validate_records};

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[path = "../schema/native.rs"]
pub mod schema;

use schema::{
    AbiSchema, CapabilityCommit, CompletionClass, FeatureGate, FieldKind, HandleDisposition,
    HandleOperation, IndirectHandles, MemoryDirection, MemoryLength, ObjectConstraint,
    OperationDisposition, ProducedObject, ProducedRights, TransferClass, ValueKind,
};

const GENERATED_RUST: &str = "src/generated.rs";
const GENERATED_C: &str = "include/hyper/native.h";
const GENERATED_REFERENCE: &str = "docs/native.md";

#[derive(Debug)]
pub enum Error {
    InvalidSchema(String),
    Io { path: PathBuf, source: io::Error },
    Drift { path: PathBuf },
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSchema(message) => {
                write!(formatter, "invalid Native ABI schema: {message}")
            }
            Self::Io { path, source } => write!(formatter, "{}: {source}", path.display()),
            Self::Drift { path } => write!(
                formatter,
                "{} is stale; run `cargo run --features generator --bin hyper-abi -- write`",
                path.display()
            ),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::InvalidSchema(_) | Self::Drift { .. } => None,
        }
    }
}

#[derive(Debug)]
pub struct GeneratedFiles {
    pub rust: String,
    pub c: String,
    pub reference: String,
}

pub fn validate(schema: &AbiSchema) -> Result<(), Error> {
    if schema.revision != 0 {
        return invalid("pre-release ABI revision must remain zero");
    }
    validate_features(schema)?;
    validate_statuses(schema)?;
    validate_object_kinds(schema)?;
    let supported_rights = validate_rights(schema)?;
    validate_signals(schema)?;
    validate_constants(schema)?;
    validate_records(schema)?;
    validate_syscalls(schema, supported_rights)?;
    validate_semantic_rules(schema)?;
    validate_generated_constant_names(schema)
}

fn validate_semantic_rules(schema: &AbiSchema) -> Result<(), Error> {
    let mut rules = BTreeSet::new();
    for rule in schema.semantic_rules {
        if rule.trim().is_empty() {
            return invalid("semantic rules must not be empty");
        }
        if !rules.insert(*rule) {
            return invalid("semantic rules must not be repeated");
        }
    }
    Ok(())
}

fn validate_statuses(schema: &AbiSchema) -> Result<(), Error> {
    let mut values = BTreeSet::new();
    let mut names = BTreeSet::new();
    for status in schema.statuses {
        validate_identifier("status", status.name)?;
        if status.value > 0 {
            return invalid(format!(
                "status {} uses positive value {}",
                status.name, status.value
            ));
        }
        if !values.insert(status.value) {
            return invalid(format!(
                "status value {} is declared more than once",
                status.value
            ));
        }
        if !names.insert(status.name) {
            return invalid(format!("status {} is declared more than once", status.name));
        }
    }
    match schema.statuses.iter().find(|status| status.name == "ok") {
        Some(status) if status.value == 0 => Ok(()),
        Some(_) => invalid("the ok status must retain value zero"),
        None => invalid("the ok status is missing"),
    }
}

pub fn generate(schema: &AbiSchema) -> Result<GeneratedFiles, Error> {
    validate(schema)?;
    Ok(GeneratedFiles {
        rust: render_rust(schema),
        c: render_c(schema),
        reference: render_reference(schema),
    })
}

pub fn write_repository_outputs(repository: &Path) -> Result<(), Error> {
    let generated = generate(&schema::NATIVE_ABI)?;
    write_output(repository, GENERATED_RUST, &generated.rust)?;
    write_output(repository, GENERATED_C, &generated.c)?;
    write_output(repository, GENERATED_REFERENCE, &generated.reference)
}

pub fn check_repository_outputs(repository: &Path) -> Result<(), Error> {
    let generated = generate(&schema::NATIVE_ABI)?;
    check_output(repository, GENERATED_RUST, &generated.rust)?;
    check_output(repository, GENERATED_C, &generated.c)?;
    check_output(repository, GENERATED_REFERENCE, &generated.reference)
}

fn validate_features(schema: &AbiSchema) -> Result<(), Error> {
    let mut bits = BTreeSet::new();
    let mut names = BTreeSet::new();
    for feature in schema.features {
        validate_identifier("feature", feature.name)?;
        if feature.bit >= 64 {
            return invalid(format!(
                "feature {} uses bit {} outside u64",
                feature.name, feature.bit
            ));
        }
        if !bits.insert(feature.bit) {
            return invalid(format!(
                "feature bit {} is declared more than once",
                feature.bit
            ));
        }
        if !names.insert(feature.name) {
            return invalid(format!(
                "feature {} is declared more than once",
                feature.name
            ));
        }
    }
    if !names.contains("core") {
        return invalid("the core feature is missing");
    }
    Ok(())
}

fn validate_object_kinds(schema: &AbiSchema) -> Result<(), Error> {
    let mut values = BTreeSet::new();
    let mut names = BTreeSet::new();
    for object in schema.object_kinds {
        validate_identifier("object kind", object.name)?;
        if !values.insert(object.value) {
            return invalid(format!(
                "object-kind value {} is declared more than once",
                object.value
            ));
        }
        if !names.insert(object.name) {
            return invalid(format!(
                "object kind {} is declared more than once",
                object.name
            ));
        }
    }
    let none = schema
        .object_kinds
        .iter()
        .find(|object| object.name == "none");
    match none {
        Some(object) if object.value == 0 && object.transfer == TransferClass::Forbidden => {}
        Some(_) => {
            return invalid(
                "the none object kind must retain reserved value zero and forbidden transfer",
            );
        }
        None => return invalid("the reserved none object kind is missing"),
    }
    Ok(())
}

fn validate_rights(schema: &AbiSchema) -> Result<u64, Error> {
    let mut bits = BTreeSet::new();
    let mut names = BTreeSet::new();
    let mut mask = 0u64;
    for right in schema.rights {
        validate_identifier("right", right.name)?;
        if right.bit >= 64 {
            return invalid(format!(
                "right {} uses bit {} outside u64",
                right.name, right.bit
            ));
        }
        if !bits.insert(right.bit) {
            return invalid(format!(
                "right bit {} is declared more than once",
                right.bit
            ));
        }
        if !names.insert(right.name) {
            return invalid(format!("right {} is declared more than once", right.name));
        }
        mask |= 1u64 << right.bit;
    }
    Ok(mask)
}

fn validate_signals(schema: &AbiSchema) -> Result<(), Error> {
    let mut names = BTreeSet::new();
    let mut bits = BTreeSet::new();
    for signal in schema.signals {
        validate_identifier("signal object", signal.object)?;
        validate_identifier("signal", signal.name)?;
        if signal.bit >= 64 {
            return invalid(format!(
                "signal {}.{} uses bit {} outside u64",
                signal.object, signal.name, signal.bit
            ));
        }
        if !schema
            .object_kinds
            .iter()
            .any(|object| object.name == signal.object && object.name != "none")
        {
            return invalid(format!(
                "signal {}.{} names unknown object kind",
                signal.object, signal.name
            ));
        }
        if !names.insert((signal.object, signal.name)) {
            return invalid(format!(
                "signal {}.{} is declared more than once",
                signal.object, signal.name
            ));
        }
        if !bits.insert((signal.object, signal.bit)) {
            return invalid(format!(
                "signal bit {} is declared more than once for {}",
                signal.bit, signal.object
            ));
        }
    }
    Ok(())
}

fn validate_constants(schema: &AbiSchema) -> Result<(), Error> {
    let mut names = BTreeSet::new();
    for constant in schema.constants {
        validate_identifier("constant", constant.name)?;
        if !names.insert(constant.name) {
            return invalid(format!(
                "constant {} is declared more than once",
                constant.name
            ));
        }
    }
    Ok(())
}

/// Rejects schema entries which render to the same public Rust/C constant.
///
/// Category-local names are insufficient here because free-form constants use
/// the root `HYPER_NATIVE_` namespace and can otherwise collide with generated
/// feature, status, object, right, signal, or syscall definitions.
fn validate_generated_constant_names(schema: &AbiSchema) -> Result<(), Error> {
    let mut names = BTreeSet::new();
    for reserved in [
        "HYPER_NATIVE_ABI_REVISION".to_owned(),
        "HYPER_NATIVE_FEATURE_MASK".to_owned(),
        "HYPER_NATIVE_RIGHTS_MASK".to_owned(),
        "HYPER_NATIVE_TRANSFER_CLASS_FORBIDDEN".to_owned(),
        "HYPER_NATIVE_TRANSFER_CLASS_GENERAL".to_owned(),
        "HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY".to_owned(),
    ] {
        names.insert(reserved);
    }

    let mut insert = |name: String| {
        if names.insert(name.clone()) {
            Ok(())
        } else {
            invalid(format!(
                "generated constant {name} is declared more than once"
            ))
        }
    };
    for feature in schema.features {
        insert(format!(
            "HYPER_NATIVE_FEATURE_{}",
            upper_snake(feature.name)
        ))?;
    }
    for status in schema.statuses {
        insert(format!("HYPER_NATIVE_STATUS_{}", upper_snake(status.name)))?;
    }
    for object in schema.object_kinds {
        insert(format!("HYPER_NATIVE_OBJECT_{}", upper_snake(object.name)))?;
    }
    for right in schema.rights {
        insert(format!("HYPER_NATIVE_RIGHT_{}", upper_snake(right.name)))?;
    }
    for signal in schema.signals {
        insert(format!(
            "HYPER_NATIVE_SIGNAL_{}_{}",
            upper_snake(signal.object),
            upper_snake(signal.name)
        ))?;
    }
    for constant in schema.constants {
        insert(format!("HYPER_NATIVE_{}", upper_snake(constant.name)))?;
    }
    for record in schema.records {
        insert(format!(
            "HYPER_NATIVE_{}_MIN_SIZE",
            upper_snake(record.name)
        ))?;
    }
    for syscall in schema.syscalls {
        insert(format!("HYPER_NATIVE_SYS_{}", upper_snake(syscall.name)))?;
    }
    Ok(())
}

fn validate_syscalls(schema: &AbiSchema, supported_rights: u64) -> Result<(), Error> {
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

fn validate_indirect_handles(
    syscall: &schema::Syscall,
    argument: &schema::Argument,
    memory: schema::UserMemory,
    handles: IndirectHandles,
    record: Option<&schema::Record>,
    supported_rights: u64,
) -> Result<(), Error> {
    if !matches!(memory.length, MemoryLength::Elements { .. }) {
        return invalid(format!(
            "syscall {} memory argument {} describes handles without element length",
            syscall.name, argument.name
        ));
    }
    match handles {
        IndirectHandles::BorrowRecords {
            handle_field,
            required_rights,
        } => {
            if memory.direction != MemoryDirection::Read {
                return invalid(format!(
                    "syscall {} borrowed-handle argument {} is not input memory",
                    syscall.name, argument.name
                ));
            }
            if required_rights & !supported_rights != 0 {
                return invalid(format!(
                    "syscall {} argument {} indirectly requires undeclared rights",
                    syscall.name, argument.name
                ));
            }
            let Some(record) = record else {
                return invalid(format!(
                    "syscall {} borrowed-handle argument {} has no record",
                    syscall.name, argument.name
                ));
            };
            require_record_field(record, handle_field, FieldKind::U64, syscall, argument)?;
        }
        IndirectHandles::ConsumeRecords {
            handle_field,
            rights_field,
            expected_kind_field,
            operation_field,
            common_rights,
            operations,
            commit: CapabilityCommit::AtomicOnOk,
        } => {
            if memory.direction != MemoryDirection::Read {
                return invalid(format!(
                    "syscall {} consumed-handle argument {} is not input memory",
                    syscall.name, argument.name
                ));
            }
            if common_rights & !supported_rights != 0 {
                return invalid(format!(
                    "syscall {} argument {} indirectly requires undeclared rights",
                    syscall.name, argument.name
                ));
            }
            let Some(record) = record else {
                return invalid(format!(
                    "syscall {} consumed-handle argument {} has no record",
                    syscall.name, argument.name
                ));
            };
            require_record_field(record, handle_field, FieldKind::U64, syscall, argument)?;
            require_record_field(record, rights_field, FieldKind::U64, syscall, argument)?;
            require_record_field(
                record,
                expected_kind_field,
                FieldKind::U32,
                syscall,
                argument,
            )?;
            require_record_field(record, operation_field, FieldKind::U32, syscall, argument)?;
            validate_distinct_fields(
                syscall,
                argument,
                &[
                    handle_field,
                    rights_field,
                    expected_kind_field,
                    operation_field,
                ],
            )?;
            validate_handle_operations(syscall.name, argument.name, operations, supported_rights)?;
            validate_transfer_operations(syscall, argument, common_rights, operations)?;
        }
        IndirectHandles::ProduceTransferred {
            handle_field,
            rights_field,
            expected_kind_field,
            flags_field,
            commit: CapabilityCommit::AtomicOnOk,
        } => {
            if memory.direction != MemoryDirection::ReadWrite {
                return invalid(format!(
                    "syscall {} transferred-handle output {} must be in/out memory",
                    syscall.name, argument.name
                ));
            }
            let Some(record) = record else {
                return invalid(format!(
                    "syscall {} transferred-handle output {} has no record",
                    syscall.name, argument.name
                ));
            };
            require_record_field(record, handle_field, FieldKind::U64, syscall, argument)?;
            require_record_field(record, rights_field, FieldKind::U64, syscall, argument)?;
            require_record_field(
                record,
                expected_kind_field,
                FieldKind::U32,
                syscall,
                argument,
            )?;
            require_record_field(record, flags_field, FieldKind::U32, syscall, argument)?;
            validate_distinct_fields(
                syscall,
                argument,
                &[handle_field, rights_field, expected_kind_field, flags_field],
            )?;
        }
    }
    Ok(())
}

fn validate_distinct_fields(
    syscall: &schema::Syscall,
    argument: &schema::Argument,
    fields: &[&str],
) -> Result<(), Error> {
    let mut names = BTreeSet::new();
    if fields.iter().all(|field| names.insert(*field)) {
        Ok(())
    } else {
        invalid(format!(
            "syscall {} memory argument {} repeats transactional record fields",
            syscall.name, argument.name
        ))
    }
}

fn validate_transfer_operations(
    syscall: &schema::Syscall,
    argument: &schema::Argument,
    common_rights: u64,
    operations: &[HandleOperation],
) -> Result<(), Error> {
    let move_operation = operations.iter().find(|operation| operation.name == "move");
    let duplicate_operation = operations
        .iter()
        .find(|operation| operation.name == "duplicate");
    if !matches!(
        move_operation,
        Some(HandleOperation {
            value: 0,
            disposition: OperationDisposition::ConsumeOnCommit,
            ..
        })
    ) || common_rights & schema::RIGHT_TRANSFER == 0
    {
        return invalid(format!(
            "syscall {} argument {} has an invalid move operation",
            syscall.name, argument.name
        ));
    }
    if !matches!(
        duplicate_operation,
        Some(HandleOperation {
            value: 1,
            disposition: OperationDisposition::Borrow,
            additional_rights,
            ..
        }) if *additional_rights & schema::RIGHT_DUPLICATE != 0
    ) || common_rights & schema::RIGHT_TRANSFER == 0
    {
        return invalid(format!(
            "syscall {} argument {} has an invalid duplicate operation",
            syscall.name, argument.name
        ));
    }
    Ok(())
}

fn validate_handle_operations(
    syscall: &str,
    argument: &str,
    operations: &[HandleOperation],
    supported_rights: u64,
) -> Result<(), Error> {
    if operations.is_empty() {
        return invalid(format!(
            "syscall {syscall} handle {argument} has no operations"
        ));
    }
    let mut names = BTreeSet::new();
    let mut values = BTreeSet::new();
    for operation in operations {
        validate_identifier("handle operation", operation.name)?;
        if !names.insert(operation.name) || !values.insert(operation.value) {
            return invalid(format!(
                "syscall {syscall} handle {argument} repeats a handle operation"
            ));
        }
        if operation.additional_rights & !supported_rights != 0 {
            return invalid(format!(
                "syscall {syscall} handle {argument} operation {} requires undeclared rights",
                operation.name
            ));
        }
    }
    Ok(())
}

fn require_object_kind(schema: &AbiSchema, syscall: &str, kind: &str) -> Result<(), Error> {
    if schema.object_kinds.iter().any(|object| object.name == kind) {
        Ok(())
    } else {
        invalid(format!(
            "syscall {syscall} names unknown object kind {kind}"
        ))
    }
}

fn validate_identifier(domain: &str, identifier: &str) -> Result<(), Error> {
    let mut bytes = identifier.bytes();
    let Some(first) = bytes.next() else {
        return invalid(format!("{domain} name is empty"));
    };
    if !first.is_ascii_lowercase()
        || !bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return invalid(format!(
            "{domain} name {identifier:?} is not lower snake case"
        ));
    }
    Ok(())
}

pub(super) fn invalid<T>(message: impl Into<String>) -> Result<T, Error> {
    Err(Error::InvalidSchema(message.into()))
}

fn write_output(repository: &Path, relative: &str, contents: &str) -> Result<(), Error> {
    let path = repository.join(relative);
    let Some(parent) = path.parent() else {
        return invalid(format!("generated path {relative} has no parent"));
    };
    fs::create_dir_all(parent).map_err(|source| Error::Io {
        path: parent.to_path_buf(),
        source,
    })?;
    fs::write(&path, contents).map_err(|source| Error::Io { path, source })
}

fn check_output(repository: &Path, relative: &str, expected: &str) -> Result<(), Error> {
    let path = repository.join(relative);
    let actual = fs::read_to_string(&path).map_err(|source| Error::Io {
        path: path.clone(),
        source,
    })?;
    if actual == expected {
        Ok(())
    } else {
        Err(Error::Drift { path })
    }
}

#[cfg(test)]
mod tests;
