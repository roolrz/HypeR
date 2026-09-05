// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Initial root-filesystem ownership and lookup policy.

#[cfg(not(feature = "kernel-self-test"))]
use hyper::fs::ramfs::Node;
use hyper::fs::ramfs::RamFs;
use hyper::sync::PublishedOnce;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InitializationError {
    Archive(hyper::fs::ramfs::Error),
    AlreadyInitialized,
}

#[cfg(not(feature = "kernel-self-test"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LookupError {
    InvalidPath,
    NotInitialized,
}

static ROOT: PublishedOnce<RamFs<'static>> = PublishedOnce::new();

pub(crate) fn initialize(boot: &super::boot::Initialization) -> Result<(), InitializationError> {
    let root = RamFs::from_newc(boot.initial_ramdisk()).map_err(InitializationError::Archive)?;
    let node_count = root.nodes().len();
    ROOT.publish(root)
        .map_err(|_| InitializationError::AlreadyInitialized)?;
    crate::println!("HypeR: mounted initramfs with {node_count} node(s)");
    Ok(())
}

#[cfg(not(feature = "kernel-self-test"))]
pub(crate) fn lookup(path: &str) -> Result<Option<Node<'static>>, LookupError> {
    let root = ROOT.get().ok_or(LookupError::NotInitialized)?;
    root.lookup(path).map_err(|_| LookupError::InvalidPath)
}
