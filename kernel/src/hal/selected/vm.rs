// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Selected hardware-virtualization capabilities.
//!
//! Kernel VM policy owns identity, publication, scheduling, demand paging,
//! devices, and exit disposition. This facade owns only the selected vCPU
//! machine state and the mechanisms which operate on it.
//!
//! `VcpuContext`, `Stage2AddressSpace`, and `InterruptController` are selected
//! layout-state re-exports rather than opaque wrappers. Assembly and backend
//! code consume their addresses and layouts directly, so wrapping them would
//! require unchecked pointer conversion or an architecture-to-HAL callback.
//! Kernel policy must still use only the operations exposed by this module.

use hyper::hal::interrupt::{HostInterruptBinding, InterruptId};

#[cfg(CONFIG_ARCH_AARCH64)]
pub(crate) use crate::arch::vm::GicAccessError;
pub(crate) use crate::arch::vm::{
    DeviceError, ExitServiceError, ExitServices, ExitServicesReady, InterruptInitializationError,
    PreparedInterruptVirtualization, RegisterValidationError, Stage2AddressSpace, Stage2Error,
    VcpuContext, VirtualInterruptError,
};
#[cfg(any(CONFIG_ARCH_AARCH64, CONFIG_ARCH_RISCV64))]
pub(crate) use crate::arch::vm::{GuestSyncAction, GuestSyncExit};
pub use crate::arch::vm::{InterruptController, InterruptError, VcpuInterruptError};

/// Selected virtualization capability which is unavailable on this target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(CONFIG_ARCH_AARCH64, allow(dead_code))]
pub enum UnsupportedCapability {
    AdministrativeStop,
    GuestStage2Retirement,
}

/// Pre-mutation proof that this backend supports typed administrative stop.
pub(crate) struct AdministrativeStopCapability {
    _private: (),
}

pub(crate) fn try_administrative_stop()
-> Result<AdministrativeStopCapability, UnsupportedCapability> {
    #[cfg(CONFIG_ARCH_AARCH64)]
    {
        Ok(AdministrativeStopCapability { _private: () })
    }
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    {
        Err(UnsupportedCapability::AdministrativeStop)
    }
}

/// Opaque local stage-2 invalidation request issued to every sticky CPU.
#[derive(Clone, Copy)]
pub(crate) struct GuestStage2RetirementRequest {
    #[cfg(CONFIG_ARCH_AARCH64)]
    backend: crate::arch::vm::GuestStage2RetirementRequest,
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    never: core::convert::Infallible,
}

/// Pre-mutation proof that the selected backend supports stage-2 retirement.
///
/// Unsupported targets cannot construct this token. Keeping capability
/// discovery separate from request preparation lets callers complete every
/// fallible mechanism check before cutting guest residency or identity state.
pub(crate) struct GuestStage2RetirementCapability {
    _private: (),
}

pub(crate) fn try_guest_stage2_retirement()
-> Result<GuestStage2RetirementCapability, UnsupportedCapability> {
    #[cfg(CONFIG_ARCH_AARCH64)]
    {
        Ok(GuestStage2RetirementCapability { _private: () })
    }
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    {
        Err(UnsupportedCapability::GuestStage2Retirement)
    }
}

pub(crate) fn prepare_guest_stage2_retirement(
    capability: &GuestStage2RetirementCapability,
    address_space: &Stage2AddressSpace,
) -> GuestStage2RetirementRequest {
    #[cfg(CONFIG_ARCH_AARCH64)]
    {
        let _ = capability;
        GuestStage2RetirementRequest {
            backend: crate::arch::vm::prepare_guest_stage2_retirement(address_space),
        }
    }
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    {
        let _ = (capability, address_space);
        // The private capability cannot be obtained on this target.
        crate::hal::cpu::halt()
    }
}

pub(crate) fn service_guest_stage2_retirement(request: GuestStage2RetirementRequest) {
    #[cfg(CONFIG_ARCH_AARCH64)]
    {
        crate::arch::vm::service_guest_stage2_retirement(request.backend)
    }
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    {
        match request.never {}
    }
}

