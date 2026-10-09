// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Dependency discovery against one explicit library directory.

use crate::elf::{self, Image};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

// Includes the main image and interpreter, as in sdk/loader's object table.
const MAX_IMAGES: usize = 16;

#[derive(Debug)]
pub enum Failure {
    NotFound,
    Invalid(String),
}

#[derive(Debug)]
pub struct Object {
    pub name: String,
    pub path: PathBuf,
    pub image: Result<Image, Failure>,
    pub interpreter: bool,
    pub dependencies: Vec<usize>,
}

pub struct Graph {
    pub library_directory: PathBuf,
    pub objects: Vec<Object>,
}

impl Graph {
    pub fn failed(&self) -> bool {
        self.objects.iter().any(|object| object.image.is_err())
    }
}

fn open_image(path: &Path) -> io::Result<(PathBuf, Image)> {
    let file = File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(elf::invalid("not a regular file"));
    }
    let image = elf::read(file)?;
    // Diagnostic paths are resolved through the caller's filesystem authority;
    // this does not grant the inspected image any new loader capabilities.
    Ok((fs::canonicalize(path)?, image))
}

/// Inspects the current files without mapping executable pages or invoking the
/// interpreter. Names are scoped to one directory, just as for Native rtld;
/// file changes during the scan are not a process-wide atomic snapshot.
pub fn inspect(path: &Path, library_directory: Option<&Path>, direct: bool) -> io::Result<Graph> {
    let input_name = path.file_name().and_then(|name| name.to_str());
    let (path, image) = open_image(path)?;
    let directory = library_directory.map(Path::to_path_buf).unwrap_or_else(|| {
        PathBuf::from(format!("/lib/{}-hyper-hyper", image.architecture.name()))
    });
    let mut builder = Builder {
        graph: Graph {
            library_directory: directory,
            objects: vec![Object {
                name: String::from("<main>"),
                path,
                image: Ok(image),
                interpreter: false,
                dependencies: Vec::new(),
            }],
        },
        names: BTreeMap::new(),
        direct,
    };
    let root = &builder.graph.objects[0];
    let root_image = root
        .image
        .as_ref()
        .map_err(|_| elf::invalid("missing root metadata"))?;
    let interpreter = root_image.interpreter.clone();
    let name = if interpreter.is_none() && !root_image.static_executable {
        // Lookup identity is the requested name, even when diagnostics show a
        // canonical path through a versioned-library symlink.
        input_name.unwrap_or("<main>")
    } else {
        "<main>"
    };
    builder.names.insert(name.to_owned(), 0);
    if let Some(path) = interpreter {
        let name = format!("ld-hyper-{}.so", root_image.architecture.name());
        let index = builder.add(&name, Path::new(&path), true)?;
        builder.graph.objects[0].dependencies.push(index);
        if !direct {
            builder.expand(index)?;
        }
    }
    builder.expand(0)?;
    Ok(builder.graph)
}

struct Builder {
    graph: Graph,
    names: BTreeMap<String, usize>,
    direct: bool,
}

impl Builder {
    fn add(&mut self, name: &str, path: &Path, interpreter: bool) -> io::Result<usize> {
        if self.graph.objects.len() == MAX_IMAGES {
            return Err(elf::invalid(
                "dependency graph exceeds Native loader limit of 16 images",
            ));
        }
        let mut resolved_path = path.to_path_buf();
        let result = open_image(path).and_then(|(resolved, image)| {
            resolved_path = resolved;
            let root = self.graph.objects[0]
                .image
                .as_ref()
                .map_err(|_| elf::invalid("missing root metadata"))?;
            if image.architecture != root.architecture {
                return Err(elf::invalid(format!(
                    "wrong architecture: {}, expected {}",
                    image.architecture.name(),
                    root.architecture.name()
                )));
            }
            if (image.os_abi, image.abi_version) != (root.os_abi, root.abi_version) {
                return Err(elf::invalid("ELF OS ABI or ABI version differs from input"));
            }
            Ok(image)
        });
        let object = Object {
            name: name.to_owned(),
            path: resolved_path,
            interpreter,
            dependencies: Vec::new(),
            image: result.map_err(|error| {
                if error.kind() == io::ErrorKind::NotFound {
                    Failure::NotFound
                } else {
                    Failure::Invalid(error.to_string())
                }
            }),
        };
        let index = self.graph.objects.len();
        // Publish before visiting dependencies so cycles terminate and each
        // exact DT_NEEDED name is opened only once during this scan.
        self.names.insert(name.to_owned(), index);
        self.graph.objects.push(object);
        Ok(index)
    }

    fn expand(&mut self, parent: usize) -> io::Result<()> {
        let Ok(image) = &self.graph.objects[parent].image else {
            return Ok(());
        };
        let needed = image.needed.clone();
        for name in needed {
            let (index, fresh) = if let Some(index) = self.names.get(&name) {
                (*index, false)
            } else {
                let path = self.graph.library_directory.join(&name);
                (self.add(&name, &path, false)?, true)
            };
            if !self.graph.objects[parent].dependencies.contains(&index) {
                self.graph.objects[parent].dependencies.push(index);
            }
            if fresh && !self.direct {
                self.expand(index)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../tests/graph.rs"]
mod tests;
