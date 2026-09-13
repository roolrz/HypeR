// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

pub mod profile;

pub mod console;

pub mod io_backend;
pub mod io_protocol;
pub mod virtio_scsi;

#[cfg(all(target_os = "hyper", target_arch = "aarch64"))]
pub mod io_guest;
