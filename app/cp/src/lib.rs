// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
#[cfg(target_os = "hyper")]
use std::os::hyper::fs::{MetadataExt, symlink};
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, symlink};
use std::path::{Path, PathBuf};
use std::{fs, io};

#[derive(Debug, Default, Parser)]
#[command(
    name = "cp",
    about = "Copy files and directories; -R preserves symbolic links"
)]
pub struct Args {
    #[command(flatten)]
    pub options: CopyOptions,
    /// Treat DEST as an exact path, even when it is a directory.
    #[arg(short = 'T', long)]
    pub no_target_directory: bool,
    /// Source paths followed by the destination. Multiple sources require a directory.
    #[arg(required = true, num_args = 2..)]
    pub paths: Vec<PathBuf>,
}

#[derive(Debug, Default, clap::Args)]
pub struct CopyOptions {
    #[arg(short = 'R', visible_short_alias = 'r', long)]
    pub recursive: bool,
    /// Skip existing destination entries; new regular files use exclusive creation.
    #[arg(short = 'n', long)]
    pub no_clobber: bool,
    /// Report copied files and links.
    #[arg(short = 'v', long)]
    pub verbose: bool,
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

fn destination_metadata(path: &Path, no_clobber: bool) -> io::Result<Option<fs::Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() && !no_clobber => {
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

fn permissions(path: &Path, permissions: fs::Permissions) -> io::Result<()> {
    match fs::set_permissions(path, permissions) {
        // A data copy is useful on filesystems such as FAT which cannot
        // represent Unix permission bits. Other failures remain errors.
        Err(error) if error.kind() == io::ErrorKind::Unsupported => Ok(()),
        result => result,
    }
}

fn copy_regular(
    source: &Path,
    target: &Path,
    mode: fs::Permissions,
    no_clobber: bool,
) -> io::Result<bool> {
    let mut source = fs::File::open(source)?;
    let mut options = fs::OpenOptions::new();
    options.write(true);
    if no_clobber {
        options.create_new(true);
    } else {
        options.create(true).truncate(true);
    }
    let mut destination = match options.open(target) {
        Ok(file) => file,
        Err(error) if no_clobber && error.kind() == io::ErrorKind::AlreadyExists => {
            return Ok(false);
        }
        Err(error) => return Err(error),
    };
    io::copy(&mut source, &mut destination)?;
    destination.set_permissions(mode).or_else(|error| {
        if error.kind() == io::ErrorKind::Unsupported {
            Ok(())
        } else {
            Err(error)
        }
    })?;
    Ok(true)
}

pub fn copy(
    source: &Path,
    target: &Path,
    options: &CopyOptions,
    output: &mut impl io::Write,
) -> io::Result<()> {
    // Keep directory depth off the application stack. Apply new directory
    // permissions after copying their children, so restrictive modes work too.
    let mut pending = vec![Work::Copy(source.to_path_buf(), target.to_path_buf())];
    while let Some(work) = pending.pop() {
        let (source, target) = match work {
            Work::Copy(source, target) => (source, target),
            Work::FinishDirectory(path, permissions) => {
                self::permissions(&path, permissions)?;
                continue;
            }
        };
        let metadata = if options.recursive {
            fs::symlink_metadata(&source)?
        } else {
            fs::metadata(&source)?
        };
        if metadata.is_dir() && !options.recursive {
            return Err(io::Error::other("is a directory (use -R)"));
        }
        let destination = destination_metadata(&target, options.no_clobber)?;
        if options.no_clobber
            && destination
                .as_ref()
                .is_some_and(|dest| !(metadata.is_dir() && dest.is_dir()))
        {
            continue;
        }
        if destination
            .as_ref()
            .is_some_and(|m| same_file(&metadata, m))
        {
            return Err(io::Error::other("source and destination are the same file"));
        }
        if metadata.file_type().is_symlink() {
            match symlink(fs::read_link(&source)?, &target) {
                Err(error)
                    if options.no_clobber && error.kind() == io::ErrorKind::AlreadyExists =>
                {
                    continue;
                }
                result => result?,
            }
        } else if metadata.is_dir() {
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
            continue;
        } else if metadata.is_file() {
            if !copy_regular(&source, &target, metadata.permissions(), options.no_clobber)? {
                continue;
            }
        } else {
            return Err(io::Error::other("unsupported source file type"));
        }
        if options.verbose {
            writeln!(output, "{} -> {}", source.display(), target.display())?;
        }
    }
    Ok(())
}

pub fn run(args: Args) -> bool {
    let Some((destination, sources)) = args.paths.split_last() else {
        return false;
    };
    let directory = !args.no_target_directory && destination.is_dir();
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
        if let Err(error) = copy(source, &target, &args.options, &mut io::stdout().lock()) {
            eprintln!("cp: {} -> {}: {error}", source.display(), target.display());
            success = false;
        }
    }
    success
}

#[cfg(test)]
#[path = "../tests/operations.rs"]
mod tests;
