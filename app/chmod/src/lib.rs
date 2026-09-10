// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
#[cfg(target_os = "hyper")]
use std::os::hyper::fs::PermissionsExt;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::{fs, io};

#[derive(Debug, Parser)]
#[command(
    name = "chmod",
    about = "Change permissions using octal modes or symbolic clauses such as u+rw,go-rwx"
)]
pub struct Args {
    /// Recurse through directories without following nested symbolic links.
    #[arg(short = 'R', long)]
    pub recursive: bool,
    /// Octal 0000..7777 or comma-separated [ugoa][+-=][rwxXstugo] clauses. Omitted who means all.
    pub mode: Mode,
    #[arg(required = true)]
    pub paths: Vec<PathBuf>,
}

#[derive(Clone, Debug)]
pub enum Mode {
    Octal(u32),
    Symbolic(Vec<Clause>),
}
#[derive(Clone, Debug)]
pub struct Clause {
    who: u32,
    operation: u8,
    permissions: Vec<u8>,
}

impl std::str::FromStr for Mode {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, String> {
        let invalid =
            || "expected an octal mode or symbolic clauses such as u+rw,go-rwx".to_string();
        if !value.is_empty() && value.bytes().all(|b| (b'0'..=b'7').contains(&b)) {
            let mode = u32::from_str_radix(value, 8).map_err(|_| invalid())?;
            return if mode <= 0o7777 {
                Ok(Self::Octal(mode))
            } else {
                Err(invalid())
            };
        }
        let mut clauses = Vec::new();
        for text in value.split(',') {
            let bytes = text.as_bytes();
            let mut offset = 0;
            let mut who = 0;
            while let Some(byte) = bytes.get(offset) {
                who |= match byte {
                    b'u' => 0o4700,
                    b'g' => 0o2070,
                    b'o' => 0o1007,
                    b'a' => 0o7777,
                    _ => break,
                };
                offset += 1;
            }
            if who == 0 {
                who = 0o7777;
            }
            let operation = *bytes.get(offset).ok_or_else(invalid)?;
            if !b"+-=".contains(&operation) {
                return Err(invalid());
            }
            let permissions = &bytes[offset + 1..];
            if !permissions.iter().all(|b| b"rwxXstugo".contains(b)) {
                return Err(invalid());
            }
            if permissions.iter().any(|b| b"ugo".contains(b)) && permissions.len() != 1 {
                return Err(invalid());
            }
            clauses.push(Clause {
                who,
                operation,
                permissions: permissions.to_vec(),
            });
        }
        Ok(Self::Symbolic(clauses))
    }
}

impl Mode {
    pub fn apply(&self, original: u32, directory: bool) -> u32 {
        let clauses = match self {
            Self::Octal(mode) => return *mode,
            Self::Symbolic(clauses) => clauses,
        };
        let mut mode = original & 0o7777;
        for clause in clauses {
            let mut bits = 0;
            for permission in &clause.permissions {
                bits |= match permission {
                    b'r' => 0o444,
                    b'w' => 0o222,
                    b'x' => 0o111,
                    b'X' if directory || mode & 0o111 != 0 => 0o111,
                    b's' => 0o6000,
                    b't' => 0o1000,
                    b'u' | b'g' | b'o' => {
                        let shift = match permission {
                            b'u' => 6,
                            b'g' => 3,
                            _ => 0,
                        };
                        let triplet = (mode >> shift) & 7;
                        triplet | triplet << 3 | triplet << 6
                    }
                    _ => 0,
                };
            }
            bits &= clause.who;
            mode = match clause.operation {
                b'+' => mode | bits,
                b'-' => mode & !bits,
                _ => (mode & !clause.who) | bits,
            };
        }
        mode
    }
}

pub fn change(path: &Path, mode: &Mode, recursive: bool) -> io::Result<()> {
    let mut pending = vec![(path.to_path_buf(), false, true)];
    while let Some((path, visited, operand)) = pending.pop() {
        let link_metadata = fs::symlink_metadata(&path)?;
        if link_metadata.file_type().is_symlink() && !operand {
            continue;
        }
        let metadata = fs::metadata(&path)?;
        if recursive && link_metadata.is_dir() && !visited {
            pending.push((path.clone(), true, operand));
            for entry in fs::read_dir(&path)? {
                pending.push((entry?.path(), false, false));
            }
            continue;
        }
        fs::set_permissions(
            &path,
            fs::Permissions::from_mode(
                mode.apply(metadata.permissions().mode(), metadata.is_dir()),
            ),
        )?;
    }
    Ok(())
}

pub fn run(args: Args) -> bool {
    let mut success = true;
    for path in args.paths {
        if let Err(error) = change(&path, &args.mode, args.recursive) {
            eprintln!("chmod: {}: {error}", path.display());
            success = false;
        }
    }
    success
}

#[cfg(test)]
#[path = "../tests/operations.rs"]
mod tests;
