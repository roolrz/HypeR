// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Selected native-user machine capability.
//!
//! Process, syscall, rights, and mapping ownership remain kernel policy. The
//! facade publishes address-space limits, root construction and activation,
//! and the narrow external-memory copy mechanism used for machine-visible
//! user pages. Kernel-owned tokens retain policy, residency, and lifetime.

use core::marker::PhantomData;

use hyper::abi::native::{NativeInvocation, NativeResult};
use hyper::cpu::{CpuIndex, PinnedExecution};
use hyper::hal::user::{UserFault, UserRunBinding};
use hyper::mm::PhysicalAddress;
use hyper::sync::InterruptMaskGuard;

#[cfg(CONFIG_ARCH_AARCH64)]
pub(crate) use crate::arch::user::UserMachineContractError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ExposedCopyError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AddressSpaceError {
    InvalidCpu,
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    Unsupported,
    #[cfg(CONFIG_ARCH_AARCH64)]
    Backend(crate::arch::user::UserAddressSpaceError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UserEntryError {
    InterruptsEnabled,
    Unsupported,
    #[cfg(CONFIG_ARCH_AARCH64)]
    Backend(crate::arch::user::UserEntryError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TranslationKind {
    VheHostStage1,
    NvheStage2Only,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostMachine {
    Aarch64,
    Riscv64,
    X86_64,
}

pub(crate) const fn host_machine() -> HostMachine {
    #[cfg(CONFIG_ARCH_AARCH64)]
    {
        HostMachine::Aarch64
    }
    #[cfg(CONFIG_ARCH_RISCV64)]
    {
        HostMachine::Riscv64
    }
    #[cfg(CONFIG_ARCH_X86_64)]
    {
        HostMachine::X86_64
    }
}

#[cfg(CONFIG_ARCH_AARCH64)]
pub(crate) fn translation_kind() -> Result<TranslationKind, AddressSpaceError> {
    Ok(if crate::arch::user::user_uses_vhe_translation() {
        TranslationKind::VheHostStage1
    } else {
        TranslationKind::NvheStage2Only
    })
}

#[derive(Clone, Copy)]
#[cfg_attr(not(CONFIG_ARCH_AARCH64), allow(dead_code))]
pub(crate) struct MappingPage {
    pub(crate) address: u64,
    pub(crate) physical: PhysicalAddress,
    pub(crate) readable: bool,
    pub(crate) writable: bool,
    pub(crate) executable: bool,
}

/// Opaque, inert hierarchy prepared before kernel mapping publication.
pub(crate) struct PreparedAddressSpace {
    #[cfg(CONFIG_ARCH_AARCH64)]
    backend: crate::arch::user::PreparedUserAddressSpace,
}

#[derive(Clone, Copy)]
pub(crate) struct LocalIdentity {
    #[cfg(CONFIG_ARCH_AARCH64)]
    backend: crate::arch::user::UserLocalIdentity,
}

#[derive(Clone, Copy)]
pub(crate) struct LocalRequest {
    #[cfg(CONFIG_ARCH_AARCH64)]
    backend: crate::arch::user::UserLocalRequest,
}

/// CPU-affine proof that privileged access isolation was established while
/// the final native-entry interrupt mask remains owned by the caller.
#[must_use = "kernel-access preparation must be consumed by native activation"]
pub(crate) struct PreparedKernelAccess<'mask> {
    cpu: CpuIndex,
    _mask: &'mask InterruptMaskGuard<crate::hal::irq::LocalMask>,
}

impl PreparedAddressSpace {
    pub(crate) fn local_identity(&self) -> LocalIdentity {
        #[cfg(CONFIG_ARCH_AARCH64)]
        {
            LocalIdentity {
                backend: self.backend.local_identity(),
            }
        }
        #[cfg(not(CONFIG_ARCH_AARCH64))]
        {
            LocalIdentity {}
        }
    }

    /// Encodes a root replacement for immediate acknowledged delivery.
    ///
    /// # Safety
    ///
    /// The prepared root and its identifier owner must remain retained until
    /// every target has acknowledged servicing the returned request.
    pub(crate) unsafe fn replace_request(&self) -> LocalRequest {
        #[cfg(CONFIG_ARCH_AARCH64)]
        {
            LocalRequest {
                backend: self
                    .backend
                    .local_request(crate::arch::user::UserLocalOperation::Replace),
            }
        }
        #[cfg(not(CONFIG_ARCH_AARCH64))]
        {
            LocalRequest {}
        }
    }

    /// Encodes a tagged invalidation for immediate acknowledged delivery.
    ///
    /// # Safety
    ///
    /// The prepared root and its identifier owner must remain retained until
    /// every target has acknowledged servicing the returned request.
    pub(crate) unsafe fn invalidate_request(&self) -> LocalRequest {
        #[cfg(CONFIG_ARCH_AARCH64)]
        {
            LocalRequest {
                backend: self
                    .backend
                    .local_request(crate::arch::user::UserLocalOperation::Invalidate),
            }
        }
        #[cfg(not(CONFIG_ARCH_AARCH64))]
        {
            LocalRequest {}
        }
    }
}

/// Applies a value-only request retained by the acknowledged RPC mailbox.
pub(crate) unsafe fn service_local_request(request: LocalRequest) -> bool {
    #[cfg(CONFIG_ARCH_AARCH64)]
    {
        // SAFETY: The closed kernel RPC keeps the root/ID owner alive until
        // this target acknowledges and closes address-space admission.
        // SAFETY: The caller retains the hierarchy and identifier until this
        // target acknowledges completion.
        unsafe { crate::arch::user::service_user_local_request(request.backend) }.is_ok()
    }
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    {
        let _ = request;
        false
    }
}

/// CPU-affine proof that one admitted local context installed a native root.
#[must_use = "active native translation must be explicitly left before releasing CPU pinning"]
pub(crate) struct ActiveAddressSpace<'pin> {
    cpu: CpuIndex,
    #[cfg(CONFIG_ARCH_AARCH64)]
    backend: Option<crate::arch::user::UserLocalActivation>,
    _pin: PhantomData<&'pin dyn PinnedExecution>,
    _owner: PhantomData<&'pin dyn hyper::hal::user::UserTranslationOwner>,
    not_send_or_sync: PhantomData<*mut ()>,
}

impl ActiveAddressSpace<'_> {
    pub(crate) const fn cpu(&self) -> CpuIndex {
        self.cpu
    }
}

/// Builds a VHE host stage-1 root from a complete immutable mapping snapshot.
///
/// # Safety
///
/// Allocator results must be new, zeroed, linearly mapped blocks of the
/// requested order and remain retained through acknowledged root retirement.
pub(crate) unsafe fn prepare_vhe_address_space(
    asid: u16,
    generation: u64,
    mut enumerate: impl FnMut(&mut dyn FnMut(MappingPage)),
    allocator: &mut impl FnMut(usize) -> Option<PhysicalAddress>,
) -> Result<PreparedAddressSpace, AddressSpaceError> {
    #[cfg(CONFIG_ARCH_AARCH64)]
    {
        let mut arch_enumerate = |visit: &mut dyn FnMut(crate::arch::user::UserMappingPage)| {
            enumerate(&mut |page| {
                visit(crate::arch::user::UserMappingPage {
                    address: page.address,
                    physical: page.physical,
                    readable: page.readable,
                    writable: page.writable,
                    executable: page.executable,
                });
            });
        };
        // SAFETY: The facade forwards the table ownership contract unchanged.
        let backend = unsafe {
            crate::arch::user::prepare_vhe_user_address_space(
                asid,
                generation,
                &mut arch_enumerate,
                allocator,
            )
        }
        .map_err(AddressSpaceError::Backend)?;
        Ok(PreparedAddressSpace { backend })
    }
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    {
        let _ = (asid, generation, &mut enumerate, allocator);
        Err(AddressSpaceError::Unsupported)
    }
}

/// Builds an nVHE private stage-2 root. The safety contract matches the VHE
/// builder, while the caller must supply a VMID from the shared guest/native
/// namespace.
pub(crate) unsafe fn prepare_nvhe_address_space(
    vmid: u16,
    generation: u64,
    mut enumerate: impl FnMut(&mut dyn FnMut(MappingPage)),
    allocator: &mut impl FnMut(usize) -> Option<PhysicalAddress>,
) -> Result<PreparedAddressSpace, AddressSpaceError> {
    #[cfg(CONFIG_ARCH_AARCH64)]
    {
        let mut arch_enumerate = |visit: &mut dyn FnMut(crate::arch::user::UserMappingPage)| {
            enumerate(&mut |page| {
                visit(crate::arch::user::UserMappingPage {
                    address: page.address,
                    physical: page.physical,
                    readable: page.readable,
                    writable: page.writable,
                    executable: page.executable,
                });
            });
        };
        // SAFETY: The facade forwards the same table ownership contract.
        let backend = unsafe {
            crate::arch::user::prepare_nvhe_user_address_space(
                vmid,
                generation,
                &mut arch_enumerate,
                allocator,
            )
        }
        .map_err(AddressSpaceError::Backend)?;
        Ok(PreparedAddressSpace { backend })
    }
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    {
        let _ = (vmid, generation, &mut enumerate, allocator);
        Err(AddressSpaceError::Unsupported)
    }
}

/// Installs a prepared root on the pinned current CPU.
///
/// # Safety
///
/// Kernel admission must remain closed against update cuts until this method
/// returns and the returned token is recorded active. The hierarchy and its
/// identifier must remain retained for the token's lifetime.
pub(crate) unsafe fn activate_local<'pin>(
    root: &PreparedAddressSpace,
    cpu: CpuIndex,
    _pin: &'pin dyn PinnedExecution,
    _owner: &'pin dyn hyper::hal::user::UserTranslationOwner,
    kernel_access: &PreparedKernelAccess<'_>,
) -> Result<ActiveAddressSpace<'pin>, AddressSpaceError> {
    if kernel_access.cpu != cpu
        || crate::hal::cpu::current_index() != Some(cpu)
        || crate::hal::irq::local_enabled()
    {
        return Err(AddressSpaceError::InvalidCpu);
    }
    #[cfg(CONFIG_ARCH_AARCH64)]
    {
        // SAFETY: The caller supplies the admission, pinning, and retention
        // proof required by the architecture-local transition.
        let backend = unsafe { crate::arch::user::activate_user_local(&root.backend) };
        Ok(ActiveAddressSpace {
            cpu,
            backend: Some(backend),
            _pin: PhantomData,
            _owner: PhantomData,
            not_send_or_sync: PhantomData,
        })
    }
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    {
        let _ = (root, cpu, kernel_access);
        Err(AddressSpaceError::Unsupported)
    }
}