/// Selected per-vCPU machine state retained by one scheduler execution.
pub struct VcpuHardwareState {
    context: VcpuContext,
    runtime_authorized: bool,
}

/// Proof that every dependency required for normal guest entry is active.
///
/// The kernel may mint this only after exit services, register validation,
/// virtual devices, timer routing, and interrupt virtualization have all
/// completed. Copies describe the same irreversible boot publication.
#[derive(Clone, Copy)]
#[must_use]
pub(crate) struct VmEntryReady {
    _private: (),
}

impl VcpuHardwareState {
    pub(crate) const fn new(context: VcpuContext, _entry: &VmEntryReady) -> Self {
        Self {
            context,
            runtime_authorized: true,
        }
    }

    #[cfg(CONFIG_ARCH_AARCH64)]
    const fn for_validation(context: VcpuContext, _services: &ExitServicesReady) -> Self {
        Self {
            context,
            runtime_authorized: false,
        }
    }
}

pub(crate) fn install_exit_services(
    services: ExitServices,
) -> Result<ExitServicesReady, ExitServiceError> {
    crate::arch::vm::install_exit_services(services)
}

/// Commits the one-way transition from callback publication to guest entry.
///
/// # Safety
///
/// Register validation and selected virtual-device initialization must have
/// completed. Every host timer route and interrupt-virtualization dependency
/// must be active and no longer eligible for rollback. The caller must own the
/// only VM initialization transaction and invoke this commit at most once.
pub(crate) unsafe fn commit_entry_initialization(_services: ExitServicesReady) -> VmEntryReady {
    VmEntryReady { _private: () }
}

pub(crate) fn validate_register_interface() -> Result<(), RegisterValidationError> {
    crate::arch::vm::validate_register_interface()
}

pub(crate) fn initialize_devices(
    timer_interrupt: InterruptId,
    host_timer_interrupt: Option<HostInterruptBinding>,
) -> Result<(), DeviceError> {
    crate::arch::vm::initialize_devices(timer_interrupt, host_timer_interrupt)
}

pub(crate) fn prepare_interrupts(
    host_timer_interrupt: Option<HostInterruptBinding>,
) -> Result<PreparedInterruptVirtualization, InterruptInitializationError> {
    crate::arch::vm::prepare_interrupts(host_timer_interrupt)
}

pub(crate) fn commit_interrupts(
    prepared: PreparedInterruptVirtualization,
) -> Result<(), InterruptInitializationError> {
    crate::arch::vm::commit_interrupts(prepared)
}

pub(crate) fn initialize_vcpu_interrupts(
    state: &mut VcpuHardwareState,
) -> Result<(), VirtualInterruptError> {
    state.context.initialize_virtual_interrupts().map(|_| ())
}

/// Sets the guest-visible virtual counter before the context becomes runnable.
pub(crate) fn set_virtual_count(context: &mut VcpuContext, physical: u64, value: u64) {
    context.set_virtual_count(physical, value);
}

/// Invalid guest-general-register assignment in an initial context plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct InitialContextError;

/// One guest-general-register value in an initial machine-context plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct InitialRegisterAssignment {
    index: usize,
    value: u64,
}

impl InitialRegisterAssignment {
    pub(crate) const fn new(index: usize, value: u64) -> Self {
        Self { index, value }
    }
}

/// Realizes a guest-ABI register plan in the selected machine context.
///
/// Register indices use the selected guest ISA's architectural general-
/// register numbering. Linux boot policy owns the values and their meaning;
/// the HAL owns construction of the machine context which carries them.
pub(crate) fn prepare_initial_context(
    entry: u64,
    registers: &[InitialRegisterAssignment],
) -> Result<VcpuContext, InitialContextError> {
    let mut context = VcpuContext::new(entry);
    for assignment in registers {
        let register = context
            .general
            .get_mut(assignment.index)
            .ok_or(InitialContextError)?;
        *register = assignment.value;
    }
    Ok(context)
}

