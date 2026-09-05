// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Immutable RAM filesystem imported from a validated `newc` archive.
//!
//! Nodes borrow their names and payloads from the retained archive. The index
//! owns only sorted metadata, so lookup does not reparse attacker-controlled
//! input and mounting does not duplicate file contents.

use alloc::vec::Vec;

use crate::archive::cpio;

const MAXIMUM_NODES: usize = 65_536;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Allocation,
    Archive(cpio::Error),
    DuplicatePath,
    InvalidPath,
    TooManyNodes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NodeKind {
    File,
    Directory,
    Symlink,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Node<'archive> {
    path: &'archive str,
    data: &'archive [u8],
    mode: u32,
    kind: NodeKind,
}

impl<'archive> Node<'archive> {
    pub const fn path(self) -> &'archive str {
        self.path
    }

    pub const fn data(self) -> &'archive [u8] {
        self.data
    }

    pub const fn mode(self) -> u32 {
        self.mode
    }

    pub const fn kind(self) -> NodeKind {
        self.kind
    }

    pub const fn is_executable(self) -> bool {
        self.mode & 0o111 != 0
    }
}

pub struct RamFs<'archive> {
    nodes: Vec<Node<'archive>>,
}

impl<'archive> RamFs<'archive> {
    pub fn from_newc(bytes: &'archive [u8]) -> Result<Self, Error> {
        let archive = cpio::Archive::new(bytes).map_err(Error::Archive)?;
        let mut count = 0usize;
        for entry in archive.entries() {
            let entry = entry.map_err(Error::Archive)?;
            if canonical_archive_path(entry.name(), entry.kind())?.is_some() {
                count = count.checked_add(1).ok_or(Error::TooManyNodes)?;
                if count > MAXIMUM_NODES {
                    return Err(Error::TooManyNodes);
                }
            }
        }

        let mut nodes = Vec::new();
        nodes
            .try_reserve_exact(count)
            .map_err(|_| Error::Allocation)?;
        for entry in archive.entries() {
            let entry = entry.map_err(Error::Archive)?;
            let Some(path) = canonical_archive_path(entry.name(), entry.kind())? else {
                continue;
            };
            nodes.push(Node {
                path,
                data: entry.data(),
                mode: entry.mode(),
                kind: map_kind(entry.kind()),
            });
        }
        nodes.sort_unstable_by(|left, right| left.path.cmp(right.path));
        if nodes.windows(2).any(|pair| pair[0].path == pair[1].path) {
            return Err(Error::DuplicatePath);
        }
        Ok(Self { nodes })
    }

    pub fn lookup(&self, absolute_path: &str) -> Result<Option<Node<'archive>>, Error> {
        let path = canonical_lookup_path(absolute_path)?;
        Ok(self
            .nodes
            .binary_search_by(|node| node.path.cmp(path))
            .ok()
            .map(|index| self.nodes[index]))
    }

    pub fn nodes(&self) -> impl ExactSizeIterator<Item = Node<'archive>> + '_ {
        self.nodes.iter().copied()
    }
}

fn canonical_archive_path(path: &str, kind: cpio::EntryKind) -> Result<Option<&str>, Error> {
    if path == "." {
        return if kind == cpio::EntryKind::Directory {
            Ok(None)
        } else {
            Err(Error::InvalidPath)
        };
    }
    let path = path.strip_prefix("./").unwrap_or(path);
    validate_relative_path(path)?;
    Ok(Some(path))
}

fn canonical_lookup_path(path: &str) -> Result<&str, Error> {
    let path = path.strip_prefix('/').ok_or(Error::InvalidPath)?;
    validate_relative_path(path)?;
    Ok(path)
}

fn validate_relative_path(path: &str) -> Result<(), Error> {
    if path.is_empty() || path.starts_with('/') || path.ends_with('/') {
        return Err(Error::InvalidPath);
    }
    if path
        .split('/')
        .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Err(Error::InvalidPath);
    }
    Ok(())
}

const fn map_kind(kind: cpio::EntryKind) -> NodeKind {
    match kind {
        cpio::EntryKind::File => NodeKind::File,
        cpio::EntryKind::Directory => NodeKind::Directory,
        cpio::EntryKind::Symlink => NodeKind::Symlink,
        cpio::EntryKind::Other => NodeKind::Other,
    }
}