pub(crate) fn local_identity_is_active(identity: LocalIdentity) -> bool {
    #[cfg(CONFIG_ARCH_AARCH64)]
    {
        crate::arch::user::user_local_identity_is_active(identity.backend)
    }
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    {
        let _ = identity;
        false
    }
}

/// Restores the translation context which preceded native activation.
///
/// # Safety
///
/// The caller must execute on `active.cpu()` while the original execution pin
/// remains valid.
pub(crate) unsafe fn deactivate_local(
    active: ActiveAddressSpace<'_>,
) -> Result<(), AddressSpaceError> {
    #[cfg(CONFIG_ARCH_AARCH64)]
    {
        let mut active = active;
        let Some(backend) = active.backend.take() else {
            crate::hal::cpu::halt();
        };
        // SAFETY: The caller proves this is the PE which installed the
        // consumed non-Send translation token.
        unsafe { crate::arch::user::deactivate_user_local(backend) };
        Ok(())
    }
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    {
        let _ = active;
        Err(AddressSpaceError::Unsupported)
    }
}

impl Drop for ActiveAddressSpace<'_> {
    fn drop(&mut self) {
        #[cfg(CONFIG_ARCH_AARCH64)]
        if self.backend.is_some() {
            // Hardware still references a borrowed root. Returning would end
            // the root and pin borrows and permit use-after-free.
            crate::hal::cpu::halt();
        }
    }
}

