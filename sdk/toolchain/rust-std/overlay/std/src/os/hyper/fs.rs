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
}

#[stable(feature = "hyper_os", since = "1.97.1")]
impl MetadataExt for Metadata {
    fn mode(&self) -> u32 { self.as_inner().mode() }
}
