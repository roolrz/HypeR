// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Deterministic C output for a validated ABI schema.

use super::super::names::upper_snake;
use super::super::schema;
use super::transfer_class_constant;
use schema::{AbiSchema, FieldKind};
use std::fmt::Write as _;

pub(in super::super) fn render_c(schema: &AbiSchema) -> String {
    let mut output = String::from("/* SPDX-FileCopyrightText: 2026 roolrz\n");
    output.push_str(" * SPDX-License-Identifier: Apache-2.0\n");
    output.push_str(" *\n");
    output.push_str(" * Generated from schema/native.rs. Do not edit.\n");
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
    render_c_bit_constants(
        &mut output,
        "HYPER_NATIVE_FEATURE",
        schema.features.iter().map(|value| (value.name, value.bit)),
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
    render_c_transfer_classes(&mut output, schema);
    render_c_bit_constants(
        &mut output,
        "HYPER_NATIVE_RIGHT",
        schema.rights.iter().map(|value| (value.name, value.bit)),
    );
    let rights_mask = schema
        .rights
        .iter()
        .fold(0u64, |mask, right| mask | (1u64 << right.bit));
    let _ = writeln!(
        output,
        "#define HYPER_NATIVE_RIGHTS_MASK UINT64_C({rights_mask:#x})\n"
    );
    for signal in schema.signals {
        let _ = writeln!(
            output,
            "#define HYPER_NATIVE_SIGNAL_{}_{} (UINT64_C(1) << {})",
            upper_snake(signal.object),
            upper_snake(signal.name),
            signal.bit
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
        let _ = writeln!(
            output,
            "#define HYPER_NATIVE_{}_MIN_SIZE UINT64_C({})",
            upper_snake(record.name),
            record.minimum_size
        );
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
            match field.kind {
                FieldKind::Bytes(size) => {
                    let _ = writeln!(output, "    uint8_t {}[{size}];", field.name);
                }
                kind => {
                    let _ = writeln!(output, "    {} {};", c_scalar_field_type(kind), field.name);
                }
            }
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

fn render_c_transfer_classes(output: &mut String, schema: &AbiSchema) {
    output.push_str("#define HYPER_NATIVE_TRANSFER_CLASS_FORBIDDEN UINT32_C(0)\n");
    output.push_str("#define HYPER_NATIVE_TRANSFER_CLASS_GENERAL UINT32_C(1)\n");
    output.push_str("#define HYPER_NATIVE_TRANSFER_CLASS_RENDEZVOUS_ONLY UINT32_C(2)\n\n");
    output.push_str(
        "static inline uint32_t hyper_native_object_transfer_class(uint32_t object_kind) {\n",
    );
    output.push_str("    switch (object_kind) {\n");
    for object in schema.object_kinds {
        let _ = writeln!(
            output,
            "        case HYPER_NATIVE_OBJECT_{}: return {};",
            upper_snake(object.name),
            transfer_class_constant(object.transfer)
        );
    }
    output.push_str("        default: return HYPER_NATIVE_TRANSFER_CLASS_FORBIDDEN;\n");
    output.push_str("    }\n");
    output.push_str("}\n\n");
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

fn render_c_bit_constants<'a>(
    output: &mut String,
    prefix: &str,
    values: impl Iterator<Item = (&'a str, u8)>,
) {
    for (name, bit) in values {
        let _ = writeln!(
            output,
            "#define {prefix}_{} (UINT64_C(1) << {bit})",
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

fn c_scalar_field_type(kind: FieldKind) -> &'static str {
    match kind {
        FieldKind::I64 => "int64_t",
        FieldKind::U32 => "uint32_t",
        FieldKind::U64 => "uint64_t",
        FieldKind::Bytes(_) => unreachable!("byte arrays require declarator-aware rendering"),
    }
}