/// Opaque native-user register owner attached to one kernel `UserThread`.
pub(crate) struct UserContext {
    #[cfg(CONFIG_ARCH_AARCH64)]
    backend: crate::arch::user::UserContext,
}

/// Creates a stopped native-user context using the selected machine contract.
pub(crate) fn prepare_context(
    entry: u64,
    stack: u64,
    tls: u64,
) -> Result<UserContext, UserEntryError> {
    #[cfg(CONFIG_ARCH_AARCH64)]
    {
        let address_limit = crate::arch::user::user_address_limit()
            .map_err(crate::arch::user::UserEntryError::from)
            .map_err(UserEntryError::Backend)?;
        let backend = crate::arch::user::UserContext::try_new(entry, stack, tls, address_limit)
            .map_err(UserEntryError::Backend)?;
        Ok(UserContext { backend })
    }
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    {
        let _ = (entry, stack, tls);
        Err(UserEntryError::Unsupported)
    }
}

/// Asserts architecture protection required before privileged Rust can run
/// with a user-bearing translation root installed.
pub(crate) fn prepare_kernel_access<'mask>(
    mask: &'mask InterruptMaskGuard<crate::hal::irq::LocalMask>,
) -> Result<PreparedKernelAccess<'mask>, UserEntryError> {
    if crate::hal::irq::local_enabled() {
        return Err(UserEntryError::InterruptsEnabled);
    }
    let cpu = crate::hal::cpu::current_index().ok_or(UserEntryError::Unsupported)?;
    #[cfg(CONFIG_ARCH_AARCH64)]
    {
        crate::arch::user::assert_kernel_pan()
            .map_err(crate::arch::user::UserEntryError::from)
            .map_err(UserEntryError::Backend)?;
        Ok(PreparedKernelAccess { cpu, _mask: mask })
    }
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    {
        let _ = (cpu, mask);
        Err(UserEntryError::Unsupported)
    }
}

