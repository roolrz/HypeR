//! Pinned lifetime ownership for installed virtual-machine runtime objects.

use alloc::boxed::Box;
use core::alloc::Layout;
use core::ptr::NonNull;

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
    let runtime = try_box(VmRuntime {
        virtual_machine,
        interrupts,
    })?;
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

fn try_box<T>(value: T) -> Result<Box<T>, Error> {
    let layout = Layout::new::<T>();
    // SAFETY: A successful allocation has the exact layout required by T.
    let pointer =
        NonNull::new(unsafe { alloc::alloc::alloc(layout) } as *mut T).ok_or(Error::Allocation)?;
    // SAFETY: pointer is aligned, writable, and uniquely owned for one T.
    unsafe {
        pointer.as_ptr().write(value);
        Ok(Box::from_raw(pointer.as_ptr()))
    }
}
