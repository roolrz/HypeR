// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Installed VM aggregate, strong bindings, and execution claims.

use hyper::cpu::CpuIndex;
use hyper::mm::FallibleArc;
use hyper::sync::InterruptSpinLock;
use hyper::vm::translation::{ExclusiveExecution, ExecutionClaim, ExecutionError};

use super::{Error, VmId};
use crate::kernel::accounting::CommittedCharge;
use crate::kernel::task::thread::ThreadId;
use crate::kernel::vm::VmInterruptController;
use crate::kernel::vm::device::VirtualDeviceSet;
use crate::kernel::vm::memory::GuestAddressSpace;

type AddressSpaceLock = InterruptSpinLock<GuestAddressSpace, crate::hal::irq::LocalMask>;

/// Non-cloneable vCPU capability retaining strong ownership of its VM.
pub(in crate::kernel) struct VmBinding {
    id: VmId,
    machine: FallibleArc<VirtualMachine>,
}

impl VmBinding {
    pub(super) fn new(id: VmId, machine: FallibleArc<VirtualMachine>) -> Self {
        Self { id, machine }
    }

    pub(crate) const fn id(&self) -> VmId {
        self.id
    }

    /// Admits at most four detailed terminal-MMIO reports and one final
    /// suppression notice over this installed VM's complete lifetime.
    #[allow(dead_code)]
    pub(in crate::kernel::vm) fn admit_unhandled_mmio(
        &self,
        vcpu: u32,
        access: hyper::vm::exit::MmioAccess,
    ) -> Option<crate::kernel::vm::diagnostics::UnhandledMmioReport> {
        self.machine
            .diagnostics
            .admit_unhandled_mmio(self.id.diagnostic_id(), vcpu, access)
    }

    pub(crate) fn interrupts(&self) -> &VmInterruptController {
        &self.machine.interrupts
    }

    fn endpoint(&self, vcpu: u32) -> Result<&crate::kernel::vm::endpoint::VcpuEndpoint, Error> {
        self.machine.endpoint(vcpu)
    }

    pub(in crate::kernel::vm) fn endpoint_owner(
        &self,
        vcpu: u32,
    ) -> Result<FallibleArc<crate::kernel::vm::endpoint::VcpuEndpoint>, Error> {
        self.machine
            .lifecycle
            .endpoint(vcpu)
            .cloned()
            .map_err(|_| Error::UnknownVcpu)
    }

    pub(in crate::kernel) fn take_interrupt_reconcile(&self, vcpu: u32) -> Result<bool, Error> {
        self.endpoint(vcpu)
            .map(|endpoint| endpoint.take_reconcile())
    }

    pub(in crate::kernel) fn restore_interrupt_reconcile(&self, vcpu: u32) -> Result<(), Error> {
        self.endpoint(vcpu)?.restore_reconcile();
        Ok(())
    }

    #[allow(dead_code)]
    pub(in crate::kernel) fn interrupt_reconcile_pending(&self, vcpu: u32) -> Result<bool, Error> {
        self.endpoint(vcpu)
            .map(crate::kernel::vm::endpoint::VcpuEndpoint::reconcile_pending)
    }

    /// Publishes a completed saved interrupt-model mutation, then prompts the
    /// scheduler-authoritative running CPU, if any.
    #[allow(dead_code)]
    pub(in crate::kernel::vm) fn publish_interrupt_reconcile(
        &self,
        vcpu: u32,
        expected_thread: ThreadId,
    ) -> Result<(), Error> {
        let endpoint = self.endpoint(vcpu)?;
        if endpoint.thread() != Some(expected_thread) {
            return Err(Error::StaleIdentity);
        }
        match endpoint.publish_reconcile() {
            Ok(()) => {}
            Err(crate::kernel::vm::endpoint_state::StateError::Closed(_)) => {
                return Err(Error::EndpointClosed);
            }
            Err(crate::kernel::vm::endpoint_state::StateError::Corrupt) => {
                crate::kernel::crash::fatal(format_args!(
                    "HypeR: vCPU endpoint contains an invalid lifecycle state"
                ));
            }
        }
        endpoint.signal_waiter();
        let Some(cpu) = crate::kernel::task::scheduler::running_vcpu_cpu(expected_thread)
            .map_err(|_| Error::Scheduler)?
        else {
            return Ok(());
        };
        if crate::hal::vm::request_guest_exit(cpu) {
            return Ok(());
        }
        // A failed hardware route may be stale because migration happened
        // after the first immutable scheduler observation. Retry one changed
        // exact target; a still-Running unpromptable Thread cannot safely be
        // allowed to continue with an unreflected interrupt model.
        let target = crate::kernel::task::scheduler::running_vcpu_cpu(expected_thread)
            .map_err(|_| Error::Scheduler)?;
        match target {
            None => {}
            Some(target) if target != cpu && crate::hal::vm::request_guest_exit(target) => {}
            Some(_) => crate::kernel::crash::fatal(format_args!(
                "HypeR: scheduler reports a running vCPU without a qualified guest-exit route"
            )),
        }
        Ok(())
    }

