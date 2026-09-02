// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Kernel ownership of architectural translation-identifier namespaces.
//!
//! VHE native address spaces use the ASID namespace. nVHE native address
//! spaces and guests use the same VMID namespace. Namespace marker types make
//! feeding an ASID to VTTBR or a VMID to TTBR a type error.

use core::marker::PhantomData;

use hyper::mm::{
    ActiveTranslationId, ReservedTranslationId, RetiringTranslationId, TranslationIdError,
    TranslationIdPool,
};
use hyper::sync::InterruptSpinLock;

const IDENTIFIER_COUNT: usize = 1 << 8;

pub(crate) enum HostAsid {}
pub(crate) enum Stage2Vmid {}

type AsidPool = TranslationIdPool<HostAsid, IDENTIFIER_COUNT>;
type VmidPool = TranslationIdPool<Stage2Vmid, IDENTIFIER_COUNT>;
type AsidLock = InterruptSpinLock<AsidPool, crate::hal::irq::LocalMask>;
type VmidLock = InterruptSpinLock<VmidPool, crate::hal::irq::LocalMask>;

// SAFETY: These are the sole pools instantiated for their private namespace
// marker types in the kernel.
static ASIDS: AsidLock = InterruptSpinLock::new(unsafe { TranslationIdPool::new() });
// SAFETY: See ASIDS; Stage2Vmid is private and has exactly this one pool.
static VMIDS: VmidLock = InterruptSpinLock::new(unsafe { TranslationIdPool::new() });

pub(crate) trait IdentifierNamespace: Sized + 'static {
    fn with_pool<Result>(
        operation: impl FnOnce(&mut TranslationIdPool<Self, IDENTIFIER_COUNT>) -> Result,
    ) -> Result;
}

impl IdentifierNamespace for HostAsid {
    fn with_pool<Result>(operation: impl FnOnce(&mut AsidPool) -> Result) -> Result {
        ASIDS.with(operation)
    }
}

impl IdentifierNamespace for Stage2Vmid {
    fn with_pool<Result>(operation: impl FnOnce(&mut VmidPool) -> Result) -> Result {
        VMIDS.with(operation)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    Exhausted,
    InvalidToken,
    UnsupportedWidth,
}

impl From<TranslationIdError> for Error {
    fn from(error: TranslationIdError) -> Self {
        match error {
            TranslationIdError::Exhausted => Self::Exhausted,
            TranslationIdError::InvalidToken => Self::InvalidToken,
        }
    }
}

#[must_use = "an identifier reservation must be activated or cancelled"]
pub(crate) struct IdentifierReservation<Namespace: IdentifierNamespace> {
    token: Option<ReservedTranslationId<Namespace>>,
}

impl<Namespace: IdentifierNamespace> IdentifierReservation<Namespace> {
    pub(crate) fn value(&self) -> u16 {
        self.token.as_ref().map_or(0, ReservedTranslationId::value)
    }

    pub(crate) fn generation(&self) -> u64 {
        self.token
            .as_ref()
            .map_or(0, ReservedTranslationId::generation)
    }

    pub(crate) fn activate(mut self) -> Result<ActiveIdentifier<Namespace>, Error> {
        let token = self.token.take().ok_or(Error::InvalidToken)?;
        let active = Namespace::with_pool(|pool| pool.activate(token))?;
        Ok(ActiveIdentifier {
            token: Some(active),
            namespace: PhantomData,
        })
    }
}

impl<Namespace: IdentifierNamespace> Drop for IdentifierReservation<Namespace> {
    fn drop(&mut self) {
        let Some(token) = self.token.take() else {
            return;
        };
        if Namespace::with_pool(|pool| pool.cancel(token)).is_err() {
            crate::hal::cpu::halt();
        }
    }
}

/// Activated hardware identifier retained until acknowledged retirement.
///
/// Safe Drop intentionally leaves the pool slot active. This fail-safe leak
/// prevents reuse while stale hardware translations might still exist.
#[must_use = "an active identifier must stay retained or enter acknowledged retirement"]
pub(crate) struct ActiveIdentifier<Namespace: IdentifierNamespace> {
    token: Option<ActiveTranslationId<Namespace>>,
    namespace: PhantomData<Namespace>,
}

impl<Namespace: IdentifierNamespace> ActiveIdentifier<Namespace> {
    pub(crate) fn value(&self) -> u16 {
        self.token.as_ref().map_or(0, ActiveTranslationId::value)
    }

    pub(crate) fn generation(&self) -> u64 {
        self.token
            .as_ref()
            .map_or(0, ActiveTranslationId::generation)
    }

    pub(crate) fn begin_retirement(mut self) -> Result<RetiringIdentifier<Namespace>, Error> {
        let token = self.token.take().ok_or(Error::InvalidToken)?;
        let retiring = Namespace::with_pool(|pool| pool.begin_retirement(token))?;
        Ok(RetiringIdentifier {
            token: Some(retiring),
            namespace: PhantomData,
        })
    }
}

#[must_use = "a retiring identifier must stay retained until invalidation is acknowledged"]
pub(crate) struct RetiringIdentifier<Namespace: IdentifierNamespace> {
    token: Option<RetiringTranslationId<Namespace>>,
    namespace: PhantomData<Namespace>,
}

impl<Namespace: IdentifierNamespace> RetiringIdentifier<Namespace> {
    pub(crate) fn value(&self) -> u16 {
        self.token.as_ref().map_or(0, RetiringTranslationId::value)
    }

    pub(crate) fn generation(&self) -> u64 {
        self.token
            .as_ref()
            .map_or(0, RetiringTranslationId::generation)
    }

    /// # Safety
    ///
    /// Every CPU which could cache this identifier must have acknowledged the
    /// matching tagged invalidation. Ambiguous completion must fail-stop.
    pub(crate) unsafe fn complete(mut self) -> Result<(), Error> {
        let token = self.token.take().ok_or(Error::InvalidToken)?;
        // SAFETY: The caller supplies the acknowledgement proof unchanged.
        unsafe { Namespace::with_pool(|pool| pool.complete_retirement(token))? };
        Ok(())
    }
}

pub(crate) fn reserve<Namespace: IdentifierNamespace>(
    width: u8,
) -> Result<IdentifierReservation<Namespace>, Error> {
    if width == 0 || width > 8 {
        return Err(Error::UnsupportedWidth);
    }
    let token = Namespace::with_pool(|pool| pool.reserve_below(1usize << width))?;
    Ok(IdentifierReservation { token: Some(token) })
}
