// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Linear quiescence and architecture-retirement authorities.

use super::super::retirement_observation::RetirementError;
#[cfg(feature = "kernel-self-test")]
use super::construction::InstalledVm;
use super::construction::VmControl;
#[cfg(feature = "kernel-self-test")]
use super::reserve;
use super::{Error, REGISTRY, VmId};

impl VmControl {
    /// Publishes administrative stop before cutting service lookup and devices.
    ///
    /// Unsupported architectures return the exact control without modifying
    /// registry or endpoint state.
    pub(in crate::kernel::vm) fn begin(self) -> Result<QuiescingVm, BeginFailure> {
        let capability = match crate::hal::vm::try_administrative_stop() {
            Ok(capability) => capability,
            Err(_) => {
                return Err(BeginFailure {
                    control: self,
                    error: BeginError::Unsupported,
                });
            }
        };
        if let Err(error) = begin_quiesce_control(self.id()) {
            return Err(BeginFailure {
                control: self,
                error: BeginError::Registry(error),
            });
        }
        Ok(QuiescingVm {
            id: self.id(),
            _capability: capability,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::kernel::vm) enum BeginError {
    Unsupported,
    Registry(Error),
}

/// Failed pre-stop transition retaining the exact linear authority.
#[must_use = "retain or retry the exact installed VM lifecycle authority"]
pub(in crate::kernel::vm) struct BeginFailure {
    control: VmControl,
    error: BeginError,
}

impl BeginFailure {
    pub(in crate::kernel::vm) const fn error(&self) -> BeginError {
        self.error
    }

    #[expect(
        dead_code,
        reason = "pre-stop failure retains retry authority; current reaper fail-stops instead of retrying begin"
    )]
    pub(in crate::kernel::vm) fn into_control(self) -> VmControl {
        self.control
    }
}

/// Authority for polling a VM after the irreversible Installed->Quiescing cut.
#[must_use = "poll the exact quiescing VM authority until retirement can begin"]
pub(in crate::kernel::vm) struct QuiescingVm {
    id: VmId,
    // Retained until every endpoint and admitted run is quiescent. This binds
    // the common lifecycle authority to the exact selected mechanism proof.
    _capability: crate::hal::vm::AdministrativeStopCapability,
}

#[must_use = "retain the exact VM authority returned by a quiescence poll"]
pub(in crate::kernel::vm) enum QuiescePoll {
    Pending(QuiescingVm),
    Quiescent(QuiescentControl),
}

impl QuiescingVm {
    /// Attempts one allocation-free promotion to registry-held unique ownership.
    ///
    /// Failure to obtain uniqueness retains and returns this authority. The
    /// registry restores the exact Quiescing owner; it never consults a racy
    /// reference-count snapshot.
    pub(in crate::kernel::vm) fn poll(self) -> QuiescePoll {
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
#[must_use = "retire the exact quiescent VM authority"]
pub(in crate::kernel::vm) struct QuiescentControl {
    id: VmId,
}

impl QuiescentControl {
    pub(in crate::kernel::vm) const fn id(&self) -> VmId {
        self.id
    }

    /// Retires architecture translation state and destroys this exact VM.
    ///
    /// Every fallible capability/topology/mailbox precheck precedes the first
    /// registry mutation. A precheck failure returns this exact authority;
    /// every later inconsistency is fail-stop with ownership retained.
    pub(in crate::kernel::vm) fn retire(self) -> Result<(), RetirementFailure> {
        let physical = match REGISTRY.with(|registry| registry.quiescent_physical(self.id)) {
            Ok(physical) => physical,
            Err(error) => crate::kernel::crash::fatal(format_args!(
                "quiescent physical owner lookup failed: {error:?}"
            )),
        };
        if let Some(physical) = physical
            && physical.object().quiesce().is_err()
        {
            // The exact quiescent owner, all imported pages and the device
            // claim remain retained. A reset timeout never permits reuse.
            return Err(RetirementFailure {
                control: self,
                error: RetirementError::DeviceQuarantined,
            });
        }
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
            .with_address_space(|address_space| address_space.begin_retirement(&capability, count))
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
        machine.with_address_space(|address_space| address_space.finish_retirement(retirement));
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

#[must_use = "retry with the exact quiescent VM retirement authority"]
pub(in crate::kernel::vm) struct RetirementFailure {
    control: QuiescentControl,
    error: RetirementError,
}

impl RetirementFailure {
    pub(in crate::kernel::vm) const fn error(&self) -> RetirementError {
        self.error
    }

    pub(in crate::kernel::vm) fn into_control(self) -> QuiescentControl {
        self.control
    }
}

fn begin_quiesce_control(id: VmId) -> Result<(), Error> {
    // Lookup is the only reversible preflight. Retain the complete machine,
    // then release the registry lock before notifying any scheduler endpoint.
    let lease = REGISTRY.with(|registry| registry.lease(id))?;
    if let Err(error) = lease.machine.request_all_stops() {
        crate::kernel::crash::fatal(format_args!(
            "HypeR: VM stop publication failed after quiescence commitment: {error:?}"
        ));
    }
    // Every endpoint now carries its durable administrative reason and run
    // admission is closed. Only now may weak lookup or MMIO routes disappear:
    // an in-flight access must not misclassify teardown as a guest MMIO fault.
    // The linear VmControl is the sole authority for this slot transition, so
    // failure after stop publication cannot masquerade as reversible failure.
    let machine = match REGISTRY.with(|registry| registry.begin_quiesce(id)) {
        Ok(machine) => machine,
        Err(error) => crate::kernel::crash::fatal(format_args!(
            "HypeR: VM registry cut failed after stop publication: {error:?}"
        )),
    };
    drop(lease);
    machine.disconnect_virtual_serial();
    machine.close_io_routes();
    if let Err(error) = machine.quiesce_devices() {
        crate::kernel::crash::fatal(format_args!(
            "HypeR: VM device quiesce failed after the registry cut: {error:?}"
        ));
    }
    Ok(())
}

fn poll_quiescent_control(id: VmId) -> Result<bool, Error> {
    REGISTRY.with(|registry| registry.try_hold_quiescent(id))
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

/// Exercises the production queue while retaining the handle-visible VM owner.
#[cfg(feature = "kernel-self-test")]
pub(crate) fn verify_observed_vm_quiesce(
    installed: InstalledVm,
) -> Result<
    hyper::mm::FallibleArc<crate::kernel::vm::installed::InstalledMachine>,
    DormantVcpuQuiesceError,
> {
    let old = installed.id_for_test();
    let owner = installed.publish_handle_lifecycle();
    crate::kernel::vm::installed::InstalledMachine::request_stop(&owner);
    crate::kernel::vm::installed::InstalledMachine::request_stop(&owner);
    let deadline =
        crate::kernel::time::deadline_after(crate::kernel::task::TEST_PROGRESS_TIMEOUT_NS)?;
    while owner.snapshot().phase
        != hyper::abi::native::HYPER_NATIVE_VIRTUAL_MACHINE_PHASE_STOPPED as u32
    {
        if hyper::hal::timer::deadline_reached(crate::kernel::time::monotonic_ticks(), deadline) {
            return Err(DormantVcpuQuiesceError::Timeout);
        }
        crate::kernel::task::sleep_ms(1)?;
    }
    let replacement = reserve()?;
    let new = replacement.id();
    if old.slot != new.slot || old.generation == new.generation {
        return Err(DormantVcpuQuiesceError::Registry(Error::InvalidReservation));
    }
    // Stopping a tombstone again cannot enqueue authority for its replacement.
    crate::kernel::vm::installed::InstalledMachine::request_stop(&owner);
    drop(replacement);
    Ok(owner)
}