    pub(in crate::kernel::vm) fn wfi_wait_ticket(
        &self,
        vcpu: u32,
    ) -> Result<crate::kernel::vm::endpoint::WaitTicket, Error> {
        Ok(self.endpoint(vcpu)?.wait_ticket())
    }

    pub(in crate::kernel::vm) fn prepare_wfi_wait(
        &self,
        vcpu: u32,
        ticket: crate::kernel::vm::endpoint::WaitTicket,
    ) -> Result<crate::kernel::vm::endpoint::PreparedWait<'_>, Error> {
        self.endpoint(vcpu)?
            .prepare_wait(ticket)
            .map_err(|_| Error::Scheduler)
    }

    pub(in crate::kernel::vm) fn arm_wfi_timer(
        &self,
        vcpu: u32,
        deadline: u64,
    ) -> Result<crate::kernel::time::ArmedReservedTimer<'_>, Error> {
        self.endpoint(vcpu)?
            .arm_timer(deadline)
            .map_err(|_| Error::Scheduler)
    }

    #[allow(dead_code)]
    pub(in crate::kernel::vm) fn close_vcpu_endpoint(
        &self,
        vcpu: u32,
        expected_thread: ThreadId,
        reason: crate::hal::vm::VcpuTerminalReason,
    ) -> Result<crate::kernel::vm::endpoint_state::GuestCloseOutcome, Error> {
        let reason = match reason {
            crate::hal::vm::VcpuTerminalReason::MemoryFault => {
                crate::kernel::vm::endpoint_state::TerminalReason::MemoryFault
            }
            crate::hal::vm::VcpuTerminalReason::Mmio => {
                crate::kernel::vm::endpoint_state::TerminalReason::Mmio
            }
            crate::hal::vm::VcpuTerminalReason::Synchronous => {
                crate::kernel::vm::endpoint_state::TerminalReason::Synchronous
            }
        };
        self.endpoint(vcpu)?
            .close(expected_thread, reason)
            .map_err(|_| Error::StaleIdentity)
    }

    #[allow(dead_code)]
    pub(in crate::kernel::vm) fn administrative_stop_requested(
        &self,
        vcpu: u32,
        expected_thread: ThreadId,
    ) -> Result<Option<crate::kernel::vm::endpoint_state::AdministrativeStopReason>, Error> {
        self.endpoint(vcpu)?
            .stop_requested(expected_thread)
            .map_err(|_| Error::StaleIdentity)
    }

    #[allow(dead_code)]
    pub(in crate::kernel::vm) fn publish_hardware_detached(
        &self,
        vcpu: u32,
        expected_thread: ThreadId,
        reason: crate::kernel::vm::endpoint_state::AdministrativeStopReason,
    ) -> Result<(), Error> {
        self.endpoint(vcpu)?
            .publish_hardware_detached(expected_thread, reason)
            .map_err(|_| Error::StaleIdentity)
    }

    // Some selected guest platforms currently expose no emulated-device exit
    // path. Retain one stable aggregate accessor so VM ownership and layout do
    // not vary with the host architecture.
    #[allow(dead_code)]
    pub(in crate::kernel::vm) fn devices(&self) -> &VirtualDeviceSet {
        &self.machine.devices
    }

    pub(in crate::kernel::vm) fn with_address_space<R>(
        &self,
        operation: impl FnOnce(&mut GuestAddressSpace) -> R,
    ) -> R {
        self.machine.address_space.with(operation)
    }

    /// Claims this VM's currently single active execution interval.
    ///
    /// The capability is intentionally independent of vCPU identity: current
    /// construction installs one boot vCPU, and the invariant remains safe if
    /// additional vCPU objects are added before VM-wide shootdown support.
    pub(in crate::kernel) fn claim_execution(
        &self,
        cpu: CpuIndex,
    ) -> Result<VmExecutionClaim, VmExecutionError> {
        let admission = self
            .machine
            .run_admission
            .admit()
            .map_err(|error| match error {
                crate::kernel::vm::run_admission::AdmissionError::Closed => {
                    VmExecutionError::AdmissionClosed
                }
                crate::kernel::vm::run_admission::AdmissionError::CountExhausted => {
                    VmExecutionError::AdmissionCountExhausted
                }
            })?;
        match self.machine.execution.claim(cpu) {
            Ok(execution) => Ok(VmExecutionClaim {
                execution,
                admission,
                residency: None,
            }),
            Err(error) => {
                self.machine.run_admission.release(admission);
                Err(VmExecutionError::Execution(error))
            }
        }
    }

    pub(in crate::kernel) fn release_execution(
        &self,
        claim: VmExecutionClaim,
        current_cpu: CpuIndex,
    ) -> Result<(), VmExecutionReleaseFailure> {
        if claim.residency.is_some() {
            // Architecture detach must consume guest residency before the
            // execution/admission capability can cross this release boundary.
            crate::hal::cpu::halt()
        }
        let VmExecutionClaim {
            execution,
            admission,
            residency: _,
        } = claim;
        match self.machine.execution.release(execution, current_cpu) {
            Ok(()) => {
                // Admission release cannot fail through this private API. It
                // follows successful execution release, so every returned
                // failure below retains both exact armed capabilities.
                self.machine.run_admission.release(admission);
                Ok(())
            }
            Err(failure) => Err(VmExecutionReleaseFailure {
                error: failure.error(),
                claim: VmExecutionClaim {
                    execution: failure.into_claim(),
                    admission,
                    residency: None,
                },
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VmExecutionError {
    AdmissionClosed,
    AdmissionCountExhausted,
    Execution(ExecutionError),
}

#[must_use = "VM execution ownership must remain live until release succeeds"]
pub(crate) struct VmExecutionClaim {
    execution: ExecutionClaim,
    admission: crate::kernel::vm::run_admission::RunAdmissionClaim,
    residency: Option<crate::kernel::vm::memory::GuestResidencyClaim>,
}

impl VmExecutionClaim {
    pub(in crate::kernel) fn attach_residency(
        &mut self,
        residency: crate::kernel::vm::memory::GuestResidencyClaim,
    ) -> Result<(), crate::kernel::vm::memory::GuestResidencyClaim> {
        if self.residency.is_some() {
            return Err(residency);
        }
        self.residency = Some(residency);
        Ok(())
    }

    pub(in crate::kernel) fn take_residency(
        &mut self,
    ) -> Option<crate::kernel::vm::memory::GuestResidencyClaim> {
        self.residency.take()
    }

    pub(in crate::kernel) fn restore_residency(
        &mut self,
        residency: crate::kernel::vm::memory::GuestResidencyClaim,
    ) {
        if self.residency.replace(residency).is_some() {
            crate::hal::cpu::halt()
        }
    }
}

#[must_use = "a failed VM execution release retains both exact claims"]
pub(crate) struct VmExecutionReleaseFailure {
    error: ExecutionError,
    claim: VmExecutionClaim,
}

impl VmExecutionReleaseFailure {
    pub(crate) const fn error(&self) -> ExecutionError {
        self.error
    }

    pub(crate) fn into_claim(self) -> VmExecutionClaim {
        self.claim
    }
}

pub(super) struct VirtualMachine {
    id: VmId,
    address_space: AddressSpaceLock,
    execution: ExclusiveExecution,
    run_admission: crate::kernel::vm::run_admission::RunAdmission,
    interrupts: VmInterruptController,
    // RISC-V's current selected set is zero-sized and has no exit consumer,
    // but the VM still owns it through the same lifecycle as other targets.
    #[allow(dead_code)]
    devices: VirtualDeviceSet,
    diagnostics: crate::kernel::vm::diagnostics::VmDiagnostics,
    lifecycle: FallibleArc<crate::kernel::vm::installed::InstalledMachine>,
    _interrupt_controller_charge: Option<CommittedCharge>,
}

impl VirtualMachine {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn try_new(
        id: VmId,
        address_space: GuestAddressSpace,
        interrupts: VmInterruptController,
        devices: VirtualDeviceSet,
        lifecycle: FallibleArc<crate::kernel::vm::installed::InstalledMachine>,
        interrupt_controller_charge: Option<CommittedCharge>,
    ) -> Result<FallibleArc<Self>, Error> {
        FallibleArc::try_new(Self {
            id,
            address_space: InterruptSpinLock::new(address_space),
            execution: ExclusiveExecution::new(id.execution_owner()),
            run_admission: crate::kernel::vm::run_admission::RunAdmission::new(
                id.execution_owner(),
            ),
            interrupts,
            devices,
            diagnostics: crate::kernel::vm::diagnostics::VmDiagnostics::new(),
            lifecycle,
            _interrupt_controller_charge: interrupt_controller_charge,
        })
        .map_err(|_| Error::Allocation)
    }

    pub(super) const fn id(&self) -> VmId {
        self.id
    }

    pub(super) fn allocation_size() -> usize {
        FallibleArc::<Self>::allocation_size()
    }

    pub(super) fn lifecycle(&self) -> FallibleArc<crate::kernel::vm::installed::InstalledMachine> {
        self.lifecycle.clone()
    }

    pub(super) fn activate_identifier_for_install(&self) -> Result<(), Error> {
        self.address_space
            .with(GuestAddressSpace::activate_identifier_for_install)
    }

    pub(super) fn with_address_space<R>(
        &self,
        operation: impl FnOnce(&mut GuestAddressSpace) -> R,
    ) -> R {
        self.address_space.with(operation)
    }

    pub(super) fn endpoint(
        &self,
        vcpu: u32,
    ) -> Result<&crate::kernel::vm::endpoint::VcpuEndpoint, Error> {
        self.lifecycle
            .endpoint(vcpu)
            .map(|endpoint| &**endpoint)
            .map_err(|_| Error::UnknownVcpu)
    }

    pub(super) fn request_all_stops(&self) -> Result<(), Error> {
        for endpoint in self.lifecycle.endpoints() {
            let Some(thread) = endpoint.thread() else {
                if endpoint.lifecycle().map_err(|_| Error::StaleIdentity)?
                    != crate::kernel::vm::endpoint_state::Lifecycle::Unbound
                {
                    return Err(Error::StaleIdentity);
                }
                continue;
            };
            match endpoint
                .request_stop(
                    thread,
                    crate::kernel::vm::endpoint_state::AdministrativeStopReason::Requested,
                )
                .map_err(|_| Error::StaleIdentity)?
            {
                crate::kernel::vm::endpoint_state::StopRequestOutcome::Published
                | crate::kernel::vm::endpoint_state::StopRequestOutcome::AlreadyRequested => {
                    match crate::kernel::task::scheduler::request_vcpu_stop(thread) {
                        Ok(()) => {}
                        Err(crate::kernel::task::scheduler::Error::ThreadNotFound) => {
                            if !endpoint
                                .thread_absence_is_terminal()
                                .map_err(|_| Error::StaleIdentity)?
                            {
                                return Err(Error::Scheduler);
                            }
                        }
                        Err(_) => return Err(Error::Scheduler),
                    }
                }
                crate::kernel::vm::endpoint_state::StopRequestOutcome::GuestTerminal(_)
                | crate::kernel::vm::endpoint_state::StopRequestOutcome::HardwareDetached
                | crate::kernel::vm::endpoint_state::StopRequestOutcome::Reaped
                | crate::kernel::vm::endpoint_state::StopRequestOutcome::Inactive => {}
            }
        }
        self.run_admission.close();
        Ok(())
    }

    pub(super) fn bind_virtual_serial(&self, vcpu: u32, thread: ThreadId) {
        self.devices.bind_virtual_serial(self.id, vcpu, thread);
    }

    pub(super) fn disconnect_virtual_serial(&self) {
        self.devices.disconnect_virtual_serial(self.id);
    }

    pub(super) fn quiesce_devices(&self) -> Result<(), super::super::device::Error> {
        super::super::device::quiesce(&self.devices)
    }

    pub(super) fn is_quiescent(&self) -> bool {
        self.run_admission.is_closed_and_quiescent()
            && self.lifecycle.endpoints().iter().all(|endpoint| {
                matches!(
                    endpoint.lifecycle(),
                    Ok(crate::kernel::vm::endpoint_state::Lifecycle::Unbound
                        | crate::kernel::vm::endpoint_state::Lifecycle::Reaped(_))
                )
            })
    }
}
