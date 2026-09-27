// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Ordered schema admission and scalar namespace validation.

mod handles;
mod records;
mod syscalls;

use super::names::upper_snake;
use super::{Error, schema};
use records::validate_records;
use schema::{AbiSchema, TransferClass};
use std::collections::BTreeSet;
use syscalls::validate_syscalls;

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
