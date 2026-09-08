// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Scheduler-owned execution state for one virtual CPU.
//!
//! This payload is VM policy, not generic Thread state. The scheduler owns its
//! allocation and stable address, while this module owns every interpretation
//! of the guest binding, hardware registers, and terminal publication.

use crate::kernel::task::thread::ThreadId;

pub(crate) struct VcpuExecution {
    vm: VcpuVm,
    instruction_context: hyper::vm::translation::GuestInstructionContext,
    terminal_mmio_report: Option<crate::kernel::vm::UnhandledMmioReport>,
    reap: ReapOwnership,
    pub(crate) vcpu_id: u32,
    pub(crate) hardware: crate::hal::vm::VcpuHardwareState,
}

// Migration eligibility remains compiler-proven. CPU-affine hardware and
// residency claims live in `vm::active_vcpu`, never in this movable payload.
const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<VcpuExecution>();
};

enum VcpuVm {
    Installed(crate::kernel::vm::registry::VmBinding),
    TimerValidation { interrupts: usize },
}

enum ReapOwnership {
    Installed(alloc::boxed::Box<VcpuReapPublication>),
    None,
}

impl VcpuExecution {
    pub(in crate::kernel) const fn allocation_size() -> Option<usize> {
        Some(core::mem::size_of::<Self>())
    }

    pub(in crate::kernel) const fn retirement_allocation_size() -> usize {
        core::mem::size_of::<VcpuReapPublication>()
    }

    pub(in crate::kernel) fn installed(
        vm: crate::kernel::vm::registry::VmBinding,
        vcpu_id: u32,
        context: crate::hal::vm::VcpuContext,
        entry_ready: &crate::hal::vm::VmEntryReady,
    ) -> Result<Self, crate::kernel::task::thread::Error> {
        let mut hardware = crate::hal::vm::VcpuHardwareState::new(context, entry_ready);
        crate::hal::vm::initialize_vcpu_interrupts(&mut hardware)?;
        let endpoint = vm
            .endpoint_owner(vcpu_id)
            .map_err(|_| crate::kernel::task::thread::Error::InvalidPlacement)?;
        let reap = hyper::mm::try_box(VcpuReapPublication::new(endpoint, vcpu_id))
            .map_err(|_| crate::kernel::task::thread::Error::Allocation)?;
        Ok(Self {
            vm: VcpuVm::Installed(vm),
            instruction_context: hyper::vm::translation::GuestInstructionContext::new(),
            terminal_mmio_report: None,
            reap: ReapOwnership::Installed(reap),
            vcpu_id,
            hardware,
        })
    }

    pub(in crate::kernel) fn vm_binding(&self) -> Option<&crate::kernel::vm::registry::VmBinding> {
        match &self.vm {
            VcpuVm::Installed(binding) => Some(binding),
            VcpuVm::TimerValidation { .. } => None,
        }
    }

    pub(super) fn enter_instruction_context(
        &mut self,
        cpu: hyper::cpu::CpuIndex,
    ) -> hyper::vm::translation::GuestInstructionContextTransition {
        self.instruction_context.enter(cpu)
    }

    #[allow(dead_code)]
    pub(in crate::kernel) fn device_context(
        &mut self,
    ) -> Option<(
        &crate::kernel::vm::registry::VmBinding,
        &mut crate::hal::vm::VcpuHardwareState,
        u32,
        &crate::kernel::vm::VmInterruptController,
    )> {
        match &self.vm {
            VcpuVm::Installed(binding) => Some((
                binding,
                &mut self.hardware,
                self.vcpu_id,
                binding.interrupts(),
            )),
            VcpuVm::TimerValidation { .. } => None,
        }
    }

    /// Splits the mutable hardware state from its immutable interrupt model.
    ///
    /// Keeping this split inside the owning type lets Rust prove that callers
    /// never receive an immutable reference into a value which they may also
    /// replace wholesale through `&mut VcpuExecution`.
    pub(in crate::kernel) fn interrupt_context(
        &mut self,
    ) -> (
        &mut crate::hal::vm::VcpuHardwareState,
        u32,
        &crate::kernel::vm::VmInterruptController,
    ) {
        let interrupts = match &self.vm {
            VcpuVm::Installed(binding) => binding.interrupts(),
            VcpuVm::TimerValidation { interrupts } => {
                // SAFETY: `for_timer_validation` requires the pointed-to model
                // to remain fixed and live through execution deactivation.
                unsafe {
                    &*core::ptr::with_exposed_provenance::<crate::kernel::vm::VmInterruptController>(
                        *interrupts,
                    )
                }
            }
        };
        (&mut self.hardware, self.vcpu_id, interrupts)
    }

    pub(crate) fn interrupts(&self) -> &crate::kernel::vm::VmInterruptController {
        match &self.vm {
            VcpuVm::Installed(binding) => binding.interrupts(),
            VcpuVm::TimerValidation { interrupts } => {
                // SAFETY: `for_timer_validation` requires the pointed-to model
                // to remain fixed and live through execution deactivation.
                unsafe {
                    &*core::ptr::with_exposed_provenance::<crate::kernel::vm::VmInterruptController>(
                        *interrupts,
                    )
                }
            }
        }
    }

