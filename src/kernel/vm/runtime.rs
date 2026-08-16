//! Pinned lifetime ownership for installed virtual-machine runtime objects.

use alloc::boxed::Box;

use hyper::sync::InterruptSpinLock;

use super::VmInterruptController;
use crate::kernel::task::thread::VirtualMachineId;

type RuntimeLock = InterruptSpinLock<Option<Box<VmRuntime>>, crate::arch::LocalInterruptMask>;

static RUNTIME: RuntimeLock = InterruptSpinLock::new(None);

struct VmRuntime {
    virtual_machine: VirtualMachineId,
    interrupts: VmInterruptController,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Allocation,
    AlreadyInstalled,
    NotInstalled,
    WrongVirtualMachine,
}

/// Pins the shared objects referenced by active vCPUs for the VM lifetime.
pub fn install(
    virtual_machine: VirtualMachineId,
    interrupts: VmInterruptController,
) -> Result<(), Error> {
    let runtime = hyper::mm::try_box(VmRuntime {
        virtual_machine,
        interrupts,
    })
    .map_err(|_| Error::Allocation)?;
    RUNTIME.with(|slot| {
        if slot.is_some() {
            return Err(Error::AlreadyInstalled);
        }
        *slot = Some(runtime);
        Ok(())
    })
}

/// Returns the pinned interrupt model for one installed VM.
pub fn interrupts(
    virtual_machine: VirtualMachineId,
) -> Result<&'static VmInterruptController, Error> {
    let pointer = RUNTIME.with(|slot| {
        let runtime = slot.as_ref().ok_or(Error::NotInstalled)?;
        if runtime.virtual_machine != virtual_machine {
            return Err(Error::WrongVirtualMachine);
        }
        Ok::<*const VmInterruptController, Error>(&runtime.interrupts)
    })?;
    // SAFETY: Installed runtimes are boxed, never replaced or removed, and
    // therefore remain pinned for every active vCPU reference.
    Ok(unsafe { &*pointer })
}
