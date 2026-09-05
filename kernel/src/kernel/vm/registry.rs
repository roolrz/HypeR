// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Installed virtual-machine identity and aggregate ownership.
//!
//! The registry is the single publication point for guest address spaces and
//! interrupt models. Construction happens outside the registry lock through a
//! non-cloneable reservation; dropping an unpublished reservation rolls its
//! slot back. Strong, scoped leases keep an installed aggregate alive after
//! the registry lock is released.
//!
//! The registry lock protects slot identity only. It is released before an
//! address-space operation and must never be nested with a VM-internal lock.
//!
//! Retirement is deliberately staged. This module can cut normal lookup,
//! stop and reap every published vCPU, drain admitted runs and scoped leases,
//! retain the allocation through acknowledged architecture retirement. Slot
//! generation advances only after the retired aggregate is destroyed outside
//! registry, RPC, and address-space locks.

use alloc::vec::Vec;

use hyper::cpu::CpuIndex;
use hyper::mm::{FallibleArc, UniqueFallibleArc};
use hyper::sync::InterruptSpinLock;
use hyper::vm::translation::{ExclusiveExecution, ExecutionClaim, ExecutionError};

use super::VmInterruptController;
use super::device::VirtualDeviceSet;
use super::memory::{GuestAddressSpace, Stage2IdentifierReservation};
use crate::kernel::task::thread::ThreadId;

type RegistryLock = InterruptSpinLock<VmRegistry, crate::hal::irq::LocalMask>;
type AddressSpaceLock = InterruptSpinLock<GuestAddressSpace, crate::hal::irq::LocalMask>;

static REGISTRY: RegistryLock = InterruptSpinLock::new(VmRegistry::new());

/// Registry metadata is fixed-capacity so reservation never allocates while
/// holding the global identity lock.
const MAX_VIRTUAL_MACHINES: usize = 64;

/// Logical identity issued by the VM registry.
///
/// The slot and generation are deliberately private. Callers may retain and
/// compare an identity, but cannot manufacture one from a hardware VMID or a
/// registry index.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct VmId {
    slot: u32,
    generation: u32,
}

impl VmId {
    const fn execution_owner(self) -> u64 {
        ((self.generation as u64) << 32) | self.slot as u64
    }

    #[allow(dead_code)]
    const fn diagnostic_id(self) -> super::diagnostics::VmDiagnosticId {
        super::diagnostics::VmDiagnosticId::new(self.slot, self.generation)
    }
}

/// Non-cloneable vCPU capability retaining strong ownership of its VM.
pub(in crate::kernel) struct VmBinding {
    id: VmId,
    machine: FallibleArc<VirtualMachine>,
}

impl VmBinding {
    pub(crate) const fn id(&self) -> VmId {
        self.id
    }

    /// Admits at most four detailed terminal-MMIO reports and one final
    /// suppression notice over this installed VM's complete lifetime.
    #[allow(dead_code)]
    pub(super) fn admit_unhandled_mmio(
        &self,
        vcpu: u32,
        access: hyper::vm::exit::MmioAccess,
    ) -> Option<super::diagnostics::UnhandledMmioReport> {
        self.machine
            .diagnostics
            .admit_unhandled_mmio(self.id.diagnostic_id(), vcpu, access)
    }

    pub(crate) fn interrupts(&self) -> &VmInterruptController {
        &self.machine.interrupts
    }

    fn endpoint(&self, vcpu: u32) -> Result<&super::endpoint::VcpuEndpoint, Error> {
        self.machine.endpoint(vcpu)
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
            .map(super::endpoint::VcpuEndpoint::reconcile_pending)
    }

    /// Publishes a completed saved interrupt-model mutation, then prompts the
    /// scheduler-authoritative running CPU, if any.
    #[allow(dead_code)]
    pub(super) fn publish_interrupt_reconcile(
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
            Err(super::endpoint_state::StateError::Closed(_)) => {
                return Err(Error::EndpointClosed);
            }
            Err(super::endpoint_state::StateError::Corrupt) => {
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
            None => Ok(()),
            Some(target) if target != cpu && crate::hal::vm::request_guest_exit(target) => Ok(()),
            Some(_) => crate::kernel::crash::fatal(format_args!(
                "HypeR: scheduler reports a running vCPU without a qualified guest-exit route"
            )),
        }?;
        Ok(())
    }

    pub(super) fn wfi_wait_ticket(&self, vcpu: u32) -> Result<super::endpoint::WaitTicket, Error> {
        Ok(self.endpoint(vcpu)?.wait_ticket())
    }

