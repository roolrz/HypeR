//! Per-CPU publication of the pinned vCPU currently executing at lower EL.

use hyper::sync::InterruptSpinLock;

use super::VmInterruptController;
use crate::kernel::task::thread::VcpuExecution;

const MAX_CPUS: usize = hyper::config::MAX_CPUS as usize;

type ActiveLock =
    InterruptSpinLock<[Option<ActiveBinding>; MAX_CPUS], crate::arch::LocalInterruptMask>;

static ACTIVE: ActiveLock = InterruptSpinLock::new([None; MAX_CPUS]);

#[derive(Clone, Copy)]
struct ActiveBinding {
    execution: usize,
    interrupts: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    ActiveVcpuMissing,
    CpuAlreadyActive,
    InvalidCpu,
}

/// Publishes the pinned objects owned by the local guest run loop.
///
/// # Safety
///
/// Both objects must remain at these addresses and exclusively associated
/// with the calling CPU until [`clear`] succeeds. Local IRQs must be masked
/// across publication and guest entry.
pub unsafe fn set(
    execution: &mut VcpuExecution,
    interrupts: &VmInterruptController,
) -> Result<(), Error> {
    let cpu = current_cpu()?;
    ACTIVE.with(|slots| {
        let slot = slots.get_mut(cpu).ok_or(Error::InvalidCpu)?;
        if slot.is_some() {
            return Err(Error::CpuAlreadyActive);
        }
        *slot = Some(ActiveBinding {
            execution: execution as *mut VcpuExecution as usize,
            interrupts: interrupts as *const VmInterruptController as usize,
        });
        Ok(())
    })
}

pub fn clear(execution: &mut VcpuExecution) -> Result<(), Error> {
    let cpu = current_cpu()?;
    ACTIVE.with(|slots| {
        let slot = slots.get_mut(cpu).ok_or(Error::InvalidCpu)?;
        let binding = slot.ok_or(Error::ActiveVcpuMissing)?;
        if binding.execution != execution as *mut VcpuExecution as usize {
            return Err(Error::ActiveVcpuMissing);
        }
        *slot = None;
        Ok(())
    })
}

pub fn with<R>(
    operation: impl FnOnce(&mut VcpuExecution, &VmInterruptController) -> R,
) -> Result<Option<R>, Error> {
    let cpu = current_cpu()?;
    let binding = ACTIVE.with(|slots| slots.get(cpu).copied().ok_or(Error::InvalidCpu))?;
    let Some(binding) = binding else {
        return Ok(None);
    };
    // SAFETY: set requires both objects to remain pinned and exclusively
    // associated with this CPU. Exception entry keeps local IRQs masked.
    let (execution, interrupts) = unsafe { binding.objects() };
    Ok(Some(operation(execution, interrupts)))
}

fn current_cpu() -> Result<usize, Error> {
    let cpu = crate::arch::current_cpu_index();
    if cpu < MAX_CPUS {
        Ok(cpu)
    } else {
        Err(Error::InvalidCpu)
    }
}

impl ActiveBinding {
    unsafe fn objects<'a>(self) -> (&'a mut VcpuExecution, &'a VmInterruptController) {
        // SAFETY: Enforced by set's contract and clear pairing.
        unsafe {
            (
                &mut *(self.execution as *mut VcpuExecution),
                &*(self.interrupts as *const VmInterruptController),
            )
        }
    }
}
