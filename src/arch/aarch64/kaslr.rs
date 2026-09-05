// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! `AArch64` kernel virtual-address randomization policy.

pub const ALIGNMENT: u64 = 2 * 1024 * 1024;
pub const WINDOW_SIZE: u64 = 512 * 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Entropy(hyper::mm::kaslr::Error),
    ImageTooLarge,
    InvalidImage,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Layout {
    pub kernel_base: u64,
    pub offset: u64,
}

/// Selects an aligned base inside the architecture-owned KASLR window.
pub fn select(seed: Option<u64>, image_size: u64) -> Result<Layout, Error> {
    if image_size == 0 {
        return Err(Error::InvalidImage);
    }
    let offset = match seed {
        Some(seed) => hyper::mm::kaslr::select_offset(seed, image_size, WINDOW_SIZE, ALIGNMENT)
            .map_err(Error::Entropy)?,
        None => 0,
    };
    let kernel_base = super::memory::kernel_region_base()
        .checked_add(offset)
        .ok_or(Error::ImageTooLarge)?;
    Ok(Layout {
        kernel_base,
        offset,
    })
}
