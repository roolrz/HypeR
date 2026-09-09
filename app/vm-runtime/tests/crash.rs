// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Explicit non-default acceptance fixture: exit without running destructors
//! while the guest is producing output and a console client is attached.

use std::sync::atomic::{AtomicUsize, Ordering};

static COLLECTED: AtomicUsize = AtomicUsize::new(0);

pub(super) fn after_output(count: usize) {
    if COLLECTED.fetch_add(count, Ordering::Relaxed) + count >= 512 {
        std::process::exit(93);
    }
}