/// Applies the selected architecture's local interrupt state for guest entry.
///
/// Callers must not assume this unmasks interrupts. In particular, `AArch64`
/// keeps IRQs masked across the non-atomic EL2-to-guest context transaction.
pub(crate) fn prepare_interrupts_for_entry() {
    crate::arch::vm::prepare_interrupts_for_entry();
}

/// Activates the local machine state for a stopped vCPU.
///
/// # Safety
///
/// `state` must be pinned and exclusively owned by the stopped vCPU. No guest
/// may execute concurrently, and local interrupts must remain masked. A caller
/// which can proceed to guest entry must activate the selected second-stage
/// hierarchy before entry; machine-state-only validation need not do so. An
/// error must leave local vCPU hardware detached so ownership can be released.
pub(crate) unsafe fn activate_hardware(
    state: &mut VcpuHardwareState,
    vcpu_id: u32,
    interrupts: &InterruptController,
    physical_count: u64,
) -> Result<bool, VcpuInterruptError> {
    // SAFETY: The facade preserves stopped-state ownership, stage-2, and
    // interrupt-mask requirements while hiding the backend context layout.
    unsafe {
        crate::arch::vm::activate_vcpu_hardware(
            &mut state.context,
            vcpu_id,
            interrupts,
            physical_count,
        )
    }
}

/// Saves and detaches the current vCPU's local machine state.
///
/// # Safety
///
/// `state` must exclusively own the active local machine state. Guest execution
/// must have stopped and local interrupts must remain masked.
pub(crate) unsafe fn deactivate_hardware(
    state: &mut VcpuHardwareState,
    vcpu_id: u32,
    interrupts: &InterruptController,
    physical_count: u64,
) -> Result<(), VcpuInterruptError> {
    // SAFETY: The facade preserves active ownership and interrupt masking.
    unsafe {
        crate::arch::vm::deactivate_vcpu_hardware(
            &mut state.context,
            vcpu_id,
            interrupts,
            physical_count,
        )
    }
}

/// Opaque exit facts returned only after the selected backend closes guest execution.
///
/// Non-returning backends cannot construct this type: its selected payload is
/// uninhabited. This keeps the lifecycle interface uniform without claiming a
/// typed unwind which the machine backend does not provide.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(CONFIG_ARCH_AARCH64), allow(dead_code))]
pub(crate) struct VcpuRunExit {
    #[cfg(CONFIG_ARCH_AARCH64)]
    backend: crate::arch::vm::GuestRunExit,
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    never: core::convert::Infallible,
}

/// Architecture-neutral terminal policy attributed to valid guest input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(CONFIG_ARCH_AARCH64), allow(dead_code))]
pub(crate) enum VcpuTerminalReason {
    MemoryFault,
    Mmio,
    Synchronous,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(CONFIG_ARCH_AARCH64)]
pub(crate) enum VcpuSynchronousTerminal {
    Undecodable,
    Failed {
        exit: GuestSyncExit,
        failure: VcpuInterruptError,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(CONFIG_ARCH_AARCH64), allow(dead_code))]
pub(crate) enum VcpuTerminalCause {
    MemoryFault,
    Mmio,
    #[cfg(CONFIG_ARCH_AARCH64)]
    Synchronous(VcpuSynchronousTerminal),
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    Synchronous,
}

/// Complete architecture exit state for one guest-policy terminal stop.
///
/// This copied value remains valid after the selected backend has detached
/// live vCPU hardware. Wait and administrative exits cannot construct it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(CONFIG_ARCH_AARCH64), allow(dead_code))]
pub(crate) struct VcpuTerminalExit {
    cause: VcpuTerminalCause,
    syndrome: u64,
    fault_address: u64,
    program_counter: u64,
    processor_state: u64,
    vector: u64,
}

