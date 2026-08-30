// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Ownership proof used by native-user translation activation.

/// Kernel-owned identity bound to one admitted native-user run.
///
/// These values are diagnostic and stale-return protection, not authority.
/// Process policy mints a fresh nonzero run generation only after closing all
/// fallible preparation and keeps it admitted until architecture exit is
/// acknowledged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UserRunBinding {
    thread: u64,
    image_generation: u64,
    run_generation: u64,
}

impl UserRunBinding {
    pub const fn new(thread: u64, image_generation: u64, run_generation: u64) -> Option<Self> {
        if thread == 0 || image_generation == 0 || run_generation == 0 {
            None
        } else {
            Some(Self {
                thread,
                image_generation,
                run_generation,
            })
        }
    }

    pub const fn thread(self) -> u64 {
        self.thread
    }

    pub const fn image_generation(self) -> u64 {
        self.image_generation
    }

    pub const fn run_generation(self) -> u64 {
        self.run_generation
    }
}

/// Architecture-neutral classification of a contained EL0 fault.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UserFaultKind {
    InstructionAbort,
    DataAbort,
    Alignment,
    IllegalInstruction,
    SystemAccess,
    Breakpoint,
    OtherSynchronous,
}

/// Owned fault report copied out of an architecture-private exception frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UserFault {
    kind: UserFaultKind,
    syndrome: u64,
    address: u64,
    program_counter: u64,
}

impl UserFault {
    pub const fn new(
        kind: UserFaultKind,
        syndrome: u64,
        address: u64,
        program_counter: u64,
    ) -> Self {
        Self {
            kind,
            syndrome,
            address,
            program_counter,
        }
    }

    pub const fn kind(self) -> UserFaultKind {
        self.kind
    }

    pub const fn syndrome(self) -> u64 {
        self.syndrome
    }

    pub const fn address(self) -> u64 {
        self.address
    }

    pub const fn program_counter(self) -> u64 {
        self.program_counter
    }
}

/// Owner which retains every machine root currently installed by its active
/// local tokens, including across acknowledged immutable-root replacement.
///
/// # Safety
///
/// Implementors must not release a root or translation identifier while any
/// active token can still name it. Replacement may retire an old root only
/// after every active CPU has installed the successor and acknowledged.
pub unsafe trait UserTranslationOwner: Sync {}
