// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Linear quiescence and architecture-retirement authorities.

#[cfg(feature = "kernel-self-test")]
use super::construction::InstalledVm;
use super::construction::VmControl;
#[cfg(feature = "kernel-self-test")]
use super::reserve;
use super::{Error, REGISTRY, VmId};

impl VmControl {
    /// Cuts service lookup before stopping producers and closing run admission.
    ///
    /// Unsupported architectures return the exact control without modifying
    /// registry or endpoint state.
    #[allow(dead_code)]
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

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::kernel::vm) enum BeginError {
    Unsupported,
    Registry(Error),
}

/// Failed pre-cut transition retaining the exact linear authority.
#[allow(dead_code)]
#[must_use = "retain or retry the exact installed VM lifecycle authority"]
pub(in crate::kernel::vm) struct BeginFailure {
    control: VmControl,
    error: BeginError,
}

#[allow(dead_code)]
impl BeginFailure {
    pub(in crate::kernel::vm) const fn error(&self) -> BeginError {
        self.error
    }

    pub(in crate::kernel::vm) fn into_control(self) -> VmControl {
        self.control
    }
}

/// Authority for polling a VM after the irreversible Installed->Quiescing cut.
#[allow(dead_code)]
#[must_use = "poll the exact quiescing VM authority until retirement can begin"]
pub(in crate::kernel::vm) struct QuiescingVm {
    id: VmId,
    // Retained until every endpoint and admitted run is quiescent. This binds
    // the common lifecycle authority to the exact selected mechanism proof.
    _capability: crate::hal::vm::AdministrativeStopCapability,
}

#[allow(dead_code)]
#[must_use = "retain the exact VM authority returned by a quiescence poll"]
pub(in crate::kernel::vm) enum QuiescePoll {
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
#[allow(dead_code)]
#[must_use = "retire the exact quiescent VM authority"]
pub(in crate::kernel::vm) struct QuiescentControl {
    id: VmId,
}

#[allow(dead_code)]
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

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::kernel::vm) enum RetirementError {
    TopologyUnavailable,
    TransportBusy,
    Unsupported,
}

#[allow(dead_code)]
#[must_use = "retry with the exact quiescent VM retirement authority"]
pub(in crate::kernel::vm) struct RetirementFailure {
    control: QuiescentControl,
    error: RetirementError,
}

#[allow(dead_code)]
impl RetirementFailure {
    pub(in crate::kernel::vm) const fn error(&self) -> RetirementError {
        self.error
    }

    pub(in crate::kernel::vm) fn into_control(self) -> QuiescentControl {
        self.control
    }
}

fn begin_quiesce_control(id: VmId) -> Result<(), Error> {
    let machine = REGISTRY.with(|registry| registry.begin_quiesce(id))?;
    // Registry visibility was cut before producers and vCPU continuations are
    // stopped. Existing strong leases remain safe and prevent unique-owner
    // promotion until their callbacks return.
    // Disconnect the explicitly assigned serial endpoint before stopping vCPUs.
    machine.disconnect_virtual_serial();
    // Publish every endpoint's durable administrative reason before disabling
    // devices. A concurrent admitted UART access may observe a closed device;
    // its terminal exit must see the already-published stop request.
    if let Err(error) = machine.request_all_stops() {
        crate::kernel::crash::fatal(format_args!(
            "HypeR: VM quiesce failed after the registry cut: {error:?}"
        ));
    }
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
