// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Immutable execution identity selected by a trusted image loader.

use core::num::NonZeroU64;

use crate::kernel::mm::user_space::UserAddress;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MachineAbi {
    Aarch64,
    Riscv64,
    X86_64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AbiFamily {
    Native,
    Linux,
    FreeBsd,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExecutionRoute {
    NativeKernel,
    /// The session is a stable diagnostic key, not authority.
    Supervised {
        session: SupervisionSessionId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SupervisionSessionId(NonZeroU64);

impl SupervisionSessionId {
    pub(crate) const fn new(value: NonZeroU64) -> Self {
        Self(value)
    }

    pub(crate) const fn get(self) -> u64 {
        self.0.get()
    }
}

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
        Ok(Self { entry, stack, tls })
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
    revision: u64,
    route: ExecutionRoute,
    entry: UserAddress,
    stack: UserAddress,
    tls: UserAddress,
}

impl ProcessImage {
    pub(crate) fn try_native(
        machine: MachineAbi,
        entry: UserAddress,
        stack: UserAddress,
        tls: UserAddress,
    ) -> Result<Self, ImageError> {
        Self::try_new(
            machine,
            AbiFamily::Native,
            hyper::abi::native::HYPER_NATIVE_ABI_REVISION,
            ExecutionRoute::NativeKernel,
            entry,
            stack,
            tls,
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
            revision,
            route,
            entry,
            stack,
            tls,
        })
    }

    pub(crate) const fn machine(&self) -> MachineAbi {
        self.machine
    }

    pub(crate) const fn family(&self) -> AbiFamily {
        self.family
    }

    pub(crate) const fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) const fn route(&self) -> ExecutionRoute {
        self.route
    }

    pub(crate) const fn entry(&self) -> UserAddress {
        self.entry
    }

    pub(crate) const fn stack(&self) -> UserAddress {
        self.stack
    }

    pub(crate) const fn tls(&self) -> UserAddress {
        self.tls
    }

    pub(crate) const fn initial_thread(&self) -> UserThreadStart {
        UserThreadStart {
            entry: self.entry,
            stack: self.stack,
            tls: self.tls,
        }
    }
}