impl VcpuTerminalExit {
    pub(crate) const fn reason(self) -> VcpuTerminalReason {
        match self.cause {
            VcpuTerminalCause::MemoryFault => VcpuTerminalReason::MemoryFault,
            VcpuTerminalCause::Mmio => VcpuTerminalReason::Mmio,
            #[cfg(CONFIG_ARCH_AARCH64)]
            VcpuTerminalCause::Synchronous(_) => VcpuTerminalReason::Synchronous,
            #[cfg(not(CONFIG_ARCH_AARCH64))]
            VcpuTerminalCause::Synchronous => VcpuTerminalReason::Synchronous,
        }
    }

    pub(crate) const fn cause(self) -> VcpuTerminalCause {
        self.cause
    }

    pub(crate) const fn syndrome(self) -> u64 {
        self.syndrome
    }

    pub(crate) const fn fault_address(self) -> u64 {
        self.fault_address
    }

    pub(crate) const fn program_counter(self) -> u64 {
        self.program_counter
    }

    pub(crate) const fn processor_state(self) -> u64 {
        self.processor_state
    }

    pub(crate) const fn vector(self) -> u64 {
        self.vector
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(CONFIG_ARCH_AARCH64), allow(dead_code))]
pub(crate) enum VcpuAdministrativeStopReason {
    Requested,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(CONFIG_ARCH_AARCH64), allow(dead_code))]
pub(crate) enum VcpuWaitReason {
    Interrupt,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(CONFIG_ARCH_AARCH64), allow(dead_code))]
pub(crate) enum VcpuTimerWake {
    None,
    PendingNow,
    Deadline(u64),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(CONFIG_ARCH_AARCH64), allow(dead_code))]
pub(crate) struct VcpuWfiState {
    pub(crate) interrupt_may_wake: bool,
    pub(crate) timer: VcpuTimerWake,
}

pub(crate) fn stopped_wfi_state(
    state: &VcpuHardwareState,
    vcpu_id: u32,
    interrupts: &InterruptController,
    physical_count: u64,
) -> Result<VcpuWfiState, StoppedVcpuQueryError> {
    #[cfg(CONFIG_ARCH_AARCH64)]
    {
        let interrupt_may_wake = interrupts
            .may_wake_wfi(hyper::vm::interrupt::VirtualCpuId::new(vcpu_id))
            .map_err(crate::arch::vm::VcpuInterruptError::Controller)
            .map_err(StoppedVcpuQueryError::Backend)?;
        let timer = match state.context.virtual_timer_wfi_wake_at(physical_count) {
            hyper::drivers::timer::arm_generic::VirtualTimerWake::None => VcpuTimerWake::None,
            hyper::drivers::timer::arm_generic::VirtualTimerWake::PendingNow => {
                VcpuTimerWake::PendingNow
            }
            hyper::drivers::timer::arm_generic::VirtualTimerWake::Deadline(deadline) => {
                VcpuTimerWake::Deadline(deadline)
            }
        };
        Ok(VcpuWfiState {
            interrupt_may_wake,
            timer,
        })
    }
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    {
        let _ = (state, vcpu_id, interrupts, physical_count);
        Err(StoppedVcpuQueryError::Unsupported)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub(crate) enum StoppedVcpuQueryError {
    Unsupported,
    Backend(VcpuInterruptError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(CONFIG_ARCH_AARCH64, allow(dead_code))]
pub enum ActiveInterruptReconcileError {
    Unsupported,
    Backend(VcpuInterruptError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(CONFIG_ARCH_AARCH64), allow(dead_code))]
pub(crate) enum VcpuRunDisposition {
    Wait(VcpuWaitReason),
    Terminal(VcpuTerminalExit),
    AdministrativeStop(VcpuAdministrativeStopReason),
}

impl core::fmt::Display for VcpuTerminalReason {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::MemoryFault => "memory fault policy stop",
            Self::Mmio => "MMIO policy stop",
            Self::Synchronous => "synchronous exit policy stop",
        })
    }
}

