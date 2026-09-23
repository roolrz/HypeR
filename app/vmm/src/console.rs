// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Bounded keyboard buffering; local control never waits for guest consumption.
use std::collections::VecDeque;

const ESCAPE: u8 = 0x1d;
pub const INPUT_CAPACITY: usize = hyper_os::channel::MAX_MESSAGE_BYTES;

#[derive(Debug, PartialEq, Eq)]
pub enum InputAction {
    Continue,
    Menu,
    Resume,
    Detach,
    Overflow,
}

#[derive(Default)]
pub struct Input {
    pending: VecDeque<u8>,
    menu: bool,
    overflow_reported: bool,
}

impl Input {
    pub fn push(&mut self, byte: u8) -> InputAction {
        if self.menu {
            self.menu = false;
            return match byte {
                b'd' | b'q' | ESCAPE => InputAction::Detach,
                _ => InputAction::Resume,
            };
        }
        if byte == ESCAPE {
            self.menu = true;
            return InputAction::Menu;
        }
        if self.pending.len() == INPUT_CAPACITY {
            if !self.overflow_reported {
                self.overflow_reported = true;
                return InputAction::Overflow;
            }
        } else {
            self.pending.push_back(byte);
        }
        InputAction::Continue
    }

    pub fn pending(&mut self) -> &[u8] {
        self.pending.make_contiguous()
    }

    pub fn is_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    pub fn sent(&mut self) {
        self.pending.clear();
        self.overflow_reported = false;
    }
}

#[cfg(test)]
#[path = "../tests/console.rs"]
mod tests;
