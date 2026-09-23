// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Immutable execution identity selected by a trusted image loader.

use core::num::NonZeroU64;

use crate::kernel::mm::user_space::UserAddress;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MachineAbi {
    Aarch64,
    Riscv64,
    #[expect(dead_code, reason = "x86 Native image loading is not implemented")]
    X86_64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AbiFamily {
    Native,
    #[expect(
        dead_code,
        reason = "Reserved image-routing vocabulary; foreign loaders are not implemented"
    )]
    Linux,
    #[expect(
        dead_code,
        reason = "Reserved image-routing vocabulary; foreign loaders are not implemented"
    )]
    FreeBsd,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExecutionRoute {
    NativeKernel,
    /// The session is a stable diagnostic key, not authority.
    #[expect(
        dead_code,
        reason = "Reserved image-routing vocabulary; foreign loaders are not implemented"
    )]
    Supervised {
        session: SupervisionSessionId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SupervisionSessionId(NonZeroU64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ImageError {
    InvalidEntry,
    InvalidStack,
    InvalidRoute,
    UnsupportedRevision,
}

/// Initial register values for one native user Thread.
///
/// A Process image supplies the first Thread's defaults. Additional Threads
/// must provide distinct stack/TLS values explicitly; silently reusing the
/// image stack would create overlapping execution state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct UserThreadStart {
    argument: u64,
    entry: UserAddress,
    stack: UserAddress,
    tls: UserAddress,
}

impl UserThreadStart {
    pub(crate) fn try_new(
        entry: UserAddress,
        stack: UserAddress,
        tls: UserAddress,
    ) -> Result<Self, ImageError> {
        if entry.get() == 0 {
            return Err(ImageError::InvalidEntry);
        }
        if stack.get() == 0 || stack.get() & 0xf != 0 {
            return Err(ImageError::InvalidStack);
        }
        Ok(Self {
            entry,
            stack,
            tls,
            argument: 0,
        })
    }

    pub(crate) const fn with_argument(mut self, argument: u64) -> Self {
        self.argument = argument;
        self
    }
    pub(crate) const fn argument(self) -> u64 {
        self.argument
    }

    pub(crate) const fn entry(self) -> UserAddress {
        self.entry
    }

    pub(crate) const fn stack(self) -> UserAddress {
        self.stack
    }

    pub(crate) const fn tls(self) -> UserAddress {
        self.tls
    }
}

/// Fixed execution identity for one installed image generation.
///
/// Mutable registers and mappings belong to `UserThread` and `NativeAddressSpace`,
/// respectively. Keeping route selection immutable prevents syscall semantics
/// from changing beneath a running thread.
pub(crate) struct ProcessImage {
    machine: MachineAbi,
    family: AbiFamily,
    route: ExecutionRoute,
    entry: UserAddress,
    stack: UserAddress,
    tls: UserAddress,
    auxiliary: hyper::exec::startup::AuxiliaryValues,
    initial_stack_vmar: Option<crate::kernel::mm::user_space::Vmar>,
}

impl ProcessImage {
    #[cfg(feature = "kernel-self-test")]
    #[cfg_attr(
        feature = "kernel-self-test",
        allow(
            dead_code,
            reason = "Native lifecycle self-tests require a HAL with user execution support"
        )
    )]
    pub(crate) fn try_native(
        machine: MachineAbi,
        entry: UserAddress,
        stack: UserAddress,
        tls: UserAddress,
    ) -> Result<Self, ImageError> {
        Self::try_native_with_auxiliary(
            machine,
            entry,
            stack,
            tls,
            hyper::exec::startup::AuxiliaryValues::minimal(entry.get()),
        )
    }

    pub(crate) fn try_native_with_auxiliary(
        machine: MachineAbi,
        entry: UserAddress,
        stack: UserAddress,
        tls: UserAddress,
        auxiliary: hyper::exec::startup::AuxiliaryValues,
    ) -> Result<Self, ImageError> {
        Self::try_new(
            machine,
            AbiFamily::Native,
            hyper::abi::native::HYPER_NATIVE_ABI_REVISION,
            ExecutionRoute::NativeKernel,
            entry,
            stack,
            tls,
            auxiliary,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn try_new(
        machine: MachineAbi,
        family: AbiFamily,
        revision: u64,
        route: ExecutionRoute,
        entry: UserAddress,
        stack: UserAddress,
        tls: UserAddress,
        auxiliary: hyper::exec::startup::AuxiliaryValues,
    ) -> Result<Self, ImageError> {
        if entry.get() == 0 {
            return Err(ImageError::InvalidEntry);
        }
        if stack.get() == 0 || stack.get() & 0xf != 0 {
            return Err(ImageError::InvalidStack);
        }
        if family == AbiFamily::Native && revision != 0 {
            return Err(ImageError::UnsupportedRevision);
        }
        if !matches!(
            (family, route),
            (AbiFamily::Native, ExecutionRoute::NativeKernel)
                | (
                    AbiFamily::Linux | AbiFamily::FreeBsd,
                    ExecutionRoute::Supervised { .. }
                )
        ) {
            return Err(ImageError::InvalidRoute);
        }
        Ok(Self {
            machine,
            family,
            route,
            entry,
            stack,
            tls,
            auxiliary,
            initial_stack_vmar: None,
        })
    }

    /// The address space owns the reservation; this token identifies the
    /// loader-created VMAR to wrap in a startup capability before first entry.
    pub(crate) fn with_initial_stack_vmar(
        mut self,
        vmar: crate::kernel::mm::user_space::Vmar,
    ) -> Self {
        self.initial_stack_vmar = Some(vmar);
        self
    }

    pub(crate) const fn initial_stack_vmar(&self) -> Option<crate::kernel::mm::user_space::Vmar> {
        self.initial_stack_vmar
    }

    pub(crate) const fn machine(&self) -> MachineAbi {
        self.machine
    }

    pub(crate) const fn family(&self) -> AbiFamily {
        self.family
    }

    pub(crate) const fn route(&self) -> ExecutionRoute {
        self.route
    }

    pub(crate) const fn auxiliary(&self) -> hyper::exec::startup::AuxiliaryValues {
        self.auxiliary
    }

    pub(crate) const fn initial_thread(&self) -> UserThreadStart {
        UserThreadStart {
            argument: 0,
            entry: self.entry,
            stack: self.stack,
            tls: self.tls,
        }
    }
}
