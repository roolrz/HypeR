// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use crate::graph::{Failure, Graph, Object};
use std::collections::BTreeSet;
use std::io::{self, Write};

pub fn escaped(text: &str) -> String {
    let mut result = String::new();
    for character in text.chars() {
        if character.is_control() || character == '\\' {
            result.extend(character.escape_default());
        } else {
            result.push(character);
        }
    }
    result
}

pub fn write(graph: &Graph, tree: bool, verbose: bool, output: &mut impl Write) -> io::Result<()> {
    if verbose {
        writeln!(
            output,
            "    library directory: {}",
            escaped(&graph.library_directory.to_string_lossy())
        )?;
        metadata(&graph.objects[0], "    ", output)?;
    }
    if graph.objects.len() == 1 && graph.objects[0].dependencies.is_empty() {
        let is_static = graph.objects[0]
            .image
            .as_ref()
            .is_ok_and(|image| image.static_executable);
        writeln!(
            output,
            "    {}",
            if is_static {
                "statically linked"
            } else {
                "no shared-library dependencies"
            }
        )?;
        return Ok(());
    }
    if tree {
        let mut shown = BTreeSet::from([0]);
        let mut active = BTreeSet::from([0]);
        branch(graph, 0, "    ", &mut shown, &mut active, verbose, output)
    } else {
        for object in graph.objects.iter().skip(1) {
            row(object, "    ", "", output)?;
            if verbose {
                metadata(object, "        ", output)?;
            }
        }
        Ok(())
    }
}

fn row(object: &Object, prefix: &str, suffix: &str, output: &mut impl Write) -> io::Result<()> {
    write!(output, "{prefix}{} => ", escaped(&object.name))?;
    let path = escaped(&object.path.to_string_lossy());
    match &object.image {
        Err(Failure::NotFound) => write!(output, "not found (searched {path})")?,
        Err(Failure::Invalid(error)) => write!(output, "{path} [{}]", escaped(error))?,
        Ok(_) => write!(output, "{path}")?,
    }
    if object.interpreter {
        write!(output, " (interpreter)")?;
    }
    writeln!(output, "{suffix}")
}

fn metadata(object: &Object, prefix: &str, output: &mut impl Write) -> io::Result<()> {
    if let Ok(image) = &object.image {
        write!(
            output,
            "{prefix}ELF64 {}, OS ABI {:#x}, ABI version {}",
            image.architecture.name(),
            image.os_abi,
            image.abi_version
        )?;
        if let Some(soname) = &image.soname {
            write!(output, ", SONAME {}", escaped(soname))?;
        }
        writeln!(output)?;
    }
    Ok(())
}

fn branch(
    graph: &Graph,
    parent: usize,
    prefix: &str,
    shown: &mut BTreeSet<usize>,
    active: &mut BTreeSet<usize>,
    verbose: bool,
    output: &mut impl Write,
) -> io::Result<()> {
    let children = &graph.objects[parent].dependencies;
    for (position, &index) in children.iter().enumerate() {
        let last = position + 1 == children.len();
        let object = &graph.objects[index];
        let repeated = !shown.insert(index);
        let suffix = if active.contains(&index) {
            " [cycle]"
        } else if repeated {
            " [already shown]"
        } else {
            ""
        };
        row(
            object,
            &format!("{prefix}{}", if last { "`-- " } else { "|-- " }),
            suffix,
            output,
        )?;
        let next = format!("{prefix}{}", if last { "    " } else { "|   " });
        if verbose {
            metadata(object, &next, output)?;
        }
        if !repeated {
            active.insert(index);
            branch(graph, index, &next, shown, active, verbose, output)?;
            active.remove(&index);
        }
    }
    Ok(())
}
