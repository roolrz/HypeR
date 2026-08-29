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
    AbiSchema, CompletionClass, FeatureGate, FieldKind, HandleDisposition, ObjectConstraint,
    ProducedObject, ProducedRights, ValueKind,
};

const GENERATED_RUST: &str = "abi/native/experimental.rs";
const GENERATED_C: &str = "abi/native/include/hyper/experimental_native.h";
const GENERATED_REFERENCE: &str = "abi/native/experimental-reference.md";

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
    match schema.publication {
        schema::PublicationState::Experimental if schema.revision != 0 => {
            return invalid("an experimental schema must use unpublished revision zero");
        }
        schema::PublicationState::Published => {
            return invalid(
                "published ABI rendering is not implemented; retain the experimental namespace",
            );
        }
        schema::PublicationState::Experimental => {}
    }
    validate_features(schema)?;
    validate_object_kinds(schema)?;
    let supported_rights = validate_rights(schema)?;
    validate_records(schema)?;
    validate_syscalls(schema, supported_rights)
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
        validate_declaration_stability(schema, "feature", feature.name, feature.stability)?;
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
        validate_declaration_stability(schema, "object kind", object.name, object.stability)?;
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
        validate_declaration_stability(schema, "right", right.name, right.stability)?;
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

fn validate_records(schema: &AbiSchema) -> Result<(), Error> {
    let mut names = BTreeSet::new();
    for record in schema.records {
        validate_identifier("record", record.name)?;
        validate_declaration_stability(schema, "record", record.name, record.stability)?;
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
            return invalid("syscall declarations must be ordered by permanent number");
        }
        previous_number = Some(syscall.number);
        if syscall.introduced > schema.revision {
            return invalid(format!(
                "syscall {} was introduced after the schema revision",
                syscall.name
            ));
        }
        validate_declaration_stability(schema, "syscall", syscall.name, syscall.stability)?;
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
            if memory.maximum_bytes == 0 {
                return invalid(format!(
                    "syscall {} argument {} has an unbounded zero maximum",
                    syscall.name, argument.name
                ));
            }
            if !memory_orders.insert(memory.validation_order) {
                return invalid(format!(
                    "syscall {} repeats memory validation order {}",
                    syscall.name, memory.validation_order
                ));
            }
            let length = syscall
                .arguments
                .iter()
                .find(|candidate| candidate.name == memory.length_argument);
            if !matches!(length, Some(candidate) if candidate.kind == ValueKind::ByteCount) {
                return invalid(format!(
                    "syscall {} memory argument {} has no byte-count length argument",
                    syscall.name, argument.name
                ));
            }
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
                if u32::from(record.size) > memory.maximum_bytes {
                    return invalid(format!(
                        "syscall {} memory argument {} cannot contain record {record_name}",
                        syscall.name, argument.name
                    ));
                }
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

fn validate_declaration_stability(
    schema: &AbiSchema,
    domain: &str,
    name: &str,
    stability: schema::PublicationState,
) -> Result<(), Error> {
    if schema.publication == schema::PublicationState::Experimental
        && stability != schema::PublicationState::Experimental
    {
        invalid(format!(
            "{domain} {name} cannot be published by an experimental schema"
        ))
    } else {
        Ok(())
    }
}

fn invalid<T>(message: impl Into<String>) -> Result<T, Error> {
    Err(Error::InvalidSchema(message.into()))
}

fn render_rust(schema: &AbiSchema) -> String {
    let mut output = String::from(
        "// SPDX-FileCopyrightText: 2026 roolrz\n\
         // SPDX-License-Identifier: Apache-2.0\n\n\
         // Generated from abi/native/schema.rs. Do not edit.\n\
         // This ABI is experimental and unpublished. Names, numbers, and layouts may change.\n\n",
    );
    let _ = writeln!(
        output,
        "pub const HYPER_EXPERIMENTAL_ABI_REVISION: u64 = {};",
        schema.revision
    );
    let _ = writeln!(
        output,
        "pub const HYPER_EXPERIMENTAL_SYSCALL_ARGUMENT_REGISTERS: usize = {};",
        schema::SYSCALL_ARGUMENT_REGISTERS
    );
    let _ = writeln!(
        output,
        "pub const HYPER_EXPERIMENTAL_SYSCALL_RESULT_REGISTERS: usize = {};",
        schema::SYSCALL_RESULT_REGISTERS
    );
    output.push_str(
        "pub type HyperExperimentalHandle = u64;\n\
         pub type HyperExperimentalStatus = i64;\n\n",
    );
    render_rust_constants(
        &mut output,
        "HYPER_EXPERIMENTAL_FEATURE",
        schema
            .features
            .iter()
            .map(|value| (value.name, 1u64 << value.bit)),
    );
    render_rust_u32_constants(
        &mut output,
        "HYPER_EXPERIMENTAL_OBJECT",
        schema
            .object_kinds
            .iter()
            .map(|value| (value.name, value.value)),
    );
    render_rust_constants(
        &mut output,
        "HYPER_EXPERIMENTAL_RIGHT",
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
        "pub const HYPER_EXPERIMENTAL_RIGHTS_MASK: u64 = {rights_mask};\n"
    );
    render_rust_constants(
        &mut output,
        "HYPER_EXPERIMENTAL_SYS",
        schema
            .syscalls
            .iter()
            .map(|value| (value.name, u64::from(value.number))),
    );
    for record in schema.records {
        let rust_name = upper_camel(record.name);
        output.push_str("#[repr(C)]\n#[derive(Clone, Copy, Debug, Eq, PartialEq)]\n");
        let _ = writeln!(output, "pub struct HyperExperimental{rust_name} {{");
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
            "const _: () = assert!(core::mem::size_of::<HyperExperimental{rust_name}>() == {});",
            record.size
        );
        let _ = writeln!(
            output,
            "const _: () = assert!(core::mem::align_of::<HyperExperimental{rust_name}>() == {});",
            record.alignment
        );
        for field in record.fields {
            let _ = writeln!(
                output,
                "const _: () = assert!(core::mem::offset_of!(HyperExperimental{rust_name}, {}) == {});",
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

fn render_c(schema: &AbiSchema) -> String {
    let mut output = String::from("/* SPDX-FileCopyrightText: 2026 roolrz\n");
    output.push_str(" * SPDX-License-Identifier: Apache-2.0\n");
    output.push_str(" *\n");
    output.push_str(" * Generated from abi/native/schema.rs. Do not edit.\n");
    output.push_str(
        " * This ABI is experimental and unpublished. Names, numbers, and layouts may change.\n",
    );
    output.push_str(" */\n\n");
    output.push_str("#ifndef HYPER_EXPERIMENTAL_NATIVE_H\n");
    output.push_str("#define HYPER_EXPERIMENTAL_NATIVE_H\n\n");
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
        "#define HYPER_EXPERIMENTAL_ABI_REVISION UINT64_C({})",
        schema.revision
    );
    let _ = writeln!(
        output,
        "#define HYPER_EXPERIMENTAL_SYSCALL_ARGUMENT_REGISTERS UINT32_C({})",
        schema::SYSCALL_ARGUMENT_REGISTERS
    );
    let _ = writeln!(
        output,
        "#define HYPER_EXPERIMENTAL_SYSCALL_RESULT_REGISTERS UINT32_C({})",
        schema::SYSCALL_RESULT_REGISTERS
    );
    output.push_str(
        "\ntypedef uint64_t hyper_experimental_handle_t;\n\
         typedef int64_t hyper_experimental_status_t;\n\n",
    );
    render_c_constants(
        &mut output,
        "HYPER_EXPERIMENTAL_FEATURE",
        schema
            .features
            .iter()
            .map(|value| (value.name, 1u64 << value.bit)),
    );
    render_c_u32_constants(
        &mut output,
        "HYPER_EXPERIMENTAL_OBJECT",
        schema
            .object_kinds
            .iter()
            .map(|value| (value.name, value.value)),
    );
    render_c_constants(
        &mut output,
        "HYPER_EXPERIMENTAL_RIGHT",
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
        "#define HYPER_EXPERIMENTAL_RIGHTS_MASK UINT64_C({rights_mask})\n"
    );
    render_c_constants(
        &mut output,
        "HYPER_EXPERIMENTAL_SYS",
        schema
            .syscalls
            .iter()
            .map(|value| (value.name, u64::from(value.number))),
    );
    for record in schema.records {
        let _ = writeln!(
            output,
            "typedef struct hyper_experimental_{}_t {{",
            record.name
        );
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
        let _ = writeln!(output, "}} hyper_experimental_{}_t;", record.name);
        let _ = writeln!(
            output,
            "HYPER_ABI_STATIC_ASSERT(sizeof(hyper_experimental_{}_t) == {}, \"{} size\");",
            record.name, record.size, record.name
        );
        let _ = writeln!(
            output,
            "HYPER_ABI_STATIC_ASSERT(HYPER_ABI_ALIGNOF(hyper_experimental_{}_t) == {}, \"{} alignment\");",
            record.name, record.alignment, record.name
        );
        for field in record.fields {
            let _ = writeln!(
                output,
                "HYPER_ABI_STATIC_ASSERT(offsetof(hyper_experimental_{}_t, {}) == {}, \"{}.{} offset\");",
                record.name, field.name, field.offset, record.name, field.name
            );
        }
        output.push('\n');
    }
    output.push_str("#undef HYPER_ABI_ALIGNOF\n#undef HYPER_ABI_STATIC_ASSERT\n\n");
    output.push_str("#endif /* HYPER_EXPERIMENTAL_NATIVE_H */\n");
    output
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

fn render_reference(schema: &AbiSchema) -> String {
    let mut output = String::from(
        "<!--\n\
         SPDX-FileCopyrightText: 2026 roolrz\n\
         SPDX-License-Identifier: Apache-2.0\n\
         -->\n\n\
         # Experimental HypeR Native ABI reference\n\n\
         This file is generated from `abi/native/schema.rs`. Do not edit it directly.\n\
         The ABI is unpublished; every name, number, and layout remains provisional.\n\n",
    );
    let _ = writeln!(
        output,
        "Experimental ABI revision: `{}`.\n",
        schema.revision
    );
    output.push_str(
        "## Syscalls\n\n\
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
        let results = joined_values(
            syscall
                .results
                .iter()
                .map(|value| format!("`{}: {}`", value.name, value_kind_name(value.kind))),
        );
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
    format!(
        "`{name}: {:?}, len={}, max={}{}; order={}`",
        memory.direction,
        memory.length_argument,
        memory.maximum_bytes,
        record,
        memory.validation_order
    )
}

fn value_kind_name(kind: ValueKind) -> &'static str {
    match kind {
        ValueKind::U32 => "u32",
        ValueKind::U64 => "u64",
        ValueKind::I64 => "i64",
        ValueKind::Handle => "handle",
        ValueKind::UserAddress => "user_address",
        ValueKind::ByteCount => "byte_count",
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
        if let Some(memory) = arguments[1].memory.as_mut() {
            memory.length_argument = "missing";
        }
        calls[4].arguments = Box::leak(arguments.into_boxed_slice());
        let calls = Box::leak(calls.into_boxed_slice());
        let candidate = AbiSchema {
            syscalls: calls,
            ..schema::NATIVE_ABI
        };
        assert!(
            matches!(validate(&candidate), Err(Error::InvalidSchema(message)) if message.contains("byte-count"))
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
    fn rejects_published_output_until_a_stable_namespace_exists() {
        let candidate = AbiSchema {
            publication: schema::PublicationState::Published,
            revision: 1,
            ..schema::NATIVE_ABI
        };
        assert!(
            matches!(validate(&candidate), Err(Error::InvalidSchema(message)) if message.contains("not implemented"))
        );
    }

    #[test]
    fn reserves_zero_for_the_none_object_kind() {
        let kinds = Box::leak(
            vec![schema::ObjectKind {
                value: 1,
                name: "none",
                stability: schema::PublicationState::Experimental,
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
}
