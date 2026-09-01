// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! `GICv3` virtualization machine-state ownership.

use hyper::hal::interrupt::HostInterruptBinding;
use hyper::sync::InterruptSpinLock;

type VgicLock = InterruptSpinLock<Option<State>, super::LocalInterruptMask>;

static VGIC: VgicLock = InterruptSpinLock::new(None);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    AlreadyInitialized,
    Architecture(super::VgicError),
    MissingHostTimerInterrupt,
}

impl From<super::VgicError> for Error {
    fn from(error: super::VgicError) -> Self {
        Self::Architecture(error)
    }
}

struct State {
    _host_timer_interrupt: HostInterruptBinding,
    capabilities: super::VgicCapabilities,
}

/// Validated capability retained privately until boot commits VM entry support.
pub struct Prepared {
    state: State,
}

impl Prepared {
    pub const fn list_registers(&self) -> u8 {
        self.state.capabilities.list_registers
    }
}

pub fn prepare(host_timer_interrupt: Option<HostInterruptBinding>) -> Result<Prepared, Error> {
    let host_timer_interrupt = host_timer_interrupt.ok_or(Error::MissingHostTimerInterrupt)?;
    let capabilities = super::validate_vgic()?;
    if VGIC.with(|slot| slot.is_some()) {
        return Err(Error::AlreadyInitialized);
    }
    Ok(Prepared {
        state: State {
            _host_timer_interrupt: host_timer_interrupt,
            capabilities,
        },
    })
}

pub fn commit(prepared: Prepared) -> Result<(), Error> {
    VGIC.with(|slot| {
        if slot.is_some() {
            return Err(Error::AlreadyInitialized);
        }
        *slot = Some(prepared.state);
        Ok(())
    })?;
    Ok(())
}

pub fn description() -> Option<(u8, u8, u8, u8)> {
    VGIC.with(|slot| {
        slot.as_ref().map(|state| {
            let capabilities = state.capabilities;
            (
                capabilities.list_registers,
                capabilities.priority_bits,
                capabilities.preemption_bits,
                capabilities.interrupt_id_bits,
            )
        })
    })
}
