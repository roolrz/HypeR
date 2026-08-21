// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use core::fmt;

/// Minimal byte-oriented output contract used during early kernel startup.
///
/// Rich terminal policy belongs above this interface. Drivers only provide a
/// reliable byte sink, which keeps this contract usable across architectures.
pub trait Console: Sync {
    fn write_byte(&self, byte: u8);

    fn write_bytes(&self, bytes: &[u8]) {
        for &byte in bytes {
            if byte == b'\n' {
                self.write_byte(b'\r');
            }
            self.write_byte(byte);
        }
    }
}

pub struct ConsoleWriter<'a>(pub &'a dyn Console);

impl fmt::Write for ConsoleWriter<'_> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.0.write_bytes(text.as_bytes());
        Ok(())
    }
}
