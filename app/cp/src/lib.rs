// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
#[cfg(target_os = "hyper")]
use std::os::hyper::fs::{MetadataExt, symlink};
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, symlink};
use std::path::{Path, PathBuf};
use std::{fs, io};

#[derive(Debug, Parser)]
#[command(
    name = "cp",
    about = "Copy files and directories; -R preserves symbolic links"
)]
pub struct Args {
    #[arg(short = 'R', visible_short_alias = 'r', long)]
    pub recursive: bool,
    /// Source paths followed by the destination. Multiple sources require a directory.
    #[arg(required = true, num_args = 2..)]
    pub paths: Vec<PathBuf>,
}

fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    #[cfg(target_os = "hyper")]
    {
        left.identity() == right.identity()
    }
    #[cfg(unix)]
    {
        (left.dev(), left.ino()) == (right.dev(), right.ino())
    }
}

fn destination_metadata(path: &Path) -> io::Result<Option<fs::Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(io::Error::other("refusing to overwrite a symbolic link"))
        }
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn check_directory_target(source: &Path, target: &Path) -> io::Result<()> {
    let source = fs::canonicalize(source)?;
    let target = match fs::canonicalize(target) {
        Ok(path) => path,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let parent = target
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            let name = target
                .file_name()
                .ok_or_else(|| io::Error::other("destination has no file name"))?;
            fs::canonicalize(parent)?.join(name)
        }
        Err(error) => return Err(error),
    };
    if target.starts_with(source) {
        return Err(io::Error::other("cannot copy a directory into itself"));
    }
    Ok(())
}

enum Work {
    Copy(PathBuf, PathBuf),
    FinishDirectory(PathBuf, fs::Permissions),
}

pub fn copy(source: &Path, target: &Path, recursive: bool) -> io::Result<()> {
    // Keep directory depth off the application stack. Apply new directory
    // permissions after copying their children, so restrictive modes work too.
    let mut pending = vec![Work::Copy(source.to_path_buf(), target.to_path_buf())];
    while let Some(work) = pending.pop() {
        let (source, target) = match work {
            Work::Copy(source, target) => (source, target),
            Work::FinishDirectory(path, permissions) => {
                fs::set_permissions(path, permissions)?;
                continue;
            }
        };
        let metadata = if recursive {
            fs::symlink_metadata(&source)?
        } else {
            fs::metadata(&source)?
        };
        let destination = destination_metadata(&target)?;
        if destination
            .as_ref()
            .is_some_and(|m| same_file(&metadata, m))
        {
            return Err(io::Error::other("source and destination are the same file"));
        }
        if metadata.file_type().is_symlink() {
            symlink(fs::read_link(&source)?, &target)?;
        } else if metadata.is_dir() {
            if !recursive {
                return Err(io::Error::other("is a directory (use -R)"));
            }
            check_directory_target(&source, &target)?;
            if let Some(metadata) = &destination {
                if !metadata.is_dir() {
                    return Err(io::Error::other("destination is not a directory"));
                }
            } else {
                fs::create_dir(&target)?;
                pending.push(Work::FinishDirectory(
                    target.clone(),
                    metadata.permissions(),
                ));
            }
            for entry in fs::read_dir(&source)? {
                let entry = entry?;
                pending.push(Work::Copy(entry.path(), target.join(entry.file_name())));
            }
        } else if metadata.is_file() {
            fs::copy(&source, &target)?;
        } else {
            return Err(io::Error::other("unsupported source file type"));
        }
    }
    Ok(())
}

pub fn run(args: Args) -> bool {
    let Some((destination, sources)) = args.paths.split_last() else {
        return false;
    };
    let directory = destination.is_dir();
    if sources.len() > 1 && !directory {
        eprintln!(
            "cp: {}: multiple sources require a destination directory",
            destination.display()
        );
        return false;
    }
    let mut success = true;
    for source in sources {
        let contents = source
            .as_os_str()
            .as_encoded_bytes()
            .rsplit(|byte| *byte == b'/')
            .find(|part| !part.is_empty())
            == Some(b".");
        let target = if directory && !contents {
            match source.file_name() {
                Some(name) => destination.join(name),
                None => {
                    eprintln!("cp: {}: source has no file name", source.display());
                    success = false;
                    continue;
                }
            }
        } else {
            destination.clone()
        };
        if let Err(error) = copy(source, &target, args.recursive) {
            eprintln!("cp: {} -> {}: {error}", source.display(), target.display());
            success = false;
        }
    }
    success
}

#[cfg(test)]
#[path = "../tests/operations.rs"]
mod tests;