/// A stopped lower-EL run which still owns its active translation token.
///
/// Kernel policy cannot dispatch, block, or schedule while this value exists.
/// The kernel address-space owner must consume this token and restore its
/// preceding translation before exposing the exit to policy code.
#[must_use = "native-user translation must be left before kernel dispatch"]
pub(crate) struct StoppedUser<'context, 'pin> {
    #[cfg(CONFIG_ARCH_AARCH64)]
    exit: Option<crate::arch::user::UserExit<'context>>,
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    _context: PhantomData<&'context mut UserContext>,
    active: Option<ActiveAddressSpace<'pin>>,
}

/// Runs one admitted native-user generation as a call-like machine operation.
///
/// The active translation token retains both execution pinning and root
/// ownership. A selected exception copies the raw frame into `context` and
/// returns to this call rather than dispatching from the vector stack.
pub(crate) fn run_user<'context, 'pin>(
    context: &'context mut UserContext,
    active: ActiveAddressSpace<'pin>,
    binding: UserRunBinding,
    kernel_access: PreparedKernelAccess<'_>,
) -> Result<StoppedUser<'context, 'pin>, UserEntryError> {
    if kernel_access.cpu != active.cpu || crate::hal::irq::local_enabled() {
        // SAFETY: `active` still owns the current-CPU pin and translation.
        if unsafe { deactivate_local(active) }.is_err() {
            crate::hal::cpu::halt();
        }
        return Err(UserEntryError::InterruptsEnabled);
    }
    #[cfg(CONFIG_ARCH_AARCH64)]
    {
        // SAFETY: `active` retains the current-CPU pin, translation root, and
        // owner for this entire call. `context` is exclusively borrowed until
        // the architecture has closed its generation-qualified publication.
        let exit = match unsafe { crate::arch::user::run_user(&mut context.backend, binding) } {
            Ok(exit) => exit,
            Err(error) => {
                // SAFETY: The consumed token proves the current CPU remains
                // pinned to the activation which this path is abandoning.
                if unsafe { deactivate_local(active) }.is_err() {
                    crate::hal::cpu::halt();
                }
                return Err(UserEntryError::Backend(error));
            }
        };
        Ok(StoppedUser {
            exit: Some(exit),
            active: Some(active),
        })
    }
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    {
        let _ = (context, active, binding, kernel_access);
        Err(UserEntryError::Unsupported)
    }
}

