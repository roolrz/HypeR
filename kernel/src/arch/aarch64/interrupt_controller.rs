// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use hyper::drivers::interrupt::gicv2::{GicV2, GicV2Local, SgiCompletions};
use hyper::drivers::interrupt::gicv3::{Error as GicError, GicV3, GicV3Local};
use hyper::hal::interrupt::{
    InterruptController, InterruptId, InterruptPriority, InterruptTransitionError,
    InterruptTrigger, KernelInterruptController, LocalInterruptController,
};
use hyper::platform::InterruptControllerInfo;

use super::{Aarch64GicCpuInterface, barrier::Aarch64Barrier, timer::ArmGenericCounter};

type Controller = GicV3<Aarch64GicCpuInterface, Aarch64Barrier, ArmGenericCounter>;

// Keep the one installed controller inline; binding must not allocate.
#[allow(clippy::large_enum_variant)]
enum ControllerKind {
    V2(GicV2<Aarch64Barrier>),
    V3(Controller),
}
pub struct Aarch64InterruptController(ControllerKind);
pub enum Aarch64LocalInterruptController {
    V2(GicV2Local<Aarch64Barrier>),
    V3(GicV3Local<Aarch64GicCpuInterface, Aarch64Barrier, ArmGenericCounter>),
}
static SGI_COMPLETIONS: SgiCompletions = SgiCompletions::new();

impl LocalInterruptController for Aarch64LocalInterruptController {
    type Error = Error;

    fn configure(
        &self,
        interrupt: InterruptId,
        priority: InterruptPriority,
        trigger: InterruptTrigger,
    ) -> Result<(), Error> {
        match self {
            Self::V2(c) => c
                .configure(interrupt, priority, trigger)
                .map_err(Into::into),
            Self::V3(c) => c
                .configure(interrupt, priority, trigger)
                .map_err(Into::into),
        }
    }

    fn enable(&self, interrupt: InterruptId) -> Result<(), InterruptTransitionError<Error>> {
        match self {
            Self::V2(c) => c.enable(interrupt).map_err(|e| e.map(Into::into)),
            Self::V3(c) => c.enable(interrupt).map_err(|e| e.map(Into::into)),
        }
    }

    fn disable(&self, interrupt: InterruptId) -> Result<(), InterruptTransitionError<Error>> {
        match self {
            Self::V2(c) => c.disable(interrupt).map_err(|e| e.map(Into::into)),
            Self::V3(c) => c.disable(interrupt).map_err(|e| e.map(Into::into)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Gic(GicError),
    GicV2(hyper::drivers::interrupt::gicv2::Error),
    AlreadyInitialized,
    Unsupported,
}

impl From<hyper::drivers::interrupt::gicv2::Error> for Error {
    fn from(error: hyper::drivers::interrupt::gicv2::Error) -> Self {
        Self::GicV2(error)
    }
}

impl From<GicError> for Error {
    fn from(error: GicError) -> Self {
        Self::Gic(error)
    }
}

impl Aarch64InterruptController {
    /// Binds and initializes the firmware-selected GIC instance.
    ///
    /// # Safety
    ///
    /// `map` must return permanent Device mappings and the caller must own the
    /// controller while local interrupts remain masked.
    pub unsafe fn bind(
        info: InterruptControllerInfo,
        map: impl FnMut(u64) -> Option<usize>,
    ) -> Result<Self, Error> {
        if super::gic_cpu_interface::interface_installed() {
            return Err(Error::AlreadyInitialized);
        }
        match info {
            InterruptControllerInfo::GicV2(info) => {
                // SAFETY: The caller owns permanent Device mappings with IRQs masked.
                let mut controller = unsafe { GicV2::bind(info, map, &SGI_COMPLETIONS)? };
                // SAFETY: No other CPU can access the new controller yet.
                unsafe { controller.initialize()? };
                super::gic_cpu_interface::install_v2(controller.local_controller())?;
                Ok(Self(ControllerKind::V2(controller)))
            }
            InterruptControllerInfo::GicV3(info) => {
                // SAFETY: The caller owns permanent Device mappings with IRQs masked.
                let mut controller = unsafe { Controller::bind(info, map)? };
                // SAFETY: The caller exclusively owns the controller at boot.
                unsafe { controller.initialize(super::current_gic_affinity())? };
                super::gic_cpu_interface::install_v3()?;
                Ok(Self(ControllerKind::V3(controller)))
            }
            _ => Err(Error::Unsupported),
        }
    }
}

impl InterruptController for Aarch64InterruptController {
    type Error = Error;

    fn enable(
        &mut self,
        interrupt: InterruptId,
    ) -> Result<(), InterruptTransitionError<Self::Error>> {
        match &mut self.0 {
            ControllerKind::V2(c) => c.enable(interrupt).map_err(|e| e.map(Into::into)),
            ControllerKind::V3(c) => c.enable(interrupt).map_err(|e| e.map(Into::into)),
        }
    }

    fn disable(
        &mut self,
        interrupt: InterruptId,
    ) -> Result<(), InterruptTransitionError<Self::Error>> {
        match &mut self.0 {
            ControllerKind::V2(c) => c.disable(interrupt).map_err(|e| e.map(Into::into)),
            ControllerKind::V3(c) => c.disable(interrupt).map_err(|e| e.map(Into::into)),
        }
    }

    fn acknowledge(&self) -> Option<InterruptId> {
        super::gic_cpu_interface::acknowledge_interrupt()
    }

    fn end(&self, interrupt: InterruptId) {
        super::gic_cpu_interface::end_interrupt(interrupt);
    }
}

impl KernelInterruptController for Aarch64InterruptController {
    type Local = Aarch64LocalInterruptController;

    fn interrupt_count(&self) -> u32 {
        match &self.0 {
            ControllerKind::V2(c) => c.interrupt_count(),
            ControllerKind::V3(c) => c.interrupt_count(),
        }
    }

    fn configure(
        &mut self,
        interrupt: InterruptId,
        priority: InterruptPriority,
        trigger: InterruptTrigger,
    ) -> Result<(), Self::Error> {
        match &mut self.0 {
            ControllerKind::V2(c) => c
                .configure(interrupt, priority, trigger)
                .map_err(Into::into),
            ControllerKind::V3(c) => c
                .configure(interrupt, priority, trigger)
                .map_err(Into::into),
        }
    }

    fn is_per_cpu(&self, interrupt: InterruptId) -> bool {
        interrupt.get() < 32
    }

    fn local_controller(&self) -> Result<Self::Local, Self::Error> {
        match &self.0 {
            ControllerKind::V2(c) => Ok(Aarch64LocalInterruptController::V2(c.local_controller())),
            ControllerKind::V3(c) => c
                .local_controller()
                .map(Aarch64LocalInterruptController::V3)
                .map_err(Into::into),
        }
    }

    unsafe fn initialize_local(&mut self) -> Result<Self::Local, Self::Error> {
        match &self.0 {
            ControllerKind::V2(c) => {
                // SAFETY: The trait caller owns the masked local interface.
                unsafe { c.initialize_local()? };
                super::gic_cpu_interface::register_v2_cpu(c.local_controller())?;
            }
            ControllerKind::V3(c) => {
                // SAFETY: The trait caller owns the masked local redistributor/interface.
                unsafe { c.initialize_local(super::current_gic_affinity())? };
            }
        }
        self.local_controller()
    }
}
