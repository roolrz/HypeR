#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuestMemoryAccess {
    Read,
    Write,
    Execute,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GuestTranslationFault {
    pub address: u64,
    pub access: GuestMemoryAccess,
    pub during_page_walk: bool,
}

pub struct GuestSyncFrame<'a> {
    fault: Option<GuestTranslationFault>,
    marker: core::marker::PhantomData<&'a mut ()>,
}

impl GuestSyncFrame<'_> {
    pub(crate) const fn translation(fault: GuestTranslationFault) -> Self {
        Self {
            fault: Some(fault),
            marker: core::marker::PhantomData,
        }
    }

    pub fn translation_fault(&self) -> Option<GuestTranslationFault> {
        self.fault
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)] // The common VM-exit contract is richer than each architecture backend.
pub enum GuestSyncAction {
    Resume,
    Injected,
    SoftwareInterrupt(u64),
    Unhandled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationError {
    VmxUnavailable,
    EptUnavailable,
}

pub fn validate() -> Result<(), ValidationError> {
    super::vmx::validate()
}

pub fn handle_guest_sync(
    _context: &mut super::VcpuContext,
    _vcpu_id: u32,
    _frame: &mut GuestSyncFrame<'_>,
) -> GuestSyncAction {
    GuestSyncAction::Unhandled
}

pub fn poll_virtual_timer(_now: u64) {}

pub fn take_timer_wakeup() -> Option<u64> {
    None
}