impl<'context, 'pin> StoppedUser<'context, 'pin> {
    pub(crate) fn release(
        mut self,
    ) -> (
        UserExit<'context>,
        ActiveAddressSpace<'pin>,
        StoppedPublication,
    ) {
        let Some(active) = self.active.take() else {
            crate::hal::cpu::halt();
        };
        #[cfg(CONFIG_ARCH_AARCH64)]
        {
            let Some(exit) = self.exit.take() else {
                crate::hal::cpu::halt();
            };
            let exit = UserExit::from_arch(exit);
            let stopped = StoppedPublication {
                binding: exit.binding(),
            };
            (exit, active, stopped)
        }
        #[cfg(not(CONFIG_ARCH_AARCH64))]
        {
            let _ = active;
            crate::hal::cpu::halt()
        }
    }
}

/// Non-forgeable proof that architecture publication for one native-user
/// generation is closed.
pub(crate) struct StoppedPublication {
    binding: UserRunBinding,
}

impl StoppedPublication {
    pub(crate) const fn binding(&self) -> UserRunBinding {
        self.binding
    }
}

impl Drop for StoppedUser<'_, '_> {
    fn drop(&mut self) {
        if self.active.is_some() {
            // Leaking a borrowed hardware root would permit both migration and
            // hierarchy retirement while the CPU still names it.
            crate::hal::cpu::halt();
        }
    }
}

#[cfg_attr(not(CONFIG_ARCH_AARCH64), allow(dead_code))]
pub(crate) enum UserExit<'context> {
    NativeCall {
        invocation: NativeInvocation,
        completion: ReturnCapability<'context>,
    },
    Fault {
        fault: UserFault,
        completion: ReturnCapability<'context>,
    },
    Interrupted {
        completion: ReturnCapability<'context>,
    },
}

#[cfg_attr(not(CONFIG_ARCH_AARCH64), allow(dead_code))]
impl<'context> UserExit<'context> {
    pub(crate) fn binding(&self) -> UserRunBinding {
        match self {
            Self::NativeCall { completion, .. }
            | Self::Fault { completion, .. }
            | Self::Interrupted { completion } => completion.binding(),
        }
    }

    #[cfg(CONFIG_ARCH_AARCH64)]
    fn from_arch(exit: crate::arch::user::UserExit<'context>) -> Self {
        match exit {
            crate::arch::user::UserExit::NativeCall {
                invocation,
                completion,
            } => Self::NativeCall {
                invocation,
                completion: ReturnCapability {
                    backend: completion,
                },
            },
            crate::arch::user::UserExit::Fault { fault, completion } => Self::Fault {
                fault,
                completion: ReturnCapability {
                    backend: completion,
                },
            },
            crate::arch::user::UserExit::Interrupted { completion } => Self::Interrupted {
                completion: ReturnCapability {
                    backend: completion,
                },
            },
        }
    }
}

#[must_use = "native-user return ownership must be resumed or discarded exactly once"]
pub(crate) struct ReturnCapability<'context> {
    #[cfg(CONFIG_ARCH_AARCH64)]
    backend: crate::arch::user::UserReturnCapability<'context>,
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    _context: PhantomData<&'context mut UserContext>,
}

#[cfg_attr(not(CONFIG_ARCH_AARCH64), allow(dead_code))]
impl<'context> ReturnCapability<'context> {
    pub(crate) fn binding(&self) -> UserRunBinding {
        #[cfg(CONFIG_ARCH_AARCH64)]
        {
            self.backend.binding()
        }
        #[cfg(not(CONFIG_ARCH_AARCH64))]
        {
            crate::hal::cpu::halt()
        }
    }

