// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Validation and deterministic rendering for the `HypeR` Native ABI schema.

use std::collections::BTreeSet;
use std::fmt::{self, Write as _};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[path = "../../../abi/native/schema.rs"]
pub mod schema;

use schema::{
    AbiSchema, CompletionClass, FeatureGate, FieldKind, HandleDisposition, IndirectHandles,
    MemoryDirection, MemoryLength, ObjectConstraint, ProducedObject, ProducedRights, ValueKind,
};

const GENERATED_RUST: &str = "abi/native/generated.rs";
const GENERATED_C: &str = "abi/native/include/hyper/native.h";
const GENERATED_REFERENCE: &str = "abi/native/reference.md";

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
                "{} is stale; run `make generate-abi`",
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
    validate_generated_constant_names(schema)
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
    match schema
        .object_kinds
        .iter()
        .find(|object| object.name == "none")
    {
        Some(object) if object.value == 0 => Ok(()),
        Some(_) => invalid("the none object kind must retain reserved value zero"),
        None => invalid("the reserved none object kind is missing"),
    }
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
    for syscall in schema.syscalls {
        insert(format!("HYPER_NATIVE_SYS_{}", upper_snake(syscall.name)))?;
    }
    Ok(())
}

fn validate_records(schema: &AbiSchema) -> Result<(), Error> {
    let mut names = BTreeSet::new();
    for record in schema.records {
        validate_identifier("record", record.name)?;
        if !names.insert(record.name) {
            return invalid(format!("record {} is declared more than once", record.name));
        }
        if !matches!(record.alignment, 1 | 2 | 4 | 8) || record.size == 0 {
            return invalid(format!(
                "record {} has an invalid size or alignment",
                record.name
            ));
        }
        if u16::from(record.alignment) > record.size
            || record.size % u16::from(record.alignment) != 0
        {
            return invalid(format!("record {} size is not aligned", record.name));
        }

        let mut field_names = BTreeSet::new();
        let mut end = 0u16;
        let mut rendered_alignment = 1u8;
        for field in record.fields {
            validate_identifier("field", field.name)?;
            if !field_names.insert(field.name) {
                return invalid(format!(
                    "record {} repeats field {}",
                    record.name, field.name
                ));
            }
            if field.offset < end {
                return invalid(format!(
                    "record {} fields overlap or are out of order",
                    record.name
                ));
            }
            if field.offset % u16::from(field.kind.alignment()) != 0 {
                return invalid(format!(
                    "record {} field {} is misaligned",
                    record.name, field.name
                ));
            }
            rendered_alignment = rendered_alignment.max(field.kind.alignment());
            end = field.offset.checked_add(field.kind.size()).ok_or_else(|| {
                Error::InvalidSchema(format!("record {} field range overflows", record.name))
            })?;
            if end > record.size {
                return invalid(format!(
                    "record {} field {} exceeds its size",
                    record.name, field.name
                ));
            }
        }
        if record.alignment != rendered_alignment {
            return invalid(format!(
                "record {} declares alignment {} but its fields render alignment {}",
                record.name, record.alignment, rendered_alignment
            ));
        }
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
            if handle.disposition == HandleDisposition::ConsumeOnCommit && argument.name.is_empty()
            {
                return invalid(format!(
                    "syscall {} has an unnamed consumed handle",
                    syscall.name
                ));
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
            if !matches!(length, Some(candidate) if candidate.kind == expected_length_kind) {
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
        IndirectHandles::ConsumeRecords {
            handle_field,
            rights_field,
            expected_kind_field,
            required_rights,
        } => {
            if memory.direction != MemoryDirection::Read {
                return invalid(format!(
                    "syscall {} consumed-handle argument {} is not input memory",
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
        }
        IndirectHandles::ProduceTransferred => {
            if memory.direction != MemoryDirection::Write
                || !matches!(
                    memory.length,
                    MemoryLength::Elements {
                        element_size: 8,
                        ..
                    }
                )
                || memory.record.is_some()
            {
                return invalid(format!(
                    "syscall {} transferred-handle output {} must be a raw u64 element array",
                    syscall.name, argument.name
                ));
            }
        }
    }
    Ok(())
}

fn require_record_field(
    record: &schema::Record,
    field: &str,
    kind: FieldKind,
    syscall: &schema::Syscall,
    argument: &schema::Argument,
) -> Result<(), Error> {
    if record
        .fields
        .iter()
        .any(|candidate| candidate.name == field && candidate.kind == kind)
    {
        Ok(())
    } else {
        invalid(format!(
            "syscall {} memory argument {} names invalid {} field {}",
            syscall.name, argument.name, record.name, field
        ))
    }
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

fn invalid<T>(message: impl Into<String>) -> Result<T, Error> {
    Err(Error::InvalidSchema(message.into()))
}

fn render_rust(schema: &AbiSchema) -> String {
    let mut output = String::from(
        "// SPDX-FileCopyrightText: 2026 roolrz\n\
         // SPDX-License-Identifier: Apache-2.0\n\n\
         // Generated from abi/native/schema.rs. Do not edit.\n\n",
    );
    let _ = writeln!(
        output,
        "pub const HYPER_NATIVE_ABI_REVISION: u64 = {};",
        schema.revision
    );
    let _ = writeln!(
        output,
        "pub const HYPER_NATIVE_SYSCALL_ARGUMENT_REGISTERS: usize = {};",
        schema::SYSCALL_ARGUMENT_REGISTERS
    );
    let _ = writeln!(
        output,
        "pub const HYPER_NATIVE_SYSCALL_RESULT_REGISTERS: usize = {};",
        schema::SYSCALL_RESULT_REGISTERS
    );
    output.push_str(
        "pub type HyperNativeHandle = u64;\n\
         pub type HyperNativeStatus = i64;\n\n",
    );
    render_rust_constants(
        &mut output,
        "HYPER_NATIVE_FEATURE",
        schema
            .features
            .iter()
            .map(|value| (value.name, 1u64 << value.bit)),
    );
    render_rust_i64_constants(
        &mut output,
        "HYPER_NATIVE_STATUS",
        schema
            .statuses
            .iter()
            .map(|value| (value.name, value.value)),
    );
    render_rust_u32_constants(
        &mut output,
        "HYPER_NATIVE_OBJECT",
        schema
            .object_kinds
            .iter()
            .map(|value| (value.name, value.value)),
    );
    render_rust_constants(
        &mut output,
        "HYPER_NATIVE_RIGHT",
        schema
            .rights
            .iter()
            .map(|value| (value.name, 1u64 << value.bit)),
    );
    let rights_mask = schema
        .rights
        .iter()
        .fold(0u64, |mask, right| mask | (1u64 << right.bit));
    let _ = writeln!(
        output,
        "pub const HYPER_NATIVE_RIGHTS_MASK: u64 = {rights_mask};\n"
    );
    for signal in schema.signals {
        let _ = writeln!(
            output,
            "pub const HYPER_NATIVE_SIGNAL_{}_{}: u64 = {};",
            upper_snake(signal.object),
            upper_snake(signal.name),
            1u64 << signal.bit
        );
    }
    if !schema.signals.is_empty() {
        output.push('\n');
    }
    render_rust_constants(
        &mut output,
        "HYPER_NATIVE",
        schema
            .constants
            .iter()
            .map(|constant| (constant.name, constant.value)),
    );
    render_rust_constants(
        &mut output,
        "HYPER_NATIVE_SYS",
        schema
            .syscalls
            .iter()
            .map(|value| (value.name, u64::from(value.number))),
    );
    render_rust_failure_result_mask(&mut output, schema);
    for record in schema.records {
        let rust_name = upper_camel(record.name);
        output.push_str("#[repr(C)]\n#[derive(Clone, Copy, Debug, Eq, PartialEq)]\n");
        let _ = writeln!(output, "pub struct HyperNative{rust_name} {{");
        let mut cursor = 0u16;
        let mut padding = 0usize;
        for field in record.fields {
            if field.offset > cursor {
                let _ = writeln!(
                    output,
                    "    pub _padding{padding}: [u8; {}],",
                    field.offset - cursor
                );
                padding += 1;
            }
            let _ = writeln!(
                output,
                "    pub {}: {},",
                field.name,
                rust_field_type(field.kind)
            );
            cursor = field.offset + field.kind.size();
        }
        if record.size > cursor {
            let _ = writeln!(
                output,
                "    pub _padding{padding}: [u8; {}],",
                record.size - cursor
            );
        }
        output.push_str("}\n");
        let _ = writeln!(
            output,
            "const _: () = assert!(core::mem::size_of::<HyperNative{rust_name}>() == {});",
            record.size
        );
        let _ = writeln!(
            output,
            "const _: () = assert!(core::mem::align_of::<HyperNative{rust_name}>() == {});",
            record.alignment
        );
        for field in record.fields {
            let _ = writeln!(
                output,
                "const _: () = assert!(core::mem::offset_of!(HyperNative{rust_name}, {}) == {});",
                field.name, field.offset
            );
        }
        output.push('\n');
    }
    if output.ends_with("\n\n") {
        output.pop();
    }
    output
}

fn render_rust_failure_result_mask(output: &mut String, schema: &AbiSchema) {
    output.push_str(
        "pub const fn hyper_native_failure_result_mask(\n    syscall_number: u64,\n    status: HyperNativeStatus,\n) -> u64 {\n    match (syscall_number, status) {\n",
    );
    for syscall in schema.syscalls {
        for failure in syscall.failure_results {
            let mut mask = 0u64;
            for result_name in failure.results {
                if let Some(index) = syscall
                    .results
                    .iter()
                    .position(|result| result.name == *result_name)
                {
                    mask |= 1u64 << index;
                }
            }
            let _ = writeln!(
                output,
                "        (HYPER_NATIVE_SYS_{}, HYPER_NATIVE_STATUS_{}) => {mask},",
                upper_snake(syscall.name),
                upper_snake(failure.status)
            );
        }
    }
    output.push_str("        _ => 0,\n    }\n}\n\n");
}

fn render_rust_constants<'a>(
    output: &mut String,
    prefix: &str,
    values: impl Iterator<Item = (&'a str, u64)>,
) {
    for (name, value) in values {
        let _ = writeln!(
            output,
            "pub const {prefix}_{}: u64 = {value};",
            upper_snake(name)
        );
    }
    output.push('\n');
}

fn render_rust_u32_constants<'a>(
    output: &mut String,
    prefix: &str,
    values: impl Iterator<Item = (&'a str, u32)>,
) {
    for (name, value) in values {
        let _ = writeln!(
            output,
            "pub const {prefix}_{}: u32 = {value};",
            upper_snake(name)
        );
    }
    output.push('\n');
}

fn render_rust_i64_constants<'a>(
    output: &mut String,
    prefix: &str,
    values: impl Iterator<Item = (&'a str, i64)>,
) {
    for (name, value) in values {
        let _ = writeln!(
            output,
            "pub const {prefix}_{}: HyperNativeStatus = {value};",
            upper_snake(name)
        );
    }
    output.push('\n');
}

fn render_c(schema: &AbiSchema) -> String {
    let mut output = String::from("/* SPDX-FileCopyrightText: 2026 roolrz\n");
    output.push_str(" * SPDX-License-Identifier: Apache-2.0\n");
    output.push_str(" *\n");
    output.push_str(" * Generated from abi/native/schema.rs. Do not edit.\n");
    output.push_str(" */\n\n");
    output.push_str("#ifndef HYPER_NATIVE_H\n");
    output.push_str("#define HYPER_NATIVE_H\n\n");
    output.push_str("#include <stddef.h>\n#include <stdint.h>\n\n");
    output.push_str("#if defined(__cplusplus)\n");
    output.push_str("#define HYPER_ABI_STATIC_ASSERT static_assert\n");
    output.push_str("#define HYPER_ABI_ALIGNOF alignof\n");
    output.push_str("#else\n");
    output.push_str("#define HYPER_ABI_STATIC_ASSERT _Static_assert\n");
    output.push_str("#define HYPER_ABI_ALIGNOF _Alignof\n");
    output.push_str("#endif\n\n");
    let _ = writeln!(
        output,
        "#define HYPER_NATIVE_ABI_REVISION UINT64_C({})",
        schema.revision
    );
    let _ = writeln!(
        output,
        "#define HYPER_NATIVE_SYSCALL_ARGUMENT_REGISTERS UINT32_C({})",
        schema::SYSCALL_ARGUMENT_REGISTERS
    );
    let _ = writeln!(
        output,
        "#define HYPER_NATIVE_SYSCALL_RESULT_REGISTERS UINT32_C({})",
        schema::SYSCALL_RESULT_REGISTERS
    );
    output.push_str(
        "\ntypedef uint64_t hyper_native_handle_t;\n\
         typedef int64_t hyper_native_status_t;\n\n",
    );
    render_c_constants(
        &mut output,
        "HYPER_NATIVE_FEATURE",
        schema
            .features
            .iter()
            .map(|value| (value.name, 1u64 << value.bit)),
    );
    render_c_i64_constants(
        &mut output,
        "HYPER_NATIVE_STATUS",
        schema
            .statuses
            .iter()
            .map(|value| (value.name, value.value)),
    );
    render_c_u32_constants(
        &mut output,
        "HYPER_NATIVE_OBJECT",
        schema
            .object_kinds
            .iter()
            .map(|value| (value.name, value.value)),
    );
    render_c_constants(
        &mut output,
        "HYPER_NATIVE_RIGHT",
        schema
            .rights
            .iter()
            .map(|value| (value.name, 1u64 << value.bit)),
    );
    let rights_mask = schema
        .rights
        .iter()
        .fold(0u64, |mask, right| mask | (1u64 << right.bit));
    let _ = writeln!(
        output,
        "#define HYPER_NATIVE_RIGHTS_MASK UINT64_C({rights_mask})\n"
    );
    for signal in schema.signals {
        let _ = writeln!(
            output,
            "#define HYPER_NATIVE_SIGNAL_{}_{} UINT64_C({})",
            upper_snake(signal.object),
            upper_snake(signal.name),
            1u64 << signal.bit
        );
    }
    if !schema.signals.is_empty() {
        output.push('\n');
    }
    render_c_constants(
        &mut output,
        "HYPER_NATIVE",
        schema
            .constants
            .iter()
            .map(|constant| (constant.name, constant.value)),
    );
    render_c_constants(
        &mut output,
        "HYPER_NATIVE_SYS",
        schema
            .syscalls
            .iter()
            .map(|value| (value.name, u64::from(value.number))),
    );
    render_c_failure_result_mask(&mut output, schema);
    for record in schema.records {
        let _ = writeln!(output, "typedef struct hyper_native_{}_t {{", record.name);
        let mut cursor = 0u16;
        let mut padding = 0usize;
        for field in record.fields {
            if field.offset > cursor {
                let _ = writeln!(
                    output,
                    "    uint8_t _padding{padding}[{}];",
                    field.offset - cursor
                );
                padding += 1;
            }
            let _ = writeln!(output, "    {} {};", c_field_type(field.kind), field.name);
            cursor = field.offset + field.kind.size();
        }
        if record.size > cursor {
            let _ = writeln!(
                output,
                "    uint8_t _padding{padding}[{}];",
                record.size - cursor
            );
        }
        let _ = writeln!(output, "}} hyper_native_{}_t;", record.name);
        let _ = writeln!(
            output,
            "HYPER_ABI_STATIC_ASSERT(sizeof(hyper_native_{}_t) == {}, \"{} size\");",
            record.name, record.size, record.name
        );
        let _ = writeln!(
            output,
            "HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_native_{}_t) == {}, \"{} alignment\");",
            record.name, record.alignment, record.name
        );
        for field in record.fields {
            let _ = writeln!(
                output,
                "HYPER_ABI_STATIC_ASSERT(offsetof(hyper_native_{}_t, {}) == {}, \"{}.{} offset\");",
                record.name, field.name, field.offset, record.name, field.name
            );
        }
        output.push('\n');
    }
    output.push_str("#undef HYPER_ABI_ALIGNOF\n#undef HYPER_ABI_STATIC_ASSERT\n\n");
    output.push_str("#endif /* HYPER_NATIVE_H */\n");
    output
}

fn render_c_failure_result_mask(output: &mut String, schema: &AbiSchema) {
    output.push_str(
        "static inline uint64_t hyper_native_failure_result_mask(\n    uint64_t syscall_number, hyper_native_status_t status)\n{\n",
    );
    for syscall in schema.syscalls {
        for failure in syscall.failure_results {
            let mut mask = 0u64;
            for result_name in failure.results {
                if let Some(index) = syscall
                    .results
                    .iter()
                    .position(|result| result.name == *result_name)
                {
                    mask |= 1u64 << index;
                }
            }
            let _ = writeln!(
                output,
                "    if (syscall_number == HYPER_NATIVE_SYS_{} &&\n        status == HYPER_NATIVE_STATUS_{}) {{\n        return UINT64_C({mask});\n    }}",
                upper_snake(syscall.name),
                upper_snake(failure.status)
            );
        }
    }
    output.push_str("    return UINT64_C(0);\n}\n\n");
}

fn render_c_constants<'a>(
    output: &mut String,
    prefix: &str,
    values: impl Iterator<Item = (&'a str, u64)>,
) {
    for (name, value) in values {
        let _ = writeln!(
            output,
            "#define {prefix}_{} UINT64_C({value})",
            upper_snake(name)
        );
    }
    output.push('\n');
}

fn render_c_u32_constants<'a>(
    output: &mut String,
    prefix: &str,
    values: impl Iterator<Item = (&'a str, u32)>,
) {
    for (name, value) in values {
        let _ = writeln!(
            output,
            "#define {prefix}_{} UINT32_C({value})",
            upper_snake(name)
        );
    }
    output.push('\n');
}

fn render_c_i64_constants<'a>(
    output: &mut String,
    prefix: &str,
    values: impl Iterator<Item = (&'a str, i64)>,
) {
    for (name, value) in values {
        let rendered = if value < 0 {
            format!("(-INT64_C({}))", value.unsigned_abs())
        } else {
            format!("INT64_C({value})")
        };
        let _ = writeln!(output, "#define {prefix}_{} {rendered}", upper_snake(name));
    }
    output.push('\n');
}

fn render_reference(schema: &AbiSchema) -> String {
    let mut output = String::from(
        "<!--\n\
         SPDX-FileCopyrightText: 2026 roolrz\n\
         SPDX-License-Identifier: Apache-2.0\n\
         -->\n\n\
         # HypeR Native ABI reference\n\n\
         This file is generated from `abi/native/schema.rs`. Do not edit it directly.\n\n",
    );
    let _ = writeln!(output, "ABI revision: `{}`.\n", schema.revision);
    output.push_str("## Status values\n\n| Value | Name |\n| ---: | --- |\n");
    for status in schema.statuses {
        let _ = writeln!(output, "| {} | `{}` |", status.value, status.name);
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
    output.push_str("\n## Public records\n\n| Name | Size | Alignment | Fields |\n| --- | ---: | ---: | --- |\n");
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
            "| `{}` | {} | {} | {} |",
            record.name, record.size, record.alignment, fields
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
    format!(
        "`{name}: {:?}, {object}, rights=0x{:x}`",
        handle.disposition, handle.required_rights
    )
}

fn describe_produced_handle(name: &str, handle: schema::ProducedHandle) -> String {
    let object = match handle.object {
        ProducedObject::SameAsArgument(argument) => format!("same-as({argument})"),
        ProducedObject::Kind(kind) => format!("kind={kind}"),
    };
    let rights = match handle.rights {
        ProducedRights::RequestedSubsetOf(argument) => format!("subset-from({argument})"),
        ProducedRights::Fixed(mask) => format!("fixed=0x{mask:x}"),
    };
    format!("`{name}: produce, {object}, {rights}`")
}

fn describe_user_memory(name: &str, memory: schema::UserMemory) -> String {
    let record = memory
        .record
        .map_or_else(String::new, |record| format!(", record={record}"));
    let length = match memory.length {
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
        Some(IndirectHandles::ConsumeRecords {
            handle_field,
            rights_field,
            expected_kind_field,
            required_rights,
        }) => format!(
            ", consume-handles=({handle_field}, {rights_field}, {expected_kind_field}), required-rights=0x{required_rights:x}"
        ),
        Some(IndirectHandles::ProduceTransferred) => String::from(", produce-transferred-handles"),
    };
    format!(
        "`{name}: {:?}, {}{}{}; order={}`",
        memory.direction, length, record, handles, memory.validation_order
    )
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

fn field_kind_name(kind: FieldKind) -> &'static str {
    match kind {
        FieldKind::U32 => "u32",
        FieldKind::U64 => "u64",
    }
}

fn rust_field_type(kind: FieldKind) -> &'static str {
    field_kind_name(kind)
}

fn c_field_type(kind: FieldKind) -> &'static str {
    match kind {
        FieldKind::U32 => "uint32_t",
        FieldKind::U64 => "uint64_t",
    }
}

fn upper_snake(identifier: &str) -> String {
    identifier.to_ascii_uppercase()
}

fn upper_camel(identifier: &str) -> String {
    let mut output = String::new();
    for component in identifier.split('_') {
        let mut characters = component.chars();
        if let Some(first) = characters.next() {
            output.extend(first.to_uppercase());
            output.extend(characters);
        }
    }
    output
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
mod tests {
    use super::*;

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
        let mut arguments = calls[13].arguments.to_vec();
        arguments[5].kind = ValueKind::ByteCount;
        calls[13].arguments = Box::leak(arguments.into_boxed_slice());
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
        let mut arguments = calls[13].arguments.to_vec();
        if let Some(memory) = arguments[4].memory.as_mut()
            && let MemoryLength::Elements { element_size, .. } = &mut memory.length
        {
            *element_size = 8;
        }
        calls[13].arguments = Box::leak(arguments.into_boxed_slice());
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
        let mut arguments = calls[13].arguments.to_vec();
        if let Some(memory) = arguments[4].memory.as_mut() {
            memory.handles = Some(IndirectHandles::ConsumeRecords {
                handle_field: "missing",
                rights_field: "rights",
                expected_kind_field: "expected_kind",
                required_rights: schema::RIGHT_TRANSFER,
            });
        }
        calls[13].arguments = Box::leak(arguments.into_boxed_slice());
        let calls = Box::leak(calls.into_boxed_slice());
        let candidate = AbiSchema {
            syscalls: calls,
            ..schema::NATIVE_ABI
        };
        assert!(
            matches!(validate(&candidate), Err(Error::InvalidSchema(message)) if message.contains("invalid channel_disposition field"))
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
    fn channel_read_declares_buffer_size_results_on_failure() {
        let syscall = &schema::SYSCALLS[14];
        assert_eq!(syscall.name, "channel_read");
        assert_eq!(syscall.failure_results.len(), 1);
        assert_eq!(syscall.failure_results[0].status, "buffer_too_small");
        assert_eq!(
            syscall.failure_results[0].results,
            &["actual_bytes", "actual_handles"]
        );
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
}