    pub(super) fn prepare_wfi_wait(
        &self,
        vcpu: u32,
        ticket: super::endpoint::WaitTicket,
    ) -> Result<super::endpoint::PreparedWait<'_>, Error> {
        self.endpoint(vcpu)?
            .prepare_wait(ticket)
            .map_err(|_| Error::Scheduler)
    }

    pub(super) fn arm_wfi_timer(
        &self,
        vcpu: u32,
        deadline: u64,
    ) -> Result<crate::kernel::time::ArmedReservedTimer<'_>, Error> {
        self.endpoint(vcpu)?
            .arm_timer(deadline)
            .map_err(|_| Error::Scheduler)
    }

    #[allow(dead_code)]
    pub(super) fn close_vcpu_endpoint(
        &self,
        vcpu: u32,
        expected_thread: ThreadId,
        reason: crate::hal::vm::VcpuTerminalReason,
    ) -> Result<super::endpoint_state::GuestCloseOutcome, Error> {
        let reason = match reason {
            crate::hal::vm::VcpuTerminalReason::MemoryFault => {
                super::endpoint_state::TerminalReason::MemoryFault
            }
            crate::hal::vm::VcpuTerminalReason::Mmio => super::endpoint_state::TerminalReason::Mmio,
            crate::hal::vm::VcpuTerminalReason::Synchronous => {
                super::endpoint_state::TerminalReason::Synchronous
            }
        };
        self.endpoint(vcpu)?
            .close(expected_thread, reason)
            .map_err(|_| Error::StaleIdentity)
    }

    #[allow(dead_code)]
    pub(in crate::kernel) fn administrative_stop_requested(
        &self,
        vcpu: u32,
        expected_thread: ThreadId,
    ) -> Result<Option<AdministrativeStopReason>, Error> {
        self.endpoint(vcpu)?
            .stop_requested(expected_thread)
            .map(|reason| reason.map(AdministrativeStopReason::from_endpoint))
            .map_err(|_| Error::StaleIdentity)
    }

    #[allow(dead_code)]
    pub(in crate::kernel) fn publish_hardware_detached(
        &self,
        vcpu: u32,
        expected_thread: ThreadId,
        reason: AdministrativeStopReason,
    ) -> Result<(), Error> {
        self.endpoint(vcpu)?
            .publish_hardware_detached(expected_thread, reason.endpoint())
            .map_err(|_| Error::StaleIdentity)
    }

    // Some selected guest platforms currently expose no emulated-device exit
    // path. Retain one stable aggregate accessor so VM ownership and layout do
    // not vary with the host architecture.
    #[allow(dead_code)]
    pub(super) fn devices(&self) -> &VirtualDeviceSet {
        &self.machine.devices
    }

    pub(super) fn with_address_space<R>(
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
                super::run_admission::AdmissionError::Closed => VmExecutionError::AdmissionClosed,
                super::run_admission::AdmissionError::CountExhausted => {
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
    admission: super::run_admission::RunAdmissionClaim,
    residency: Option<super::memory::GuestResidencyClaim>,
}

impl VmExecutionClaim {
    pub(in crate::kernel) fn attach_residency(
        &mut self,
        residency: super::memory::GuestResidencyClaim,
    ) -> Result<(), super::memory::GuestResidencyClaim> {
        if self.residency.is_some() {
            return Err(residency);
        }
        self.residency = Some(residency);
        Ok(())
    }

    pub(in crate::kernel) fn take_residency(
        &mut self,
    ) -> Option<super::memory::GuestResidencyClaim> {
        self.residency.take()
    }

    pub(in crate::kernel) fn restore_residency(
        &mut self,
        residency: super::memory::GuestResidencyClaim,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    AdministrativeStopUnsupported,
    Allocation,
    EndpointClosed,
    IdentityExhausted,
    InvalidReservation,
    NotInstalled,
    Quiescing,
    RegistryFull,
    Scheduler,
    StaleIdentity,
    UnknownVcpu,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::kernel) enum AdministrativeStopReason {
    Requested,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::kernel) enum VcpuClosureReason {
    Guest(crate::hal::vm::VcpuTerminalReason),
    Administrative(AdministrativeStopReason),
}

impl VcpuClosureReason {
    const fn endpoint(self) -> super::endpoint_state::ClosureReason {
        match self {
            Self::Guest(reason) => super::endpoint_state::ClosureReason::Guest(match reason {
                crate::hal::vm::VcpuTerminalReason::MemoryFault => {
                    super::endpoint_state::TerminalReason::MemoryFault
                }
                crate::hal::vm::VcpuTerminalReason::Mmio => {
                    super::endpoint_state::TerminalReason::Mmio
                }
                crate::hal::vm::VcpuTerminalReason::Synchronous => {
                    super::endpoint_state::TerminalReason::Synchronous
                }
            }),
            Self::Administrative(reason) => {
                super::endpoint_state::ClosureReason::Administrative(reason.endpoint())
            }
        }
    }
}

#[allow(dead_code)]
impl AdministrativeStopReason {
    pub(super) const fn endpoint(self) -> super::endpoint_state::AdministrativeStopReason {
        match self {
            Self::Requested => super::endpoint_state::AdministrativeStopReason::Requested,
        }
    }

    pub(super) const fn from_endpoint(
        reason: super::endpoint_state::AdministrativeStopReason,
    ) -> Self {
        match reason {
            super::endpoint_state::AdministrativeStopReason::Requested => Self::Requested,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(in crate::kernel) struct VcpuReapPublication {
    vm: VmId,
    vcpu: u32,
    thread: ThreadId,
    reason: VcpuClosureReason,
}

#[allow(dead_code)]
impl VcpuReapPublication {
    pub(in crate::kernel) const fn new(
        vm: VmId,
        vcpu: u32,
        thread: ThreadId,
        reason: VcpuClosureReason,
    ) -> Self {
        Self {
            vm,
            vcpu,
            thread,
            reason,
        }
    }
}

/// Rollback capability for one unpublished registry slot.
///
/// This type is intentionally neither `Copy` nor `Clone`. The builder consumes
/// it on successful publication; every earlier return drops it and returns the
/// slot to `Vacant` with a new generation.
pub(crate) struct VmReservation {
    id: VmId,
    hardware_vmid: Option<
        crate::kernel::mm::translation_id::IdentifierReservation<
            crate::kernel::mm::translation_id::Stage2Vmid,
        >,
    >,
    unpublished: bool,
}

impl VmReservation {
    pub(crate) const fn id(&self) -> VmId {
        self.id
    }

    pub(crate) fn take_hardware_vmid(&mut self) -> Result<Stage2IdentifierReservation, Error> {
        self.hardware_vmid.take().ok_or(Error::InvalidReservation)
    }
}

impl Drop for VmReservation {
    fn drop(&mut self) {
        if self.unpublished {
            REGISTRY.with(|registry| registry.cancel(self.id));
        }
    }
}

/// Locally complete VM state awaiting its single registry publication.
pub(crate) struct VmBuilder {
    machine: FallibleArc<VirtualMachine>,
    // Drop last so the logical and hardware identities cannot be reused while
    // unpublished VM-owned resources are still being destroyed.
    reservation: VmReservation,
}

impl VmBuilder {
    pub(crate) fn new(
        reservation: VmReservation,
        address_space: GuestAddressSpace,
        interrupts: VmInterruptController,
        devices: VirtualDeviceSet,
        vcpu_count: u32,
    ) -> Result<Self, Error> {
        let id = reservation.id;
        let endpoints = prepare_endpoints(vcpu_count)?;
        let machine = FallibleArc::try_new(VirtualMachine {
            id,
            address_space: InterruptSpinLock::new(address_space),
            execution: ExclusiveExecution::new(id.execution_owner()),
            run_admission: super::run_admission::RunAdmission::new(id.execution_owner()),
            interrupts,
            devices,
            diagnostics: super::diagnostics::VmDiagnostics::new(),
            endpoints,
        })
        .map_err(|_| Error::Allocation)?;
        Ok(Self {
            machine,
            reservation,
        })
    }

    fn vcpu_binding(&self) -> VmBinding {
        VmBinding {
            id: self.reservation.id,
            machine: self.machine.clone(),
        }
    }

    /// Prepares the non-runnable scheduler object and absorbs its rollback
    /// capability into the installation transaction.
    ///
    /// Only the returned typestate exposes installation. Neither the VM
    /// binding nor the reserved `ThreadId` can escape through a safe API.
    pub(crate) fn prepare_boot_vcpu(
        self,
        vcpu_id: u32,
        context: crate::hal::vm::VcpuContext,
    ) -> Result<PreparedVm, crate::kernel::task::scheduler::Error> {
        let dormant = super::vcpu::create_thread(self.vcpu_binding(), vcpu_id, context)?;
        // SAFETY: PreparedVm takes ownership of the rollback capability and
        // cannot expose this identity until registry installation succeeds.
        let thread = unsafe { dormant.id_for_vm_install() };
        let endpoint = self
            .machine
            .endpoint(vcpu_id)
            .map_err(|_| crate::kernel::task::scheduler::Error::InvalidThreadState)?;
        if endpoint.bind_thread(thread).is_err() {
            crate::hal::cpu::halt()
        }
        Ok(PreparedVm {
            dormant,
            machine: self.machine,
            reservation: self.reservation,
        })
    }
}

/// Fully allocated VM aggregate that can only be installed or rolled back.
pub(crate) struct PreparedVm {
    // Drop first so an unpublished vCPU releases its strong VM binding before
    // the builder owner and identity are released.
    dormant: crate::kernel::task::scheduler::DormantVcpuThread,
    machine: FallibleArc<VirtualMachine>,
    // Drop last for the same identity-reuse ordering as VmBuilder.
    reservation: VmReservation,
}

impl PreparedVm {
    /// Publishes the fully constructed aggregate in one registry transition.
    pub(crate) fn install(self) -> Result<InstalledVm, Error> {
        let id = self.reservation.id;
        REGISTRY.with(|registry| registry.validate_install(id, &self.machine))?;
        self.machine
            .address_space
            .with(GuestAddressSpace::activate_identifier_for_install)?;

        // Preserve PreparedVm's declared rollback drop order through every
        // fallible operation. Only the infallible publication tail may split
        // its fields into independent local owners.
        let Self {
            dormant,
            machine,
            mut reservation,
        } = self;
        let mut machine = Some(machine);
        // The first locked validation proved this uniquely reserved slot and
        // complete machine. A live reservation prevents any intervening slot
        // transition, so final publication is infallible after VMID activation.
        REGISTRY.with(|registry| registry.install_prevalidated(id, &mut machine));
        reservation.unpublished = false;
        drop(reservation);
        // SAFETY: Installation transferred one strong owner into the registry
        // before exposing the ThreadId retained by the VM and vCPU binding.
        let boot_vcpu = unsafe { dormant.commit_after_vm_install() };
        Ok(InstalledVm {
            id,
            boot_vcpu,
            control: VmControl::mint_for_install(id),
        })
    }
}

/// Capabilities exposed only after complete VM publication.
pub(crate) struct InstalledVm {
    id: VmId,
    boot_vcpu: ThreadId,
    control: VmControl,
}

impl InstalledVm {
    pub(super) fn into_boot_parts(self) -> (VmId, ThreadId, VmControl) {
        (self.id, self.boot_vcpu, self.control)
    }
}

/// Sole authority to start retirement of one installed VM incarnation.
///
/// Only `PreparedVm::install` can mint this non-Clone token. In particular,
/// no module can reconstruct it from the intentionally Copy `VmId`.
pub(super) struct VmControl {
    id: VmId,
}

impl VmControl {
    const fn mint_for_install(id: VmId) -> Self {
        Self { id }
    }

    /// Cuts service lookup before stopping producers and closing run admission.
    ///
    /// Unsupported architectures return the exact control without modifying
    /// registry or endpoint state.
    #[allow(dead_code)]
    pub(super) fn begin(self) -> Result<QuiescingVm, BeginFailure> {
        let capability = match crate::hal::vm::try_administrative_stop() {
            Ok(capability) => capability,
            Err(_) => {
                return Err(BeginFailure {
                    control: self,
                    error: BeginError::Unsupported,
                });
            }
        };
        if let Err(error) = begin_quiesce_control(self.id) {
            return Err(BeginFailure {
                control: self,
                error: BeginError::Registry(error),
            });
        }
        Ok(QuiescingVm {
            id: self.id,
            _capability: capability,
        })
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BeginError {
    Unsupported,
    Registry(Error),
}

/// Failed pre-cut transition retaining the exact linear authority.
#[allow(dead_code)]
pub(super) struct BeginFailure {
    control: VmControl,
    error: BeginError,
}

#[allow(dead_code)]
impl BeginFailure {
    pub(super) const fn error(&self) -> BeginError {
        self.error
    }

    pub(super) fn into_control(self) -> VmControl {
        self.control
    }
}

/// Authority for polling a VM after the irreversible Installed->Quiescing cut.
#[allow(dead_code)]
pub(super) struct QuiescingVm {
    id: VmId,
    // Retained until every endpoint and admitted run is quiescent. This binds
    // the common lifecycle authority to the exact selected mechanism proof.
    _capability: crate::hal::vm::AdministrativeStopCapability,
}

#[allow(dead_code)]
pub(super) enum QuiescePoll {
    Pending(QuiescingVm),
    Quiescent(QuiescentControl),
}

#[allow(dead_code)]
impl QuiescingVm {
    /// Attempts one allocation-free promotion to registry-held unique ownership.
    ///
    /// Failure to obtain uniqueness retains and returns this authority. The
    /// registry restores the exact Quiescing owner; it never consults a racy
    /// reference-count snapshot.
    pub(super) fn poll(self) -> QuiescePoll {
        match poll_quiescent_control(self.id) {
            Ok(true) => QuiescePoll::Quiescent(QuiescentControl { id: self.id }),
            Ok(false) => QuiescePoll::Pending(self),
            Err(error) => crate::kernel::crash::fatal(format_args!(
                "HypeR: VM quiescence poll violated registry state: {error:?}"
            )),
        }
    }
}

/// IDs-only proof that the registry owns the VM allocation uniquely.
///
/// Final retirement consumes this token before extracting any owner. Dropping
/// it leaves an inert `QuiescentHeld` tombstone and cannot free an active
/// address space.
#[allow(dead_code)]
pub(super) struct QuiescentControl {
    id: VmId,
}

#[allow(dead_code)]
impl QuiescentControl {
    pub(super) const fn id(&self) -> VmId {
        self.id
    }

    /// Retires architecture translation state and destroys this exact VM.
    ///
    /// Every fallible capability/topology/mailbox precheck precedes the first
    /// registry mutation. A precheck failure returns this exact authority;
    /// every later inconsistency is fail-stop with ownership retained.
    pub(super) fn retire(self) -> Result<(), RetirementFailure> {
        let capability = match crate::hal::vm::try_guest_stage2_retirement() {
            Ok(capability) => capability,
            Err(_) => {
                return Err(RetirementFailure {
                    control: self,
                    error: RetirementError::Unsupported,
                });
            }
        };
        let Some(topology) = crate::kernel::cpu::frozen_topology() else {
            return Err(RetirementFailure {
                control: self,
                error: RetirementError::TopologyUnavailable,
            });
        };
        let count = topology.count();
        if count == 0 || count != crate::kernel::cpu::online_cpu_count() {
            return Err(RetirementFailure {
                control: self,
                error: RetirementError::TopologyUnavailable,
            });
        }
        let mut transport =
            match crate::kernel::irq::cross_call::GuestStage2Transaction::try_acquire() {
                Ok(transport) => transport,
                Err(()) => {
                    return Err(RetirementFailure {
                        control: self,
                        error: RetirementError::TransportBusy,
                    });
                }
            };
        let machine = match REGISTRY.with(|registry| registry.begin_retirement(self.id)) {
            Ok(machine) => machine,
            Err(error) => crate::kernel::crash::fatal(format_args!(
                "HypeR: VM retirement registry cut failed after preflight: {error:?}"
            )),
        };
        let retirement = match machine
            .address_space
            .with(|address_space| address_space.begin_retirement(&capability, count))
        {
            Ok(retirement) => retirement,
            Err(error) => crate::kernel::crash::fatal(format_args!(
                "HypeR: guest stage-2 retirement failed after registry cut: {error:?}"
            )),
        };
        let outcome = transport.execute(retirement.local_request(), count, retirement.targets());
        if outcome.rejected_cpu.is_some() || outcome.ambiguous_cpu.is_some() {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: guest stage-2 retirement was not acknowledged"
            ));
        }
        machine
            .address_space
            .with(|address_space| address_space.finish_retirement(retirement));
        // Release the operation lease and serialized mailbox before
        // extracting or destroying the registry's unique owner.
        drop(machine);
        drop(transport);
        if let Err(error) = REGISTRY.with(|registry| registry.promote_retired(self.id)) {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: retired VM could not recover unique ownership: {error:?}"
            ));
        }
        let owner = match REGISTRY.with(|registry| registry.begin_destroy(self.id)) {
            Ok(owner) => owner,
            Err(error) => crate::kernel::crash::fatal(format_args!(
                "HypeR: retired VM destruction could not begin: {error:?}"
            )),
        };
        drop(owner);
        if let Err(error) = REGISTRY.with(|registry| registry.finish_destroy(self.id)) {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: retired VM slot could not advance generation: {error:?}"
            ));
        }
        Ok(())
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RetirementError {
    TopologyUnavailable,
    TransportBusy,
    Unsupported,
}

#[allow(dead_code)]
#[must_use = "retry with the exact quiescent VM retirement authority"]
pub(super) struct RetirementFailure {
    control: QuiescentControl,
    error: RetirementError,
}

#[allow(dead_code)]
impl RetirementFailure {
    pub(super) const fn error(&self) -> RetirementError {
        self.error
    }

    pub(super) fn into_control(self) -> QuiescentControl {
        self.control
    }
}

struct VirtualMachine {
    id: VmId,
    address_space: AddressSpaceLock,
    execution: ExclusiveExecution,
    run_admission: super::run_admission::RunAdmission,
    interrupts: VmInterruptController,
    // RISC-V's current selected set is zero-sized and has no exit consumer,
    // but the VM still owns it through the same lifecycle as other targets.
    #[allow(dead_code)]
    devices: VirtualDeviceSet,
    diagnostics: super::diagnostics::VmDiagnostics,
    // Construction reserves the complete endpoint array. It never grows after
    // publication, so references remain stable for the VM lifetime.
    endpoints: Vec<super::endpoint::VcpuEndpoint>,
}

impl VirtualMachine {
    fn endpoint(&self, vcpu: u32) -> Result<&super::endpoint::VcpuEndpoint, Error> {
        let index = usize::try_from(vcpu).map_err(|_| Error::UnknownVcpu)?;
        let endpoint = self.endpoints.get(index).ok_or(Error::UnknownVcpu)?;
        endpoint
            .is_valid_for(vcpu)
            .then_some(endpoint)
            .ok_or(Error::UnknownVcpu)
    }

    fn request_all_stops(&self) -> Result<(), Error> {
        for endpoint in &self.endpoints {
            let Some(thread) = endpoint.thread() else {
                if endpoint.lifecycle().map_err(|_| Error::StaleIdentity)?
                    != super::endpoint_state::Lifecycle::Unbound
                {
                    return Err(Error::StaleIdentity);
                }
                continue;
            };
            match endpoint
                .request_stop(
                    thread,
                    super::endpoint_state::AdministrativeStopReason::Requested,
                )
                .map_err(|_| Error::StaleIdentity)?
            {
                super::endpoint_state::StopRequestOutcome::Published
                | super::endpoint_state::StopRequestOutcome::AlreadyRequested => {
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
                super::endpoint_state::StopRequestOutcome::GuestTerminal(_)
                | super::endpoint_state::StopRequestOutcome::HardwareDetached
                | super::endpoint_state::StopRequestOutcome::Reaped
                | super::endpoint_state::StopRequestOutcome::Inactive => {}
            }
        }
        self.run_admission.close();
        Ok(())
    }

    fn is_quiescent(&self) -> bool {
        self.run_admission.is_closed_and_quiescent()
            && self.endpoints.iter().all(|endpoint| {
                matches!(
                    endpoint.lifecycle(),
                    Ok(super::endpoint_state::Lifecycle::Unbound
                        | super::endpoint_state::Lifecycle::Reaped(_))
                )
            })
    }
}

fn prepare_endpoints(vcpu_count: u32) -> Result<Vec<super::endpoint::VcpuEndpoint>, Error> {
    let count = usize::try_from(vcpu_count).map_err(|_| Error::Allocation)?;
    if count == 0 {
        return Err(Error::UnknownVcpu);
    }
    let mut endpoints = Vec::new();
    endpoints
        .try_reserve_exact(count)
        .map_err(|_| Error::Allocation)?;
    for id in 0..vcpu_count {
        endpoints.push(super::endpoint::VcpuEndpoint::try_new(id).map_err(|_| Error::Allocation)?);
    }
    Ok(endpoints)
}

struct VmRegistry {
    slots: [VmSlot; MAX_VIRTUAL_MACHINES],
}

enum VmSlot {
    Vacant {
        generation: u32,
    },
    Reserved {
        generation: u32,
    },
    Installed(FallibleArc<VirtualMachine>),
    Quiescing(FallibleArc<VirtualMachine>),
    QuiescentHeld {
        owner: UniqueFallibleArc<VirtualMachine>,
    },
    RetiringHeld(FallibleArc<VirtualMachine>),
    RetiredHeld {
        owner: UniqueFallibleArc<VirtualMachine>,
    },
    Destroying {
        generation: u32,
    },
    Exhausted,
}

impl VmRegistry {
    const fn new() -> Self {
        Self {
            slots: [const { VmSlot::Vacant { generation: 0 } }; MAX_VIRTUAL_MACHINES],
        }
    }

    fn reserve(&mut self) -> Result<VmId, Error> {
        let (slot, generation) = self
            .slots
            .iter()
            .enumerate()
            .find_map(|(index, slot)| match slot {
                VmSlot::Vacant { generation } => Some((index, *generation)),
                VmSlot::Reserved { .. } | VmSlot::Installed(_) | VmSlot::Exhausted => None,
                VmSlot::Quiescing(_)
                | VmSlot::QuiescentHeld { .. }
                | VmSlot::RetiringHeld(_)
                | VmSlot::RetiredHeld { .. }
                | VmSlot::Destroying { .. } => None,
            })
            .ok_or(Error::RegistryFull)?;
        let slot_u32 = u32::try_from(slot).map_err(|_| Error::IdentityExhausted)?;
        self.slots[slot] = VmSlot::Reserved { generation };
        Ok(VmId {
            slot: slot_u32,
            generation,
        })
    }

    fn cancel(&mut self, id: VmId) {
        let Ok(slot) = usize::try_from(id.slot) else {
            return;
        };
        let Some(entry) = self.slots.get_mut(slot) else {
            return;
        };
        if matches!(entry, VmSlot::Reserved { generation } if *generation == id.generation) {
            *entry = match id.generation.checked_add(1) {
                Some(generation) => VmSlot::Vacant { generation },
                None => VmSlot::Exhausted,
            };
        }
    }

    fn validate_install(&self, id: VmId, candidate: &VirtualMachine) -> Result<(), Error> {
        if candidate.id != id
            || candidate
                .endpoint(0)
                .ok()
                .and_then(super::endpoint::VcpuEndpoint::thread)
                .is_none()
        {
            return Err(Error::InvalidReservation);
        }
        let slot = usize::try_from(id.slot).map_err(|_| Error::InvalidReservation)?;
        let entry = self.slots.get(slot).ok_or(Error::InvalidReservation)?;
        if !matches!(entry, VmSlot::Reserved { generation } if *generation == id.generation) {
            return Err(Error::InvalidReservation);
        }
        Ok(())
    }

    fn install_prevalidated(
        &mut self,
        id: VmId,
        machine: &mut Option<FallibleArc<VirtualMachine>>,
    ) {
        let valid = machine
            .as_deref()
            .is_some_and(|candidate| self.validate_install(id, candidate).is_ok());
        if !valid {
            crate::hal::cpu::halt();
        }
        let Some(machine) = machine.take() else {
            crate::hal::cpu::halt();
        };
        let Some(entry) = self.slots.get_mut(id.slot as usize) else {
            crate::hal::cpu::halt();
        };
        *entry = VmSlot::Installed(machine);
    }

    fn installed(&self, id: VmId) -> Result<&FallibleArc<VirtualMachine>, Error> {
        let slot = usize::try_from(id.slot).map_err(|_| Error::StaleIdentity)?;
        match self.slots.get(slot) {
            Some(VmSlot::Installed(machine)) if machine.id == id => Ok(machine),
            Some(
                VmSlot::Installed(_)
                | VmSlot::Reserved { .. }
                | VmSlot::Vacant { .. }
                | VmSlot::Exhausted,
            ) => Err(Error::StaleIdentity),
            Some(
                VmSlot::Quiescing(_)
                | VmSlot::QuiescentHeld { .. }
                | VmSlot::RetiringHeld(_)
                | VmSlot::RetiredHeld { .. }
                | VmSlot::Destroying { .. },
            ) => Err(Error::StaleIdentity),
            None => Err(Error::NotInstalled),
        }
    }

    fn lease(&self, id: VmId) -> Result<VmLease, Error> {
        self.installed(id).map(|machine| VmLease {
            machine: machine.clone(),
        })
    }

    fn lifecycle_machine(&self, id: VmId) -> Result<&FallibleArc<VirtualMachine>, Error> {
        let slot = usize::try_from(id.slot).map_err(|_| Error::StaleIdentity)?;
        match self.slots.get(slot) {
            Some(VmSlot::Installed(machine) | VmSlot::Quiescing(machine)) if machine.id == id => {
                Ok(machine)
            }
            Some(_) => Err(Error::StaleIdentity),
            None => Err(Error::NotInstalled),
        }
    }

    fn begin_quiesce(&mut self, id: VmId) -> Result<FallibleArc<VirtualMachine>, Error> {
        let slot = usize::try_from(id.slot).map_err(|_| Error::StaleIdentity)?;
        let entry = self.slots.get_mut(slot).ok_or(Error::NotInstalled)?;
        if !matches!(entry, VmSlot::Installed(machine) if machine.id == id) {
            return Err(Error::StaleIdentity);
        }
        let old = core::mem::replace(entry, VmSlot::Exhausted);
        let VmSlot::Installed(machine) = old else {
            crate::hal::cpu::halt()
        };
        let lease = machine.clone();
        *entry = VmSlot::Quiescing(machine);
        Ok(lease)
    }

    #[allow(dead_code)]
    fn is_installed(&self, id: VmId) -> bool {
        self.installed(id).is_ok()
    }

    fn try_hold_quiescent(&mut self, id: VmId) -> Result<bool, Error> {
        let slot = usize::try_from(id.slot).map_err(|_| Error::StaleIdentity)?;
        let entry = self.slots.get_mut(slot).ok_or(Error::NotInstalled)?;
        let ready = match entry {
            VmSlot::Quiescing(machine) if machine.id == id => machine.is_quiescent(),
            VmSlot::Quiescing(_) => return Err(Error::StaleIdentity),
            _ => return Err(Error::Quiescing),
        };
        if !ready {
            return Ok(false);
        }
        let old = core::mem::replace(entry, VmSlot::Exhausted);
        let VmSlot::Quiescing(machine) = old else {
            crate::hal::cpu::halt()
        };
        match machine.try_into_unique() {
            Ok(owner) => {
                *entry = VmSlot::QuiescentHeld { owner };
                Ok(true)
            }
            Err(machine) => {
                *entry = VmSlot::Quiescing(machine);
                Ok(false)
            }
        }
    }

    fn begin_retirement(&mut self, id: VmId) -> Result<FallibleArc<VirtualMachine>, Error> {
        let slot = usize::try_from(id.slot).map_err(|_| Error::StaleIdentity)?;
        let entry = self.slots.get_mut(slot).ok_or(Error::NotInstalled)?;
        if !matches!(entry, VmSlot::QuiescentHeld { owner } if owner.id == id) {
            return Err(Error::StaleIdentity);
        }
        let old = core::mem::replace(entry, VmSlot::Exhausted);
        let VmSlot::QuiescentHeld { owner } = old else {
            crate::hal::cpu::halt()
        };
        let machine = owner.into_shared();
        let operation = machine.clone();
        *entry = VmSlot::RetiringHeld(machine);
        Ok(operation)
    }

    fn promote_retired(&mut self, id: VmId) -> Result<(), Error> {
        let slot = usize::try_from(id.slot).map_err(|_| Error::StaleIdentity)?;
        let entry = self.slots.get_mut(slot).ok_or(Error::NotInstalled)?;
        if !matches!(entry, VmSlot::RetiringHeld(machine) if machine.id == id) {
            return Err(Error::StaleIdentity);
        }
        let old = core::mem::replace(entry, VmSlot::Exhausted);
        let VmSlot::RetiringHeld(machine) = old else {
            crate::hal::cpu::halt()
        };
        match machine.try_into_unique() {
            Ok(owner) => {
                *entry = VmSlot::RetiredHeld { owner };
                Ok(())
            }
            Err(machine) => {
                *entry = VmSlot::RetiringHeld(machine);
                Err(Error::Quiescing)
            }
        }
    }

    fn begin_destroy(&mut self, id: VmId) -> Result<UniqueFallibleArc<VirtualMachine>, Error> {
        let slot = usize::try_from(id.slot).map_err(|_| Error::StaleIdentity)?;
        let entry = self.slots.get_mut(slot).ok_or(Error::NotInstalled)?;
        if !matches!(entry, VmSlot::RetiredHeld { owner } if owner.id == id) {
            return Err(Error::StaleIdentity);
        }
        let old = core::mem::replace(
            entry,
            VmSlot::Destroying {
                generation: id.generation,
            },
        );
        let VmSlot::RetiredHeld { owner } = old else {
            crate::hal::cpu::halt()
        };
        Ok(owner)
    }

    fn finish_destroy(&mut self, id: VmId) -> Result<(), Error> {
        let slot = usize::try_from(id.slot).map_err(|_| Error::StaleIdentity)?;
        let entry = self.slots.get_mut(slot).ok_or(Error::NotInstalled)?;
        if !matches!(entry, VmSlot::Destroying { generation } if *generation == id.generation) {
            return Err(Error::StaleIdentity);
        }
        *entry = match id.generation.checked_add(1) {
            Some(generation) => VmSlot::Vacant { generation },
            None => VmSlot::Exhausted,
        };
        Ok(())
    }
}

/// Scoped strong ownership returned by registry lookup.
///
/// This wrapper intentionally does not implement `Deref`; operations must use
/// a narrow method so registry internals cannot escape as untracked borrows.
struct VmLease {
    machine: FallibleArc<VirtualMachine>,
}

impl VmLease {
    fn with_address_space<R>(&self, operation: impl FnOnce(&mut GuestAddressSpace) -> R) -> R {
        self.machine.address_space.with(operation)
    }
}

/// Runs a VM operation through a generation-qualified strong lease.
///
/// The registry lock is released before `operation`; the temporary binding
/// keeps every VM-owned device, endpoint, and interrupt model alive. Callers
/// must release their subsystem locks before invoking scheduler notification.
#[allow(dead_code)]
pub(super) fn with_binding<R>(
    id: VmId,
    operation: impl FnOnce(&VmBinding) -> R,
) -> Result<R, Error> {
    let lease = REGISTRY.with(|registry| registry.lease(id))?;
    let binding = VmBinding {
        id,
        machine: lease.machine.clone(),
    };
    Ok(operation(&binding))
}

/// Completes endpoint reaping after the scheduler has dropped the Thread and
/// its strong `VmBinding`. Exact VM/thread generations reject stale callbacks.
pub(in crate::kernel) fn complete_vcpu_reap(publication: VcpuReapPublication) -> Result<(), Error> {
    let machine = REGISTRY.with(|registry| registry.lifecycle_machine(publication.vm).cloned())?;
    machine
        .endpoint(publication.vcpu)?
        .publish_reaped(publication.thread, publication.reason.endpoint())
        .map_err(|_| Error::StaleIdentity)
}

#[allow(dead_code)]
pub(super) fn is_installed(id: VmId) -> bool {
    REGISTRY.with(|registry| registry.is_installed(id))
}

fn begin_quiesce_control(id: VmId) -> Result<(), Error> {
    let machine = REGISTRY.with(|registry| registry.begin_quiesce(id))?;
    // Registry visibility was cut before producers and vCPU continuations are
    // stopped. Existing strong leases remain safe and prevent unique-owner
    // promotion until their callbacks return.
    // The console route is optional. Its stable device seam is an honest no-op
    // when the selected guest model has no route owner.
    super::device::clear_console_route_for_vm(id);
    if let Err(error) = machine.request_all_stops() {
        crate::kernel::crash::fatal(format_args!(
            "HypeR: VM quiesce failed after the registry cut: {error:?}"
        ));
    }
    Ok(())
}

fn poll_quiescent_control(id: VmId) -> Result<bool, Error> {
    REGISTRY.with(|registry| registry.try_hold_quiescent(id))
}

pub(crate) fn reserve() -> Result<VmReservation, Error> {
    let id = REGISTRY.with(VmRegistry::reserve)?;
    let hardware_vmid = match crate::kernel::mm::translation_id::reserve::<
        crate::kernel::mm::translation_id::Stage2Vmid,
    >(8)
    {
        Ok(identifier) => identifier,
        Err(_) => {
            REGISTRY.with(|registry| registry.cancel(id));
            return Err(Error::IdentityExhausted);
        }
    };
    Ok(VmReservation {
        id,
        hardware_vmid: Some(hardware_vmid),
        unpublished: true,
    })
}

/// Runs an operation under the installed VM's address-space lock.
///
/// Lookup clones a non-dereferenceable strong lease while the registry lock is
/// held. The lease keeps the aggregate alive after unlocking and is dropped
/// only after the address-space callback completes.
pub(super) fn with_address_space<R>(
    id: VmId,
    operation: impl FnOnce(&mut GuestAddressSpace) -> R,
) -> Result<R, Error> {
    let lease = REGISTRY.with(|registry| registry.lease(id))?;
    Ok(lease.with_address_space(operation))
}

#[cfg(feature = "kernel-self-test")]
pub(crate) fn verify_reservation_rollback() -> Result<(), Error> {
    let first = reserve()?;
    let first_id = first.id();
    drop(first);
    let second = reserve()?;
    if second.id().slot != first_id.slot || second.id().generation == first_id.generation {
        return Err(Error::InvalidReservation);
    }
    drop(second);
    Ok(())
}

#[cfg(feature = "kernel-self-test")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DormantVcpuQuiesceError {
    Registry(Error),
    Sleep(crate::kernel::task::SleepError),
    Time(crate::kernel::time::Error),
    Timeout,
}

#[cfg(feature = "kernel-self-test")]
impl From<Error> for DormantVcpuQuiesceError {
    fn from(error: Error) -> Self {
        Self::Registry(error)
    }
}

#[cfg(feature = "kernel-self-test")]
impl From<crate::kernel::task::SleepError> for DormantVcpuQuiesceError {
    fn from(error: crate::kernel::task::SleepError) -> Self {
        Self::Sleep(error)
    }
}

#[cfg(feature = "kernel-self-test")]
impl From<crate::kernel::time::Error> for DormantVcpuQuiesceError {
    fn from(error: crate::kernel::time::Error) -> Self {
        Self::Time(error)
    }
}

#[cfg(feature = "kernel-self-test")]
pub(crate) fn verify_dormant_vcpu_quiesce(
    installed: InstalledVm,
) -> Result<(), DormantVcpuQuiesceError> {
    let (_, _, control) = installed.into_boot_parts();
    let mut quiescing = match control.begin() {
        Ok(quiescing) => quiescing,
        Err(failure) => {
            return Err(DormantVcpuQuiesceError::Registry(match failure.error() {
                BeginError::Registry(error) => error,
                BeginError::Unsupported => Error::AdministrativeStopUnsupported,
            }));
        }
    };
    let deadline =
        crate::kernel::time::deadline_after(crate::kernel::task::TEST_PROGRESS_TIMEOUT_NS)?;
    loop {
        quiescing = match quiescing.poll() {
            QuiescePoll::Pending(quiescing) => quiescing,
            QuiescePoll::Quiescent(control) => {
                let retired = control.id();
                if control.retire().is_err() {
                    return Err(DormantVcpuQuiesceError::Registry(Error::Quiescing));
                }
                let replacement = reserve()?;
                let replacement_id = replacement.id();
                if replacement_id.slot != retired.slot
                    || replacement_id.generation == retired.generation
                {
                    return Err(DormantVcpuQuiesceError::Registry(Error::InvalidReservation));
                }
                drop(replacement);
                return Ok(());
            }
        };
        if hyper::hal::timer::deadline_reached(crate::kernel::time::monotonic_ticks(), deadline) {
            return Err(DormantVcpuQuiesceError::Timeout);
        }
        // A local yield does not guarantee that QEMU TCG schedules the CPU
        // running the vCPU or reaper. Block this Thread briefly so both owners
        // receive physical execution time without relying on logging delays.
        crate::kernel::task::sleep_ms(1)?;
    }
}
