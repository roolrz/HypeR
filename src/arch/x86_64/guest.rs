//! Architecture-facing representation of synchronous x86 guest exits.
//!
//! VMX and SVM own their machine state and place only owned, decoded facts in
//! this adapter. The lifetime parameter preserves the common architecture
//! facade shape; this backend does not borrow a VMCS or VMCB through it.

use hyper::vm::exit::GuestMemoryFault;

pub(crate) struct GuestSyncFrame<'a> {
    fault: Option<GuestMemoryFault>,
    marker: core::marker::PhantomData<&'a mut ()>,
}

impl GuestSyncFrame<'_> {
    pub(crate) const fn memory_fault(fault: GuestMemoryFault) -> Self {
        Self {
            fault: Some(fault),
            marker: core::marker::PhantomData,
        }
    }

    pub(crate) fn guest_memory_fault(&self) -> Option<GuestMemoryFault> {
        self.fault
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)] // The common VM-exit contract is richer than each architecture backend.
pub(crate) enum GuestSyncAction {
    Resume,
    Injected,
    SoftwareInterrupt(u64),
    Unhandled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationError {
    HardwareUnavailable,
    SecondLevelPagingUnavailable,
    MissingNextRip,
    BackendConflict,
}

pub(super) fn validate() -> Result<(), ValidationError> {
    super::virtualization::validate().map(|_| ())
}

pub(crate) fn handle_guest_sync(
    _context: &mut super::VcpuContext,
    _vcpu_id: u32,
    _frame: &mut GuestSyncFrame<'_>,
) -> GuestSyncAction {
    GuestSyncAction::Unhandled
}

pub(super) fn poll_virtual_timer(_now: u64) {}

pub(super) fn take_timer_wakeup() -> Option<u64> {
    None
}
