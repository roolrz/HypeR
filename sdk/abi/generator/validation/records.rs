// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Record layout, alignment, and typed field validation.

use super::{Error, invalid, schema, validate_identifier};
use schema::{AbiSchema, FieldKind};
use std::collections::BTreeSet;

pub(in super::super) fn validate_records(schema: &AbiSchema) -> Result<(), Error> {
    let mut names = BTreeSet::new();
    for record in schema.records {
        validate_identifier("record", record.name)?;
        if !names.insert(record.name) {
            return invalid(format!("record {} is declared more than once", record.name));
        }
        if !matches!(record.alignment, 1 | 2 | 4 | 8)
            || record.minimum_size == 0
            || record.size == 0
        {
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
        if record.minimum_size > record.size {
            return invalid(format!(
                "record {} minimum size exceeds its current size",
                record.name
            ));
        }

        let mut field_names = BTreeSet::new();
        let mut end = 0u16;
        let mut rendered_alignment = 1u8;
        for field in record.fields {
            validate_identifier("field", field.name)?;
            if matches!(field.kind, FieldKind::Bytes(0)) {
                return invalid(format!(
                    "record {} field {} has an empty byte array",
                    record.name, field.name
                ));
            }
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
            if field.offset < record.minimum_size && end > record.minimum_size {
                return invalid(format!(
                    "record {} minimum size splits field {}",
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

pub(in super::super) fn require_record_field(
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
