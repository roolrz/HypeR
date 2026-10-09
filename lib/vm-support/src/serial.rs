// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Userspace UART models shared by the business-VM and I/O-VM runtimes.
//! Model ownership stays on one worker; console transport is a bounded duplex
//! `ByteChannel`. Saturation defers guest MMIO instead of discarding output.

/// Maximum byte payload in either direction of the UART transport.
pub const MESSAGE_BYTES: usize = 4096;
const _: () = assert!(MESSAGE_BYTES <= hyper_os::channel::MAX_MESSAGE_BYTES);

pub mod ns16550;
pub mod pl011;
#[allow(dead_code)]
pub mod pl011_registers;
mod worker;
pub use worker::Port;

#[cfg(test)]
#[path = "../tests/ns16550.rs"]
mod ns16550_tests;
#[cfg(test)]
#[path = "../tests/pl011.rs"]
mod pl011_tests;
