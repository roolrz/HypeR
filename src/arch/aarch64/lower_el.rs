// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Generation-qualified ownership of the lower-EL return world.
//!
//! A lower-AArch64 vector identifies neither a guest nor a native Process.
//! Guest activation therefore publishes a non-repeating generation alongside
//! the pinned vCPU context identity. Native execution already publishes its
//! own generation in `user_entry`; exception entry combines both sources and
//! rejects missing or conflicting ownership before interpreting a raw frame.

use hyper::sync::atomic::{AtomicU64, Ordering};

use super::VcpuContext;

const MAX_CPUS: usize = hyper::config::MAX_CPUS as usize;
const EMPTY: u64 = 0;
const TRANSITION: u64 = u64::MAX;

struct GuestWorldSlot {
    /// A normal generation Release-publishes the preceding context identity.
    generation: AtomicU64,
    context: AtomicU64,
}

impl GuestWorldSlot {
    const fn empty() -> Self {
        Self {
            generation: AtomicU64::new(EMPTY),
            context: AtomicU64::new(0),
        }
    }
}

static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);
static GUEST_WORLDS: [GuestWorldSlot; MAX_CPUS] = [const { GuestWorldSlot::empty() }; MAX_CPUS];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    AlreadyPublished,
    GenerationExhausted,
    InvalidCpu,
    InvalidOwner,
    NativeActive,
    NotPublished,
    TransitionInProgress,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum World {
    Guest { generation: u64 },
    Native,
    None,
    Conflict,
    Corrupt,
}

/// Publishes the pinned guest context which owns the next lower-EL return.
///
/// The caller keeps local interrupts masked and retains the context at this
/// address until [`retire_guest`] succeeds. The address is used only as an
/// ownership identity; exception entry never reconstructs a Rust reference
/// from it.
pub(super) fn publish_guest(context: &mut VcpuContext) -> Result<(), Error> {
    if super::user_entry::active_on_current_cpu() {
        return Err(Error::NativeActive);
    }
    let cpu = current_cpu()?;
    let generation = next_generation()?;
    let slot = &GUEST_WORLDS[cpu];
    slot.generation
        .compare_exchange(EMPTY, TRANSITION, Ordering::Acquire, Ordering::Relaxed)
        .map_err(|_| Error::AlreadyPublished)?;
    slot.context.store(
        core::ptr::from_mut(context).expose_provenance() as u64,
        Ordering::Relaxed,
    );
    slot.generation.store(generation, Ordering::Release);
    Ok(())
}

/// Retires exactly the guest context published by [`publish_guest`].
pub(super) fn retire_guest(context: &mut VcpuContext) -> Result<(), Error> {
    let cpu = current_cpu()?;
    let slot = &GUEST_WORLDS[cpu];
    let generation = slot.generation.load(Ordering::Acquire);
    if generation == EMPTY {
        return Err(Error::NotPublished);
    }
    if generation == TRANSITION {
        return Err(Error::TransitionInProgress);
    }
    let expected = core::ptr::from_mut(context).expose_provenance() as u64;
    if slot.context.load(Ordering::Relaxed) != expected {
        return Err(Error::InvalidOwner);
    }
    slot.generation
        .compare_exchange(generation, TRANSITION, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| Error::InvalidOwner)?;
    slot.context.store(0, Ordering::Relaxed);
    slot.generation.store(EMPTY, Ordering::Release);
    Ok(())
}

/// Identifies the only world allowed to interpret a lower-AArch64 frame.
pub(super) fn current_world() -> World {
    let native = super::user_entry::active_on_current_cpu();
    let guest = guest_generation();
    match (native, guest) {
        (false, Ok(None)) => World::None,
        (true, Ok(None)) => World::Native,
        (false, Ok(Some(generation))) => World::Guest { generation },
        (true, Ok(Some(_))) => World::Conflict,
        (_, Err(_)) => World::Corrupt,
    }
}

fn guest_generation() -> Result<Option<u64>, Error> {
    let cpu = current_cpu()?;
    let slot = &GUEST_WORLDS[cpu];
    let generation = slot.generation.load(Ordering::Acquire);
    match generation {
        EMPTY => {
            if slot.context.load(Ordering::Relaxed) == 0 {
                Ok(None)
            } else {
                Err(Error::InvalidOwner)
            }
        }
        TRANSITION => Err(Error::TransitionInProgress),
        generation if slot.context.load(Ordering::Relaxed) != 0 => Ok(Some(generation)),
        _ => Err(Error::InvalidOwner),
    }
}

fn next_generation() -> Result<u64, Error> {
    let mut current = NEXT_GENERATION.load(Ordering::Relaxed);
    loop {
        if current == EMPTY || current == TRANSITION {
            return Err(Error::GenerationExhausted);
        }
        let next = current
            .checked_add(1)
            .filter(|next| *next != TRANSITION)
            .ok_or(Error::GenerationExhausted)?;
        match NEXT_GENERATION.compare_exchange_weak(
            current,
            next,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => return Ok(current),
            Err(observed) => current = observed,
        }
    }
}

fn current_cpu() -> Result<usize, Error> {
    let cpu = super::current_cpu_index();
    (cpu < MAX_CPUS).then_some(cpu).ok_or(Error::InvalidCpu)
}