impl VcpuRunExit {
    #[cfg_attr(not(CONFIG_ARCH_AARCH64), allow(dead_code))]
    pub(crate) const fn disposition(self) -> VcpuRunDisposition {
        #[cfg(CONFIG_ARCH_AARCH64)]
        {
            match self.backend {
                crate::arch::vm::GuestRunExit::Wait(
                    crate::arch::vm::GuestWaitReason::Interrupt,
                ) => VcpuRunDisposition::Wait(VcpuWaitReason::Interrupt),
                crate::arch::vm::GuestRunExit::Terminal(exit) => {
                    let cause = match exit.cause() {
                        crate::arch::vm::GuestTerminalCause::MemoryFault => {
                            VcpuTerminalCause::MemoryFault
                        }
                        crate::arch::vm::GuestTerminalCause::Mmio => VcpuTerminalCause::Mmio,
                        crate::arch::vm::GuestTerminalCause::Synchronous(synchronous) => {
                            let synchronous = match synchronous {
                                crate::arch::vm::GuestSynchronousTerminal::Undecodable => {
                                    VcpuSynchronousTerminal::Undecodable
                                }
                                crate::arch::vm::GuestSynchronousTerminal::Failed {
                                    exit,
                                    failure:
                                        crate::arch::vm::GuestSyncFailure::VirtualInterrupt(failure),
                                } => VcpuSynchronousTerminal::Failed { exit, failure },
                            };
                            VcpuTerminalCause::Synchronous(synchronous)
                        }
                    };
                    VcpuRunDisposition::Terminal(VcpuTerminalExit {
                        cause,
                        syndrome: exit.syndrome(),
                        fault_address: exit.fault_address(),
                        program_counter: exit.program_counter(),
                        processor_state: exit.processor_state(),
                        vector: exit.vector(),
                    })
                }
                crate::arch::vm::GuestRunExit::AdministrativeStop(
                    crate::arch::vm::GuestAdministrativeStopReason::Requested,
                ) => {
                    VcpuRunDisposition::AdministrativeStop(VcpuAdministrativeStopReason::Requested)
                }
            }
        }
        #[cfg(not(CONFIG_ARCH_AARCH64))]
        {
            match self.never {}
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(CONFIG_ARCH_AARCH64), allow(dead_code))]
pub(crate) enum VcpuRunError {
    Owner,
    Return,
    State,
}

#[cfg(CONFIG_ARCH_AARCH64)]
impl From<crate::arch::vm::GuestRunError> for VcpuRunError {
    fn from(error: crate::arch::vm::GuestRunError) -> Self {
        match error {
            crate::arch::vm::GuestRunError::Owner => Self::Owner,
            crate::arch::vm::GuestRunError::Return => Self::Return,
            crate::arch::vm::GuestRunError::State => Self::State,
        }
    }
}

/// Linear proof that the selected backend has stopped lower-world execution.
///
/// On non-returning backends the payload is uninhabited, so safe code cannot
/// manufacture a false stopped-state proof.
#[must_use = "stopped vCPU hardware must be detached exactly once"]
#[cfg_attr(not(CONFIG_ARCH_AARCH64), allow(dead_code))]
pub(crate) struct StoppedVcpuRun {
    #[cfg(CONFIG_ARCH_AARCH64)]
    backend: crate::arch::vm::StoppedGuestRun,
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    never: core::convert::Infallible,
}

impl StoppedVcpuRun {
    #[cfg_attr(not(CONFIG_ARCH_AARCH64), allow(dead_code))]
    pub(crate) fn exit(&self) -> VcpuRunExit {
        #[cfg(CONFIG_ARCH_AARCH64)]
        {
            VcpuRunExit {
                backend: self.backend.exit(),
            }
        }
        #[cfg(not(CONFIG_ARCH_AARCH64))]
        {
            match self.never {}
        }
    }
}

/// Runs an active vCPU until the selected backend either returns a linear
/// stopped proof or transfers control through its non-returning entry path.
#[cfg_attr(not(CONFIG_ARCH_AARCH64), allow(dead_code))]
pub(crate) unsafe fn run(state: *mut VcpuHardwareState) -> Result<StoppedVcpuRun, VcpuRunError> {
    if state.is_null() || !state.is_aligned() {
        return Err(VcpuRunError::Owner);
    }
    // SAFETY: The caller guarantees a valid exclusive state pointer.
    if !unsafe { (*state).runtime_authorized } {
        crate::hal::cpu::halt()
    }
    // SAFETY: The validated state pointer exclusively owns this pinned field.
    let context = unsafe { core::ptr::addr_of_mut!((*state).context) };
    #[cfg(CONFIG_ARCH_AARCH64)]
    {
        // SAFETY: The facade preserves the backend's active, pinned run contract.
        unsafe { VcpuContext::run(context) }
            .map(|backend| StoppedVcpuRun { backend })
            .map_err(Into::into)
    }
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    {
        // SAFETY: The facade preserves the backend's non-returning entry contract.
        unsafe { VcpuContext::enter(context) }
    }
}

pub(crate) struct StoppedDetachFailure {
    #[cfg(CONFIG_ARCH_AARCH64)]
    backend: crate::arch::vm::StoppedDeactivationFailure,
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    never: core::convert::Infallible,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub(crate) enum StoppedDetachError {
    Unsupported,
    Backend(VcpuInterruptError),
}

impl StoppedDetachFailure {
    pub(crate) const fn error(&self) -> StoppedDetachError {
        #[cfg(CONFIG_ARCH_AARCH64)]
        {
            StoppedDetachError::Backend(self.backend.error())
        }
        #[cfg(not(CONFIG_ARCH_AARCH64))]
        {
            match self.never {}
        }
    }
}

/// Detaches hardware for an `AArch64` stopped run whose lower world is closed.
pub(crate) unsafe fn deactivate_stopped_hardware(
    state: &mut VcpuHardwareState,
    vcpu_id: u32,
    interrupts: &InterruptController,
    physical_count: u64,
    stopped: StoppedVcpuRun,
) -> Result<(), StoppedDetachFailure> {
    #[cfg(CONFIG_ARCH_AARCH64)]
    {
        // SAFETY: The caller preserves the stopped proof, exact context, and mask.
        unsafe {
            crate::arch::vm::deactivate_stopped_vcpu_hardware(
                &mut state.context,
                vcpu_id,
                interrupts,
                physical_count,
                stopped.backend,
            )
        }
        .map_err(|backend| StoppedDetachFailure { backend })
    }
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    {
        let _ = (state, vcpu_id, interrupts, physical_count);
        match stopped.never {}
    }
}

pub(crate) fn handle_virtual_timer_interrupt(
    state: &mut VcpuHardwareState,
    vcpu_id: u32,
    interrupts: &InterruptController,
) -> Result<bool, VcpuInterruptError> {
    crate::arch::vm::handle_virtual_timer_interrupt(&mut state.context, vcpu_id, interrupts)
}

pub(crate) fn handle_maintenance_interrupt(
    state: &mut VcpuHardwareState,
    vcpu_id: u32,
    interrupts: &InterruptController,
) -> Result<bool, VcpuInterruptError> {
    crate::arch::vm::handle_maintenance_interrupt(&mut state.context, vcpu_id, interrupts)
}

pub(crate) fn maintenance_interrupt_pending() -> bool {
    crate::arch::vm::maintenance_interrupt_pending()
}

/// Disables local virtual-interrupt delivery after ownership is lost.
pub(crate) fn quiesce_virtual_interrupt_delivery() {
    crate::arch::vm::quiesce_virtual_interrupt_delivery();
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct InterruptVirtualizationDescription {
    pub(crate) list_registers: u8,
    pub(crate) priority_bits: u8,
    pub(crate) preemption_bits: u8,
    pub(crate) interrupt_id_bits: u8,
}

pub(crate) fn interrupt_virtualization_description() -> Option<InterruptVirtualizationDescription> {
    crate::arch::vm::interrupt_virtualization_description().map(
        |(list_registers, priority_bits, preemption_bits, interrupt_id_bits)| {
            InterruptVirtualizationDescription {
                list_registers,
                priority_bits,
                preemption_bits,
                interrupt_id_bits,
            }
        },
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TimerValidationError {
    #[cfg(CONFIG_ARCH_AARCH64)]
    InterruptController(InterruptError),
    #[cfg(CONFIG_ARCH_AARCH64)]
    InvalidInterrupt,
    #[cfg(CONFIG_ARCH_AARCH64)]
    VirtualInterrupt(VirtualInterruptError),
}

/// Prepared admission to the selected backend's destructive timer probe.
///
/// Unsupported targets return `None` and cannot construct the private proof
/// required by injection or result inspection.
pub(crate) struct PreparedTimerValidation {
    capability: TimerValidationCapability,
    interrupts: InterruptController,
    hardware: VcpuHardwareState,
}

pub(crate) struct TimerValidationCapability {
    #[cfg(CONFIG_ARCH_AARCH64)]
    _private: (),
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    never: core::convert::Infallible,
}

impl PreparedTimerValidation {
    pub(crate) fn into_parts(
        self,
    ) -> (
        TimerValidationCapability,
        InterruptController,
        VcpuHardwareState,
    ) {
        (self.capability, self.interrupts, self.hardware)
    }
}

pub(crate) fn prepare_timer_validation(
    timer_interrupt: InterruptId,
    physical_count: u64,
    services: &ExitServicesReady,
    prepared: &PreparedInterruptVirtualization,
) -> Result<Option<PreparedTimerValidation>, TimerValidationError> {
    #[cfg(CONFIG_ARCH_AARCH64)]
    {
        let timer = hyper::vm::arm::gic::GicInterruptId::new(timer_interrupt.get())
            .ok_or(TimerValidationError::InvalidInterrupt)?;
        let interrupts = InterruptController::new(1, timer, usize::from(prepared.list_registers()))
            .map_err(TimerValidationError::InterruptController)?;
        let mut context = VcpuContext::new(0);
        context
            .initialize_virtual_interrupts()
            .map_err(TimerValidationError::VirtualInterrupt)?;
        context.set_virtual_count(physical_count, physical_count);
        context.set_virtual_timer_deadline(physical_count.wrapping_add(1_000_000));
        context.set_virtual_timer_enabled(true);
        Ok(Some(PreparedTimerValidation {
            capability: TimerValidationCapability { _private: () },
            interrupts,
            hardware: VcpuHardwareState::for_validation(context, services),
        }))
    }
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    {
        let _ = (timer_interrupt, physical_count, services, prepared);
        Ok(None)
    }
}

pub(crate) fn create_interrupt_controller(
    vcpu_count: u32,
    timer_interrupt: hyper::vm::interrupt::VirtualInterruptId,
) -> Result<InterruptController, InterruptError> {
    #[cfg(CONFIG_ARCH_AARCH64)]
    {
        let timer = hyper::vm::arm::gic::GicInterruptId::new(timer_interrupt.get())
            .ok_or(InterruptError::InvalidInterrupt)?;
        let description =
            interrupt_virtualization_description().ok_or(InterruptError::MissingCapabilities)?;
        InterruptController::new(vcpu_count, timer, usize::from(description.list_registers))
    }
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    {
        InterruptController::new(vcpu_count, timer_interrupt)
    }
}

pub(crate) fn inject_timer_for_validation(
    capability: &TimerValidationCapability,
    state: &mut VcpuHardwareState,
    vcpu_id: u32,
    interrupts: &InterruptController,
) -> Result<(), VcpuInterruptError> {
    #[cfg(CONFIG_ARCH_AARCH64)]
    {
        let _ = capability;
        crate::arch::vm::inject_timer_for_validation(&mut state.context, vcpu_id, interrupts)
    }
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    {
        let _ = (state, vcpu_id, interrupts);
        match capability.never {}
    }
}

pub(crate) fn timer_validation_succeeded(
    capability: &TimerValidationCapability,
    interrupts: &InterruptController,
) -> Result<bool, InterruptError> {
    #[cfg(CONFIG_ARCH_AARCH64)]
    {
        let _ = capability;
        let snapshot = interrupts
            .timer_snapshot(hyper::vm::interrupt::VirtualCpuId::new(0))
            .map_err(InterruptError::Vgic)?;
        Ok(snapshot.pending && snapshot.listed)
    }
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    {
        let _ = interrupts;
        match capability.never {}
    }
}

#[cfg(any(CONFIG_ARCH_AARCH64, CONFIG_ARCH_RISCV64))]
pub(crate) fn handle_guest_sync(
    state: &mut VcpuHardwareState,
    vcpu_id: u32,
    interrupts: &InterruptController,
    exit: GuestSyncExit,
) -> GuestSyncAction {
    crate::arch::vm::handle_guest_sync(&mut state.context, vcpu_id, interrupts, exit)
}

#[cfg(CONFIG_ARCH_AARCH64)]
pub(crate) fn update_guest_device_interrupt(
    state: &mut VcpuHardwareState,
    vcpu_id: u32,
    interrupts: &InterruptController,
    interrupt: hyper::vm::arm::gic::GicInterruptId,
    asserted: bool,
) -> Result<(), VcpuInterruptError> {
    crate::arch::vm::update_guest_device_interrupt(
        &mut state.context,
        vcpu_id,
        interrupts,
        interrupt,
        asserted,
    )
}

#[cfg(CONFIG_ARCH_AARCH64)]
pub(crate) fn update_saved_guest_device_interrupt(
    interrupts: &InterruptController,
    vcpu_id: u32,
    interrupt: hyper::vm::arm::gic::GicInterruptId,
    asserted: bool,
) -> Result<(), VcpuInterruptError> {
    crate::arch::vm::update_saved_guest_device_interrupt(interrupts, vcpu_id, interrupt, asserted)
}

pub(crate) fn reconcile_active_interrupts(
    state: &mut VcpuHardwareState,
    vcpu_id: u32,
    interrupts: &InterruptController,
) -> Result<(), ActiveInterruptReconcileError> {
    #[cfg(CONFIG_ARCH_AARCH64)]
    {
        crate::arch::vm::reconcile_active_interrupts(&mut state.context, vcpu_id, interrupts)
            .map_err(ActiveInterruptReconcileError::Backend)
    }
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    {
        let _ = (state, vcpu_id, interrupts);
        Err(ActiveInterruptReconcileError::Unsupported)
    }
}

/// Prompts the selected CPU to leave guest execution without publishing
/// scheduler policy. `false` explicitly reports that the selected backend has
/// no qualified targeted guest-exit mechanism; it does not consume the
/// caller's durable stop request.
pub(crate) fn request_guest_exit(cpu: hyper::cpu::CpuIndex) -> bool {
    #[cfg(CONFIG_ARCH_AARCH64)]
    {
        crate::arch::vm::request_guest_exit(cpu)
    }
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    {
        let _ = cpu;
        false
    }
}

#[cfg(CONFIG_ARCH_AARCH64)]
pub(crate) fn access_guest_gic(
    state: &mut VcpuHardwareState,
    vcpu_id: u32,
    interrupts: &InterruptController,
    access: hyper::vm::aarch64::device::gicv3::DecodedAccess,
    operation: hyper::vm::exit::MmioOperation,
) -> Result<Option<u64>, GicAccessError> {
    crate::arch::vm::access_guest_gic(&mut state.context, vcpu_id, interrupts, access, operation)
}

#[cfg(any(CONFIG_ARCH_X86_64, feature = "kernel-self-test"))]
pub(crate) fn guest_execution_available() -> bool {
    crate::arch::vm::guest_execution_available()
}

#[cfg(CONFIG_ARCH_X86_64)]
pub(crate) fn virtualization_backend_name() -> &'static str {
    crate::arch::vm::virtualization_backend_name()
}
