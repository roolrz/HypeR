// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Per-CPU publication of the pinned vCPU currently executing at lower EL.

use hyper::cpu::{CpuIndex, PerCpu};
use hyper::sync::InterruptSpinLock;

use super::VmInterruptController;
use crate::kernel::task::thread::VcpuExecution;

type ActiveLock = InterruptSpinLock<PerCpu<Option<ActiveBinding>>, crate::hal::irq::LocalMask>;

static ACTIVE: ActiveLock = InterruptSpinLock::new(PerCpu::new([None; hyper::cpu::MAX_CPUS]));

#[derive(Clone, Copy)]
struct ActiveBinding {
    execution: usize,
    borrowed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    ActiveVcpuMissing,
    CpuAlreadyActive,
    InterruptsEnabled,
    InvalidCpu,
    InvalidExecution,
    ReentrantAccess,
}

/// Publishes the pinned execution owned by the local guest run loop.
///
/// # Safety
///
/// `execution` must be the scheduler-origin raw pointer, not a pointer derived
/// from a temporary Rust reference. It must remain at this address and
/// exclusively associated with the calling CPU until [`clear`] succeeds.
/// Local IRQs must be masked across publication and guest entry.
pub unsafe fn set_raw(execution: *mut VcpuExecution) -> Result<(), Error> {
    if execution.is_null() || !execution.is_aligned() {
        return Err(Error::InvalidExecution);
    }
    ensure_interrupts_masked()?;
    let cpu = current_cpu()?;
    ACTIVE.with(|slots| {
        let slot = &mut slots[cpu];
        if slot.is_some() {
            return Err(Error::CpuAlreadyActive);
        }
        *slot = Some(ActiveBinding {
            execution: execution.expose_provenance(),
            borrowed: false,
        });
        Ok(())
    })
}

pub fn clear(execution: &mut VcpuExecution) -> Result<(), Error> {
    ensure_interrupts_masked()?;
    let cpu = current_cpu()?;
    ACTIVE.with(|slots| {
        let slot = &mut slots[cpu];
        let binding = slot.ok_or(Error::ActiveVcpuMissing)?;
        if binding.borrowed {
            return Err(Error::ReentrantAccess);
        }
        if binding.execution != core::ptr::from_mut(execution).expose_provenance() {
            return Err(Error::ActiveVcpuMissing);
        }
        *slot = None;
        Ok(())
    })
}

pub fn with<R>(
    operation: impl FnOnce(&mut VcpuExecution, &VmInterruptController) -> R,
) -> Result<Option<R>, Error> {
    ensure_interrupts_masked()?;
    let cpu = current_cpu()?;
    let binding = ACTIVE.with(|slots| {
        let binding = &mut slots[cpu];
        let Some(binding) = binding.as_mut() else {
            return Ok(None);
        };
        if binding.borrowed {
            return Err(Error::ReentrantAccess);
        }
        binding.borrowed = true;
        Ok(Some(*binding))
    })?;
    let Some(binding) = binding else {
        return Ok(None);
    };
    let borrow = ActiveBorrow {
        cpu,
        execution: binding.execution,
    };
    // SAFETY: set requires the execution to remain pinned and exclusively
    // associated with this CPU. Its VmBinding supplies the VM-owned interrupt
    // model. Exception entry keeps local IRQs masked and scopes both references
    // to this callback rather than a caller-selected lifetime.
    let result = unsafe {
        let execution =
            &mut *core::ptr::with_exposed_provenance_mut::<VcpuExecution>(binding.execution);
        let interrupts = core::ptr::from_ref(execution.interrupts());
        operation(execution, &*interrupts)
    };
    drop(borrow);
    Ok(Some(result))
}

struct ActiveBorrow {
    cpu: CpuIndex,
    execution: usize,
}

impl Drop for ActiveBorrow {
    fn drop(&mut self) {
        ACTIVE.with(|slots| {
            let Some(binding) = slots[self.cpu].as_mut() else {
                return;
            };
            if binding.execution == self.execution {
                binding.borrowed = false;
            }
        });
    }
}

fn ensure_interrupts_masked() -> Result<(), Error> {
    if crate::hal::irq::local_enabled() {
        Err(Error::InterruptsEnabled)
    } else {
        Ok(())
    }
}

fn current_cpu() -> Result<CpuIndex, Error> {
    crate::kernel::cpu::current_index().ok_or(Error::InvalidCpu)
}
