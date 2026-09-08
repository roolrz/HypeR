// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Safe Native virtual-machine construction and lifecycle bindings.

use core::num::NonZeroU64;

use crate::handle::{
    AnyObject, HandleRef, OwnedHandle, PendingVirtualMachineObject, ResourceDomainObject, Rights,
    TypedObject, VirtualCpuObject, VirtualMachineCreationAuthorityObject,
    VirtualMachineCreationLeaseObject, VirtualMachineObject, VirtualSerialObject, VmoObject,
};
use crate::{Error, Result, Status};

const CREATION_LEASE_RIGHTS: Rights = Rights::TRANSFER
    .union(Rights::INSPECT)
    .union(Rights::CREATE_VIRTUAL_MACHINE);
const PENDING_RIGHTS: Rights = Rights::TRANSFER
    .union(Rights::INSPECT)
    .union(Rights::WRITE)
    .union(Rights::START)
    .union(Rights::REQUEST_STOP);
const MACHINE_RIGHTS: Rights = Rights::TRANSFER
    .union(Rights::WAIT)
    .union(Rights::INSPECT)
    .union(Rights::REQUEST_STOP);
const VCPU_RIGHTS: Rights = Rights::TRANSFER
    .union(Rights::WAIT)
    .union(Rights::INSPECT)
    .union(Rights::START);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Architecture {
    Aarch64 = hyper_abi::HYPER_NATIVE_VIRTUAL_MACHINE_ARCHITECTURE_AARCH64 as u32,
    Riscv64 = hyper_abi::HYPER_NATIVE_VIRTUAL_MACHINE_ARCHITECTURE_RISCV64 as u32,
    X86_64 = hyper_abi::HYPER_NATIVE_VIRTUAL_MACHINE_ARCHITECTURE_X86_64 as u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum PlatformProfile {
    Aarch64Reference = hyper_abi::HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE as u32,
}

impl PlatformProfile {
    #[must_use]
    pub const fn guest_ram_base(self) -> u64 {
        match self {
            Self::Aarch64Reference => {
                hyper_abi::HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_GUEST_RAM_BASE
            }
        }
    }

    #[must_use]
    pub const fn device_tree_offset(self) -> u64 {
        match self {
            Self::Aarch64Reference => {
                hyper_abi::HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_DTB_OFFSET
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Configuration {
    pub guest_physical_base: u64,
    pub memory_size: u64,
    pub vcpu_count: u32,
    pub architecture: Architecture,
    pub platform_profile: PlatformProfile,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtualCpuBootstrap {
    pub entry: u64,
    pub stack: u64,
    pub arguments: [u64; 4],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VirtualMachinePhase {
    Installed,
    Running,
    Stopping,
    Stopped,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VirtualCpuPhase {
    Dormant,
    Started,
    Stopped,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VirtualCpuTermination {
    MemoryFault,
    Mmio,
    Synchronous,
    Administrative,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtualMachineInfo {
    pub phase: VirtualMachinePhase,
    pub vcpu_count: u32,
    pub guest_physical_base: u64,
    pub memory_size: u64,
    pub architecture: Architecture,
    pub platform_profile: PlatformProfile,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtualCpuInfo {
    pub id: u32,
    pub phase: VirtualCpuPhase,
    pub scheduler_thread_id: u64,
    pub terminal: Option<VirtualCpuTermination>,
}

/// A rejected consume-on-success VM operation and its unchanged input handle.
pub struct ConsumingFailure<T: crate::handle::ObjectType> {
    error: Error,
    handle: OwnedHandle<T>,
}

impl<T: crate::handle::ObjectType> ConsumingFailure<T> {
    #[must_use]
    pub const fn error(&self) -> Error {
        self.error
    }

    #[must_use]
    pub fn into_handle(self) -> OwnedHandle<T> {
        self.handle
    }
}

pub fn derive_creation_lease(
    authority: HandleRef<'_, VirtualMachineCreationAuthorityObject>,
    domain: HandleRef<'_, ResourceDomainObject>,
) -> Result<OwnedHandle<VirtualMachineCreationLeaseObject>> {
    let authority_raw = authority.raw();
    let domain_raw = domain.raw();
    // SAFETY: both raw handles are borrowed for the complete call; successful
    // output is adopted exactly once below.
    let result = unsafe {
        hyper_sys::virtual_machine_creation_lease_create(authority_raw.get(), domain_raw.get())
    };
    Status::from_raw(result.status).into_result()?;
    adopt(
        result.value0,
        CREATION_LEASE_RIGHTS,
        &[authority_raw, domain_raw],
    )
}

pub fn create(
    lease: OwnedHandle<VirtualMachineCreationLeaseObject>,
    configuration: Configuration,
) -> core::result::Result<
    OwnedHandle<PendingVirtualMachineObject>,
    ConsumingFailure<VirtualMachineCreationLeaseObject>,
> {
    let record = hyper_abi::HyperNativeVirtualMachineConfiguration {
        guest_physical_base: configuration.guest_physical_base,
        memory_size: configuration.memory_size,
        vcpu_count: configuration.vcpu_count,
        architecture: configuration.architecture as u32,
        platform_profile: configuration.platform_profile as u32,
        flags: 0,
    };
    let raw = lease.as_handle_ref().raw();
    // SAFETY: the stack record remains readable for the call and `lease`
    // remains owned until success is observed.
    let result = unsafe { hyper_sys::virtual_machine_create(raw.get(), &record) };
    let status = Status::from_raw(result.status);
    if status != Status::OK {
        return Err(ConsumingFailure {
            error: Error::Status(status),
            handle: lease,
        });
    }
    let _ = lease.into_raw();
    match adopt(result.value0, PENDING_RIGHTS, &[]) {
        Ok(pending) => Ok(pending),
        Err(_) => crate::handle::ownership_invariant(),
    }
}

pub fn set_memory(
    pending: HandleRef<'_, PendingVirtualMachineObject>,
    memory: HandleRef<'_, VmoObject>,
) -> Result<()> {
    // SAFETY: both handles remain borrowed for the complete call.
    Status::from_raw(unsafe {
        hyper_sys::pending_virtual_machine_set_memory(pending.raw().get(), memory.raw().get())
    })
    .into_result()
}

/// Transfers device-binding authority into the pending VM's serial device.
///
/// On failure, the returned value contains the unchanged virtual-serial handle.
pub fn set_virtual_serial(
    pending: HandleRef<'_, PendingVirtualMachineObject>,
    serial: OwnedHandle<VirtualSerialObject>,
) -> core::result::Result<(), ConsumingFailure<VirtualSerialObject>> {
    let raw = serial.as_handle_ref().raw();
    // SAFETY: both handles remain live for the complete call. Ownership of
    // `serial` is relinquished only after the kernel reports success.
    let status = Status::from_raw(unsafe {
        hyper_sys::pending_virtual_machine_set_virtual_serial(pending.raw().get(), raw.get())
    });
    if status != Status::OK {
        return Err(ConsumingFailure {
            error: Error::Status(status),
            handle: serial,
        });
    }
    let _ = serial.into_raw();
    Ok(())
}

pub fn set_bootstrap(
    pending: HandleRef<'_, PendingVirtualMachineObject>,
    bootstrap: VirtualCpuBootstrap,
) -> Result<()> {
    let record = hyper_abi::HyperNativeVirtualCpuBootstrap {
        entry: bootstrap.entry,
        stack: bootstrap.stack,
        argument0: bootstrap.arguments[0],
        argument1: bootstrap.arguments[1],
        argument2: bootstrap.arguments[2],
        argument3: bootstrap.arguments[3],
        vcpu_id: 0,
        flags: 0,
        reserved: 0,
    };
    // SAFETY: the handle and stack record remain valid for the complete call.
    Status::from_raw(unsafe {
        hyper_sys::pending_virtual_machine_set_bootstrap(pending.raw().get(), &record)
    })
    .into_result()
}

pub fn seal(pending: HandleRef<'_, PendingVirtualMachineObject>) -> Result<()> {
    // SAFETY: the handle remains borrowed for the complete call.
    Status::from_raw(unsafe { hyper_sys::pending_virtual_machine_seal(pending.raw().get()) })
        .into_result()
}

pub fn install(
    pending: OwnedHandle<PendingVirtualMachineObject>,
) -> core::result::Result<
    (
        OwnedHandle<VirtualMachineObject>,
        OwnedHandle<VirtualCpuObject>,
    ),
    ConsumingFailure<PendingVirtualMachineObject>,
> {
    let raw = pending.as_handle_ref().raw();
    // SAFETY: ownership is retained until the consume-on-success result is
    // observed, then both returned handles are adopted exactly once.
    let result = unsafe { hyper_sys::pending_virtual_machine_install(raw.get()) };
    let status = Status::from_raw(result.status);
    if status != Status::OK {
        return Err(ConsumingFailure {
            error: Error::Status(status),
            handle: pending,
        });
    }
    let _ = pending.into_raw();
    // SAFETY: an OK PENDING_VIRTUAL_MACHINE_INSTALL result transfers ownership
    // of every distinct nonzero output, including a malformed pair. The helper
    // closes malformed outputs before this committed operation fails closed.
    let (machine_owner, vcpu_owner) = match unsafe {
        crate::handle::adopt_produced_handle_pair([result.value0, result.value1])
    } {
        Ok(pair) => pair,
        Err(_) => crate::handle::ownership_invariant(),
    };
    let machine = match validate_adopted::<VirtualMachineObject>(machine_owner, MACHINE_RIGHTS) {
        Ok(machine) => machine,
        Err(_) => crate::handle::ownership_invariant(),
    };
    let vcpu = match validate_adopted::<VirtualCpuObject>(vcpu_owner, VCPU_RIGHTS) {
        Ok(vcpu) => vcpu,
        Err(_) => crate::handle::ownership_invariant(),
    };
    Ok((machine, vcpu))
}

/// Makes an installed dormant vCPU scheduler-runnable exactly once.
pub fn start_vcpu(vcpu: HandleRef<'_, VirtualCpuObject>) -> Result<()> {
    // SAFETY: the typed vCPU handle remains borrowed for the complete call.
    Status::from_raw(unsafe { hyper_sys::virtual_cpu_start(vcpu.raw().get()) }).into_result()
}

pub fn abort(
    pending: OwnedHandle<PendingVirtualMachineObject>,
) -> core::result::Result<(), ConsumingFailure<PendingVirtualMachineObject>> {
    let raw = pending.as_handle_ref().raw();
    // SAFETY: ownership is retained until success is observed.
    let status = Status::from_raw(unsafe { hyper_sys::pending_virtual_machine_abort(raw.get()) });
    if status != Status::OK {
        return Err(ConsumingFailure {
            error: Error::Status(status),
            handle: pending,
        });
    }
    let _ = pending.into_raw();
    Ok(())
}

pub fn request_stop(machine: HandleRef<'_, VirtualMachineObject>) -> Result<()> {
    // SAFETY: the handle remains borrowed for the complete call.
    Status::from_raw(unsafe { hyper_sys::virtual_machine_request_stop(machine.raw().get()) })
        .into_result()
}

/// Waits for acknowledged VM retirement after a stop request or owner loss.
pub fn wait_terminated(machine: HandleRef<'_, VirtualMachineObject>, deadline: u64) -> Result<()> {
    let expected = hyper_abi::HYPER_NATIVE_SIGNAL_VIRTUAL_MACHINE_TERMINATED;
    // SAFETY: the typed VM handle remains borrowed across this non-retaining
    // wait operation.
    let result = unsafe { hyper_sys::object_wait_one(machine.raw().get(), expected, deadline) };
    Status::from_raw(result.status).into_result()?;
    if result.value0 & expected == expected {
        Ok(())
    } else {
        Err(Error::InvalidResponse)
    }
}

/// Waits until one vCPU has completed scheduler reaping and its terminal
/// reason is stable for inspection.
pub fn wait_vcpu_terminated(vcpu: HandleRef<'_, VirtualCpuObject>, deadline: u64) -> Result<()> {
    let expected = hyper_abi::HYPER_NATIVE_SIGNAL_VIRTUAL_CPU_TERMINATED;
    // SAFETY: the typed vCPU handle remains borrowed across this non-retaining
    // wait operation.
    let result = unsafe { hyper_sys::object_wait_one(vcpu.raw().get(), expected, deadline) };
    Status::from_raw(result.status).into_result()?;
    if result.value0 & expected == expected {
        Ok(())
    } else {
        Err(Error::InvalidResponse)
    }
}

pub fn machine_info(machine: HandleRef<'_, VirtualMachineObject>) -> Result<VirtualMachineInfo> {
    let mut record = hyper_abi::HyperNativeVirtualMachineInfo {
        phase: 0,
        vcpu_count: 0,
        guest_physical_base: 0,
        memory_size: 0,
        architecture: 0,
        platform_profile: 0,
    };
    // SAFETY: the handle remains borrowed and the output record writable.
    let result = unsafe { hyper_sys::virtual_machine_get_info(machine.raw().get(), &mut record) };
    let _supported_size = crate::validate_info_result(
        result,
        hyper_abi::HYPER_NATIVE_VIRTUAL_MACHINE_INFO_MIN_SIZE,
    )?;
    Ok(VirtualMachineInfo {
        phase: decode_machine_phase(record.phase)?,
        vcpu_count: record.vcpu_count,
        guest_physical_base: record.guest_physical_base,
        memory_size: record.memory_size,
        architecture: decode_architecture(record.architecture)?,
        platform_profile: decode_platform_profile(record.platform_profile)?,
    })
}

pub fn vcpu_info(vcpu: HandleRef<'_, VirtualCpuObject>) -> Result<VirtualCpuInfo> {
    let mut record = hyper_abi::HyperNativeVirtualCpuInfo {
        vcpu_id: 0,
        phase: 0,
        scheduler_thread_id: 0,
        terminal_reason: 0,
        reserved: 0,
    };
    // SAFETY: the handle remains borrowed and the output record writable.
    let result = unsafe { hyper_sys::virtual_cpu_get_info(vcpu.raw().get(), &mut record) };
    let _supported_size =
        crate::validate_info_result(result, hyper_abi::HYPER_NATIVE_VIRTUAL_CPU_INFO_MIN_SIZE)?;
    if record.reserved != 0 {
        return Err(Error::InvalidResponse);
    }
    let phase = decode_vcpu_phase(record.phase)?;
    let terminal = decode_vcpu_terminal(record.terminal_reason)?;
    if (phase == VirtualCpuPhase::Stopped) != terminal.is_some() {
        return Err(Error::InvalidResponse);
    }
    Ok(VirtualCpuInfo {
        id: record.vcpu_id,
        phase,
        scheduler_thread_id: record.scheduler_thread_id,
        terminal,
    })
}

fn decode_architecture(raw: u32) -> Result<Architecture> {
    match raw {
        value if value == Architecture::Aarch64 as u32 => Ok(Architecture::Aarch64),
        value if value == Architecture::Riscv64 as u32 => Ok(Architecture::Riscv64),
        value if value == Architecture::X86_64 as u32 => Ok(Architecture::X86_64),
        _ => Err(Error::InvalidResponse),
    }
}

fn decode_platform_profile(raw: u32) -> Result<PlatformProfile> {
    match raw {
        value if value == PlatformProfile::Aarch64Reference as u32 => {
            Ok(PlatformProfile::Aarch64Reference)
        }
        _ => Err(Error::InvalidResponse),
    }
}

fn decode_machine_phase(raw: u32) -> Result<VirtualMachinePhase> {
    match u64::from(raw) {
        hyper_abi::HYPER_NATIVE_VIRTUAL_MACHINE_PHASE_INSTALLED => {
            Ok(VirtualMachinePhase::Installed)
        }
        hyper_abi::HYPER_NATIVE_VIRTUAL_MACHINE_PHASE_RUNNING => Ok(VirtualMachinePhase::Running),
        hyper_abi::HYPER_NATIVE_VIRTUAL_MACHINE_PHASE_STOPPING => Ok(VirtualMachinePhase::Stopping),
        hyper_abi::HYPER_NATIVE_VIRTUAL_MACHINE_PHASE_STOPPED => Ok(VirtualMachinePhase::Stopped),
        _ => Err(Error::InvalidResponse),
    }
}

fn decode_vcpu_phase(raw: u32) -> Result<VirtualCpuPhase> {
    match u64::from(raw) {
        hyper_abi::HYPER_NATIVE_VIRTUAL_CPU_PHASE_DORMANT => Ok(VirtualCpuPhase::Dormant),
        hyper_abi::HYPER_NATIVE_VIRTUAL_CPU_PHASE_STARTED => Ok(VirtualCpuPhase::Started),
        hyper_abi::HYPER_NATIVE_VIRTUAL_CPU_PHASE_STOPPED => Ok(VirtualCpuPhase::Stopped),
        _ => Err(Error::InvalidResponse),
    }
}

fn decode_vcpu_terminal(raw: u32) -> Result<Option<VirtualCpuTermination>> {
    match u64::from(raw) {
        hyper_abi::HYPER_NATIVE_VIRTUAL_CPU_TERMINAL_NONE => Ok(None),
        hyper_abi::HYPER_NATIVE_VIRTUAL_CPU_TERMINAL_MEMORY_FAULT => {
            Ok(Some(VirtualCpuTermination::MemoryFault))
        }
        hyper_abi::HYPER_NATIVE_VIRTUAL_CPU_TERMINAL_MMIO => Ok(Some(VirtualCpuTermination::Mmio)),
        hyper_abi::HYPER_NATIVE_VIRTUAL_CPU_TERMINAL_SYNCHRONOUS => {
            Ok(Some(VirtualCpuTermination::Synchronous))
        }
        hyper_abi::HYPER_NATIVE_VIRTUAL_CPU_TERMINAL_ADMINISTRATIVE => {
            Ok(Some(VirtualCpuTermination::Administrative))
        }
        _ => Err(Error::InvalidResponse),
    }
}

fn adopt<T: TypedObject>(
    raw: u64,
    rights: Rights,
    live_inputs: &[NonZeroU64],
) -> Result<OwnedHandle<T>> {
    // SAFETY: every caller invokes this exactly once for a successful syscall
    // result; malformed aliases of retained borrows are rejected first.
    let owner =
        unsafe { crate::handle::adopt_produced_handle_excluding::<AnyObject>(raw, live_inputs)? };
    validate_adopted(owner, rights)
}

fn validate_adopted<T: TypedObject>(
    owner: OwnedHandle<AnyObject>,
    rights: Rights,
) -> Result<OwnedHandle<T>> {
    let info = owner.info()?;
    if info.rights != rights {
        return Err(Error::InvalidResponse);
    }
    owner.downcast::<T>().map_err(|failure| failure.error())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtual_machine_architectures_decode_to_typed_values() {
        assert_eq!(
            decode_architecture(
                hyper_abi::HYPER_NATIVE_VIRTUAL_MACHINE_ARCHITECTURE_AARCH64 as u32
            ),
            Ok(Architecture::Aarch64)
        );
        assert_eq!(
            decode_architecture(
                hyper_abi::HYPER_NATIVE_VIRTUAL_MACHINE_ARCHITECTURE_RISCV64 as u32
            ),
            Ok(Architecture::Riscv64)
        );
        assert_eq!(
            decode_architecture(hyper_abi::HYPER_NATIVE_VIRTUAL_MACHINE_ARCHITECTURE_X86_64 as u32),
            Ok(Architecture::X86_64)
        );
        assert_eq!(decode_architecture(u32::MAX), Err(Error::InvalidResponse));
    }

    #[test]
    fn virtual_machine_platform_profiles_decode_to_typed_values() {
        assert_eq!(
            decode_platform_profile(
                hyper_abi::HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE as u32,
            ),
            Ok(PlatformProfile::Aarch64Reference)
        );
        assert_eq!(
            decode_platform_profile(u32::MAX),
            Err(Error::InvalidResponse)
        );
    }
}
