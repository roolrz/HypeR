// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Capability-containment policy for staged Native process construction.

use hyper::abi::native;

/// Explicitly reviewed object kinds which may be retained by `ProcessBuilder`.
///
/// `TransferClass` alone is insufficient: a builder is a persistent capability
/// container and can outlive or cross out of the process that configured it.
/// Lifecycle-bearing Process, Thread, `TaskGroup`, and VMAR objects are excluded
/// until their ownership graphs have dedicated lifecycle tests. Nested
/// `ProcessBuilder` authority is always forbidden.
pub(crate) struct BuilderStorable;

impl BuilderStorable {
    pub(crate) const fn permits_kind_id(kind: u32) -> bool {
        kind == native::HYPER_NATIVE_OBJECT_EVENT
            || kind == native::HYPER_NATIVE_OBJECT_BYTE_CHANNEL
            || kind == native::HYPER_NATIVE_OBJECT_CAPABILITY_CHANNEL
            || kind == native::HYPER_NATIVE_OBJECT_RESOURCE_DOMAIN
            || kind == native::HYPER_NATIVE_OBJECT_TASK_FACTORY
            || kind == native::HYPER_NATIVE_OBJECT_EXECUTABLE_AUTHORITY
            || kind == native::HYPER_NATIVE_OBJECT_VMO
            || kind == native::HYPER_NATIVE_OBJECT_CONSOLE
            || kind == native::HYPER_NATIVE_OBJECT_BOOT_FS
            || kind == native::HYPER_NATIVE_OBJECT_BOOT_FILE
    }
}