    pub(crate) fn complete_native(
        self,
        expected: UserRunBinding,
        result: NativeResult,
    ) -> Result<(), CompletionFailure<'context>> {
        #[cfg(CONFIG_ARCH_AARCH64)]
        {
            self.backend
                .complete_native(expected, result)
                .map_err(CompletionFailure::from_arch)
        }
        #[cfg(not(CONFIG_ARCH_AARCH64))]
        {
            let _ = (self, expected, result);
            crate::hal::cpu::halt()
        }
    }

    pub(crate) fn resume_interrupted(
        self,
        expected: UserRunBinding,
    ) -> Result<(), CompletionFailure<'context>> {
        #[cfg(CONFIG_ARCH_AARCH64)]
        {
            self.backend
                .resume_interrupted(expected)
                .map_err(CompletionFailure::from_arch)
        }
        #[cfg(not(CONFIG_ARCH_AARCH64))]
        {
            let _ = (self, expected);
            crate::hal::cpu::halt()
        }
    }

    pub(crate) fn discard(
        self,
        expected: UserRunBinding,
    ) -> Result<(), CompletionFailure<'context>> {
        #[cfg(CONFIG_ARCH_AARCH64)]
        {
            self.backend
                .discard(expected)
                .map_err(CompletionFailure::from_arch)
        }
        #[cfg(not(CONFIG_ARCH_AARCH64))]
        {
            let _ = (self, expected);
            crate::hal::cpu::halt()
        }
    }
}

#[must_use = "completion failure retains native-user return ownership"]
pub(crate) struct CompletionFailure<'context> {
    error: UserEntryError,
    completion: ReturnCapability<'context>,
}

impl<'context> CompletionFailure<'context> {
    #[cfg(CONFIG_ARCH_AARCH64)]
    fn from_arch(failure: crate::arch::user::UserCompletionFailure<'context>) -> Self {
        let (error, completion) = failure.into_parts();
        Self {
            error: UserEntryError::Backend(error),
            completion: ReturnCapability {
                backend: completion,
            },
        }
    }

    pub(crate) fn into_parts(self) -> (UserEntryError, ReturnCapability<'context>) {
        (self.error, self.completion)
    }
}

#[cfg(CONFIG_ARCH_AARCH64)]
pub(crate) fn address_limit() -> Result<u64, UserMachineContractError> {
    crate::arch::user::user_address_limit()
}

/// Copies from machine-visible memory using the selected architecture seam.
///
/// # Safety
///
/// The ranges must be resident, valid for `length`, and non-overlapping. The
/// operation is non-faulting by construction of the retained page mapping.
pub(crate) unsafe fn copy_from_exposed(
    source: *const u8,
    destination: *mut u8,
    length: usize,
) -> Result<(), ExposedCopyError> {
    #[cfg(CONFIG_ARCH_AARCH64)]
    {
        // SAFETY: The caller provides the contract forwarded unchanged to the
        // selected AArch64 external-memory implementation.
        unsafe { crate::arch::user::copy_from_exposed(source, destination, length) };
        Ok(())
    }
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    {
        let _ = (source, destination, length);
        Err(ExposedCopyError)
    }
}

/// Copies to machine-visible memory using the selected architecture seam.
///
/// # Safety
///
/// The same requirements as [`copy_from_exposed`] apply with reversed data
/// direction.
pub(crate) unsafe fn copy_to_exposed(
    source: *const u8,
    destination: *mut u8,
    length: usize,
) -> Result<(), ExposedCopyError> {
    #[cfg(CONFIG_ARCH_AARCH64)]
    {
        // SAFETY: The caller provides the forwarded external-memory contract.
        unsafe { crate::arch::user::copy_to_exposed(source, destination, length) };
        Ok(())
    }
    #[cfg(not(CONFIG_ARCH_AARCH64))]
    {
        let _ = (source, destination, length);
        Err(ExposedCopyError)
    }
}
