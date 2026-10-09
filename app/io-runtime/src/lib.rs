// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::panic))]

#[cfg(target_os = "hyper")]
extern crate hyper_vm_policy_shared as hyper_vm_policy;
#[cfg(target_os = "hyper")]
extern crate hyper_vm_support_shared as hyper_vm_support;

pub mod clients;

pub mod config;

pub mod device_policy;

pub mod deadline;

pub mod admission_policy;

pub mod broker_exchange;

mod firmware;

pub mod sdhci;

pub mod rp1;

pub mod pci;

pub mod guest_log;
