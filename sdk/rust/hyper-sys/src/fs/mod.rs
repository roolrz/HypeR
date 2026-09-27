// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Filesystem syscall bindings grouped by namespace and open-file operations.

mod directory;
mod file;

pub use directory::{
    directory_canonicalize, directory_create_directory, directory_create_file, directory_get_info,
    directory_get_metadata, directory_get_self_metadata, directory_link, directory_open_directory,
    directory_open_directory_nofollow, directory_open_file, directory_open_file_with_options,
    directory_read, directory_read_link, directory_remove, directory_remove_if, directory_rename,
    directory_scope_create, directory_set_metadata, directory_symlink,
};
pub use file::{
    file_create_executable_vmo, file_create_snapshot, file_get_info, file_get_metadata, file_lock,
    file_read_at, file_resize, file_set_metadata, file_sync, file_unlock, file_write_at,
};
