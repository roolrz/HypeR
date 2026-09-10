// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native filesystem metadata extensions.
#![stable(feature = "hyper_os", since = "1.97.1")]

use crate::fs::Metadata;
use crate::sys::AsInner;

/// Access to the Native file permission mode.
#[stable(feature = "hyper_os", since = "1.97.1")]
pub trait MetadataExt {
    /// Returns permission and special mode bits; file type is queried separately.
    #[stable(feature = "hyper_os", since = "1.97.1")]
    fn mode(&self) -> u32;
    /// Returns filesystem and node identity, shared by hard links to one node.
    #[stable(feature = "hyper_os", since = "1.97.1")]
    fn identity(&self) -> (u64, u64);
}

#[stable(feature = "hyper_os", since = "1.97.1")]
impl MetadataExt for Metadata {
    fn mode(&self) -> u32 {
        self.as_inner().mode()
    }
    fn identity(&self) -> (u64, u64) {
        self.as_inner().identity()
    }
}

/// Creates a Native symbolic link. Relative targets resolve from its parent.
#[stable(feature = "hyper_os", since = "1.97.1")]
pub fn symlink<P: AsRef<crate::path::Path>, Q: AsRef<crate::path::Path>>(
    target: P,
    link: Q,
) -> crate::io::Result<()> {
    crate::sys::fs::symlink(target.as_ref(), link.as_ref())
}
/// Native mode-bit construction without assuming Unix credentials.
#[stable(feature = "hyper_os", since = "1.97.1")]
pub trait PermissionsExt {
    #[stable(feature = "hyper_os", since = "1.97.1")]
    fn mode(&self) -> u32;
    #[stable(feature = "hyper_os", since = "1.97.1")]
    fn set_mode(&mut self, mode: u32);
    #[stable(feature = "hyper_os", since = "1.97.1")]
    fn from_mode(mode: u32) -> Self;
}
#[stable(feature = "hyper_os", since = "1.97.1")]
impl PermissionsExt for crate::fs::Permissions {
    fn mode(&self) -> u32 {
        self.as_inner().mode()
    }
    fn set_mode(&mut self, mode: u32) {
        *self = Self::from_mode(mode);
    }
    fn from_mode(mode: u32) -> Self {
        use crate::sys::FromInner;
        Self::from_inner(crate::sys::fs::FilePermissions::from_mode(mode))
    }
}

/// Updates timestamps by path, including directories. Symbolic links are followed.
#[stable(feature = "hyper_os", since = "1.97.1")]
pub fn set_times<P: AsRef<crate::path::Path>>(
    path: P,
    mut times: crate::fs::FileTimes,
) -> crate::io::Result<()> {
    use crate::sys::AsInnerMut;
    crate::sys::fs::set_times(path.as_ref(), *times.as_inner_mut())
}