    #[allow(dead_code)]
    pub(in crate::kernel) fn publish_terminal_mmio_report(
        &mut self,
        report: crate::kernel::vm::UnhandledMmioReport,
    ) -> Result<(), ()> {
        if self.terminal_mmio_report.is_some() {
            return Err(());
        }
        self.terminal_mmio_report = Some(report);
        Ok(())
    }

    pub(in crate::kernel) const fn terminal_mmio_report_pending(&self) -> bool {
        self.terminal_mmio_report.is_some()
    }

    pub(in crate::kernel) fn take_terminal_mmio_report(
        &mut self,
    ) -> Option<crate::kernel::vm::UnhandledMmioReport> {
        self.terminal_mmio_report.take()
    }

    pub(super) fn arm_reap_publication(
        &mut self,
        thread: ThreadId,
        reason: crate::kernel::vm::endpoint_state::ClosureReason,
    ) -> Result<(), ()> {
        match &mut self.reap {
            ReapOwnership::Installed(publication) => publication.arm(thread, reason),
            ReapOwnership::None => Err(()),
        }
    }

    pub(in crate::kernel) fn take_reap_publication(
        &mut self,
    ) -> Option<crate::kernel::task::thread::ThreadRetirement> {
        let ReapOwnership::Installed(publication) =
            core::mem::replace(&mut self.reap, ReapOwnership::None)
        else {
            return None;
        };
        if !publication.is_armed() {
            self.reap = ReapOwnership::Installed(publication);
            return None;
        }
        Some(crate::kernel::task::thread::ThreadRetirement::from_box(
            publication,
        ))
    }

    /// Builds the non-runnable execution used by architecture timer checks.
    ///
    /// # Safety
    ///
    /// `interrupts` must remain fixed and live until this execution is
    /// deactivated and dropped.
    #[allow(dead_code)]
    pub(crate) unsafe fn for_timer_validation(
        hardware: crate::hal::vm::VcpuHardwareState,
        interrupts: &crate::kernel::vm::VmInterruptController,
    ) -> Self {
        Self {
            vm: VcpuVm::TimerValidation {
                interrupts: core::ptr::from_ref(interrupts).expose_provenance(),
            },
            instruction_context: hyper::vm::translation::GuestInstructionContext::new(),
            terminal_mmio_report: None,
            reap: ReapOwnership::None,
            vcpu_id: 0,
            hardware,
        }
    }
}

impl crate::kernel::task::thread::ExternalThreadExecutionLifecycle for VcpuExecution {
    fn take_thread_retirement(&mut self) -> Option<crate::kernel::task::thread::ThreadRetirement> {
        self.take_reap_publication()
    }
}

/// Exact terminal publication extracted only after scheduler detachment.
#[must_use = "the canonical vCPU terminal transition must be published"]
pub(in crate::kernel) struct VcpuReapPublication {
    endpoint: hyper::mm::FallibleArc<crate::kernel::vm::endpoint::VcpuEndpoint>,
    vcpu: u32,
    terminal: Option<(ThreadId, crate::kernel::vm::endpoint_state::ClosureReason)>,
}

impl VcpuReapPublication {
    const fn new(
        endpoint: hyper::mm::FallibleArc<crate::kernel::vm::endpoint::VcpuEndpoint>,
        vcpu: u32,
    ) -> Self {
        Self {
            endpoint,
            vcpu,
            terminal: None,
        }
    }

    fn arm(
        &mut self,
        thread: ThreadId,
        reason: crate::kernel::vm::endpoint_state::ClosureReason,
    ) -> Result<(), ()> {
        if self.terminal.replace((thread, reason)).is_some() {
            return Err(());
        }
        Ok(())
    }

    const fn is_armed(&self) -> bool {
        self.terminal.is_some()
    }

    /// Completes VM-owned terminal publication after the Thread is destroyed.
    fn publish(self) -> Result<(), crate::kernel::task::thread::ThreadRetirementError> {
        if !self.endpoint.is_valid_for(self.vcpu) {
            return Err(crate::kernel::task::thread::ThreadRetirementError::PublicationRejected);
        }
        let Some((thread, reason)) = self.terminal else {
            return Err(crate::kernel::task::thread::ThreadRetirementError::PublicationRejected);
        };
        self.endpoint
            .publish_reaped(thread, reason)
            .map_err(|_| crate::kernel::task::thread::ThreadRetirementError::PublicationRejected)
    }
}

impl crate::kernel::task::thread::ThreadRetirementAction for VcpuReapPublication {
    fn complete(
        self: alloc::boxed::Box<Self>,
    ) -> Result<(), crate::kernel::task::thread::ThreadRetirementError> {
        self.publish()
    }
}
