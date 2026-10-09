// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Exercise the dispatcher protocol against production continuation slots.
//! Mutexes stand in for the endpoint and Event signal locks; the Event is only
//! a prompt. A completed request must never acknowledge another CPU's prompt.

use crate::require_ok;
use hyper::vm::device::mmio::PendingMmio;
use hyper::vm::exit::MmioAction;
use loom::sync::{Arc, Mutex};
use loom::thread;

#[test]
fn clearing_before_scanning_preserves_requests_from_two_vcpus() {
    loom::model(|| {
        let slots = Arc::new([
            Mutex::new(PendingMmio::new()),
            Mutex::new(PendingMmio::new()),
        ]);
        let event = Arc::new(Mutex::new(false));
        let producers: Vec<_> = (0..2)
            .map(|cpu| {
                let slots = slots.clone();
                let event = event.clone();
                thread::spawn(move || {
                    {
                        let mut slot = require_ok(slots[cpu].lock());
                        require_ok(slot.stage_firmware_console(7, b'A' + cpu as u8));
                        require_ok(slot.publish());
                    }
                    *require_ok(event.lock()) = true;
                })
            })
            .collect();
        // This ordering is the userspace DeviceDoorbell contract.
        *require_ok(event.lock()) = false;
        for slot in slots.iter() {
            let mut slot = require_ok(slot.lock());
            if let Some(request) = slot.pending() {
                require_ok(slot.complete(request.id, MmioAction::CompleteWrite));
            }
        }
        for producer in producers {
            require_ok(producer.join());
        }
        // Joining is intentional: test durable work/prompt ownership after all
        // publishers complete, rather than claiming a memory-ordering proof.
        let pending = slots
            .iter()
            .any(|slot| require_ok(slot.lock()).pending().is_some());
        assert!(
            !pending || *require_ok(event.lock()),
            "published device work lost its wake prompt"
        );
    });
}
