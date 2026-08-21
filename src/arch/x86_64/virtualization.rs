// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Runtime selection between the Intel VMX and AMD SVM backends.

use hyper::sync::atomic::{AtomicU8, Ordering};

use super::context::VcpuContext;

const NONE: u8 = 0;
const VMX: u8 = 1;
const SVM: u8 = 2;

static BACKEND: AtomicU8 = AtomicU8::new(NONE);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Backend {
    Vmx,
    Svm,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Stage2Format {
    Ept,
    Npt,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Stage2InvalidationError {
    Vmx(super::vmx::Error),
}

impl Backend {
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Vmx => "Intel VMX/EPT",
            Self::Svm => "AMD SVM/NPT",
        }
    }

    pub(super) const fn stage2_format(self) -> Stage2Format {
        match self {
            Self::Vmx => Stage2Format::Ept,
            Self::Svm => Stage2Format::Npt,
        }
    }

    pub(super) fn activate_stage2(self, root: u64) {
        match self {
            Self::Vmx => super::vmx::activate_ept(root),
            Self::Svm => super::svm::activate_npt(root),
        }
    }

    pub(super) fn invalidate_stage2(self, root: u64) -> Result<(), Stage2InvalidationError> {
        match self {
            Self::Vmx => super::vmx::invalidate_ept(root).map_err(Stage2InvalidationError::Vmx),
            Self::Svm => {
                super::svm::invalidate_npt();
                Ok(())
            }
        }
    }
}

pub(super) fn validate() -> Result<Backend, super::guest::ValidationError> {
    if let Some(backend) = selected() {
        return Ok(backend);
    }
    let basic = core::arch::x86_64::__cpuid(1);
    let extended_max = core::arch::x86_64::__cpuid(0x8000_0000).eax;
    let extended = (extended_max >= 0x8000_0001).then(|| core::arch::x86_64::__cpuid(0x8000_0001));
    let backend = if basic.ecx & (1 << 5) != 0 {
        super::vmx::validate()?;
        Backend::Vmx
    } else if extended.is_some_and(|features| features.ecx & (1 << 2) != 0) {
        super::svm::validate()?;
        Backend::Svm
    } else {
        return Err(super::guest::ValidationError::HardwareUnavailable);
    };
    let encoded = encode(backend);
    match BACKEND.compare_exchange(NONE, encoded, Ordering::AcqRel, Ordering::Acquire) {
        Ok(_) => Ok(backend),
        Err(value) if value == encoded => Ok(backend),
        Err(_) => Err(super::guest::ValidationError::BackendConflict),
    }
}

pub(super) fn selected() -> Option<Backend> {
    match BACKEND.load(Ordering::Acquire) {
        VMX => Some(Backend::Vmx),
        SVM => Some(Backend::Svm),
        _ => None,
    }
}

pub(super) fn backend_name() -> &'static str {
    selected().map_or("unselected x86 virtualization", Backend::name)
}

pub(super) fn observe_host_interrupt(vector: u32) {
    if selected() == Some(Backend::Svm) {
        super::svm::observe_host_interrupt(vector);
    }
}

pub(super) unsafe fn enter(context: *mut VcpuContext) -> ! {
    match selected() {
        // SAFETY: The caller's raw-context contract is forwarded to the selected backend.
        Some(Backend::Vmx) => unsafe { super::vmx::enter(context) },
        // SAFETY: The caller's raw-context contract is forwarded to the selected backend.
        Some(Backend::Svm) => unsafe { super::svm::enter(context) },
        None => crate::kernel::boot::fail("x86 virtualization backend selection", NONE),
    }
}

const fn encode(backend: Backend) -> u8 {
    match backend {
        Backend::Vmx => VMX,
        Backend::Svm => SVM,
    }
}
