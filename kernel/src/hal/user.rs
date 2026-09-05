// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native-user entry and translation ownership contracts.

use core::marker::PhantomData;
use core::ptr::NonNull;

use crate::abi::native::{NativeInvocation, NativeResult};

/// Result of a Native syscall reached directly from synchronous exception entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeCallAction {
    /// Encode the completed result in the interrupted frame and return to EL0.
    Return(NativeResult),
    /// Stop the machine run and continue on the ordinary kernel Thread stack.
    Unwind,
}

/// Nonblocking policy callable while a Native user machine run is active.
///
/// # Safety
///
/// Implementations run with local interrupts masked, preemption disabled, and
/// the caller's user translation still installed. They must not block,
/// schedule, retain `invocation`, or invoke an operation that requires leaving
/// the current CPU. Returning [`NativeCallAction::Return`] asserts that all
/// work completed under those restrictions. A call requiring ordinary kernel
/// execution must return [`NativeCallAction::Unwind`] without side effects.
pub unsafe trait NativeCallHandler {
    /// Handles one invocation under the Native exception-entry restrictions.
    ///
    /// # Safety
    ///
    /// The caller must satisfy the interrupt, preemption, translation,
    /// CPU-affinity, and lifetime conditions documented on this trait.
    unsafe fn dispatch(&self, invocation: NativeInvocation) -> NativeCallAction;
}

/// Borrowed, type-erased Native exception service for one machine run.
///
/// The value is CPU-affine. Architecture entry may publish its address only
/// while the borrow and the current Thread's execution pin remain active.
pub struct NativeCallService<'handler> {
    handler: NonNull<()>,
    dispatch: unsafe fn(NonNull<()>, NativeInvocation) -> NativeCallAction,
    _lifetime: PhantomData<&'handler ()>,
    _not_send: PhantomData<*mut ()>,
}

impl<'handler> NativeCallService<'handler> {
    pub fn new<T: NativeCallHandler>(handler: &'handler T) -> Self {
        unsafe fn dispatch<T: NativeCallHandler>(
            handler: NonNull<()>,
            invocation: NativeInvocation,
        ) -> NativeCallAction {
            // SAFETY: `NativeCallService::new` stores the address of a live T,
            // and the service lifetime prevents use after that borrow ends.
            let handler = unsafe { handler.cast::<T>().as_ref() };
            // SAFETY: The erased entry point is reached only through
            // `NativeCallService::handle`, which forwards its caller contract.
            unsafe { handler.dispatch(invocation) }
        }

        Self {
            handler: NonNull::from(handler).cast(),
            dispatch: dispatch::<T>,
            _lifetime: PhantomData,
            _not_send: PhantomData,
        }
    }

    /// Invokes the borrowed handler for the active Native machine run.
    ///
    /// # Safety
    ///
    /// The caller must own the CPU-pinned machine run which published this
    /// service and satisfy [`NativeCallHandler::dispatch`]'s context contract.
    pub unsafe fn handle(&self, invocation: NativeInvocation) -> NativeCallAction {
        // SAFETY: Construction binds this function pointer and erased address
        // to the same handler type for the full service lifetime. The caller
        // supplies the execution-context preconditions forwarded above.
        unsafe { (self.dispatch)(self.handler, invocation) }
    }
}

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
