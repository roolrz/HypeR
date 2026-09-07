// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Symbolic manifest names for standard process-construction authorities.

use hyper_os::handle::DirectoryObject;
use hyper_os::startup::StartupPurpose;

pub const ROOT_DIRECTORY_NAME: &str = "process.root-directory";
pub const TASK_FACTORY_NAME: &str = "process.task-factory";
pub const TASK_GROUP_NAME: &str = "process.task-group";
pub const RESOURCE_DOMAIN_NAME: &str = "process.resource-domain";
pub const TASK_INSPECTOR_NAME: &str = "process.task-inspector";
pub const OBJECT_INSPECTOR_NAME: &str = "process.object-inspector";

/// Conventional current-directory authority delegated to command processes.
pub const WORKING_DIRECTORY: StartupPurpose<DirectoryObject> = StartupPurpose::new(0x8004_0001);
