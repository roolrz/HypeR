// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Lock-free, one-shot supplements for imminent terminal crash entry.

use core::fmt;

use hyper::cpu::{CpuIndex, PerCpu};
use hyper::sync::PublishedOnce;

const SUPPLEMENT_CAPACITY: usize = 256;
type CrashSupplement = super::fixed_text::FixedText<SUPPLEMENT_CAPACITY>;

static SUPPLEMENTS: PerCpu<PublishedOnce<CrashSupplement>> =
    PerCpu::new([const { PublishedOnce::new() }; hyper::cpu::MAX_CPUS]);

/// Publishes text that the calling CPU's imminent fatal entry will consume.
///
/// This function never allocates, waits, takes a lock, or accesses a console.
/// The caller must have local interrupts masked and must take a terminal action
/// that cannot resume the interrupted execution or migrate before fatal entry.
/// The one-shot slot is intentionally never cleared: allowing a non-terminal
/// caller would otherwise attach stale context to a later unrelated crash.
#[allow(dead_code)]
pub(crate) fn publish(arguments: fmt::Arguments<'_>) -> bool {
    let Some(cpu) = super::super::cpu::current_index() else {
        return false;
    };
    SUPPLEMENTS[cpu]
        .publish(CrashSupplement::capture(arguments))
        .is_ok()
}

/// Reads the immutable supplement published by this CPU's terminal path.
pub(super) fn read_for_fatal(cpu: CpuIndex) -> Option<&'static CrashSupplement> {
    SUPPLEMENTS[cpu].get()
}
