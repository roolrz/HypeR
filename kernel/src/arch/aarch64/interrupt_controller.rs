// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use hyper::drivers::interrupt::gicv3::{Error as GicError, GicV3, GicV3Local};
use hyper::hal::interrupt::{
    InterruptController, InterruptId, InterruptPriority, InterruptTransitionError,
    InterruptTrigger, KernelInterruptController, LocalInterruptController,
};
use hyper::platform::InterruptControllerInfo;

use super::{Aarch64GicCpuInterface, barrier::Aarch64Barrier, timer::ArmGenericCounter};

type Controller = GicV3<Aarch64GicCpuInterface, Aarch64Barrier, ArmGenericCounter>;

pub struct Aarch64InterruptController(Controller);
pub struct Aarch64LocalInterruptController(
    GicV3Local<Aarch64GicCpuInterface, Aarch64Barrier, ArmGenericCounter>,
);

impl LocalInterruptController for Aarch64LocalInterruptController {
    type Error = Error;

    fn configure(
        &self,
        interrupt: InterruptId,
        priority: InterruptPriority,
        trigger: InterruptTrigger,
    ) -> Result<(), Error> {
        self.0
            .configure(interrupt, priority, trigger)
            .map_err(Into::into)
    }

    fn enable(&self, interrupt: InterruptId) -> Result<(), InterruptTransitionError<Error>> {
        self.0
            .enable(interrupt)
            .map_err(|error| error.map(Into::into))
    }

    fn disable(&self, interrupt: InterruptId) -> Result<(), InterruptTransitionError<Error>> {
        self.0
            .disable(interrupt)
            .map_err(|error| error.map(Into::into))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Gic(GicError),
    Unsupported,
}

impl From<GicError> for Error {
    fn from(error: GicError) -> Self {
        Self::Gic(error)
    }
}

impl Aarch64InterruptController {
    /// Binds and initializes the firmware-selected `GICv3` instance.
    ///
    /// # Safety
    ///
    /// `map` must return permanent Device mappings and the caller must own the
    /// controller while local interrupts remain masked.
    pub unsafe fn bind(
        info: InterruptControllerInfo,
        map: impl FnMut(u64) -> Option<usize>,
    ) -> Result<Self, Error> {
        let InterruptControllerInfo::GicV3(info) = info else {
            return Err(Error::Unsupported);
        };
        // SAFETY: The caller guarantees permanent Device mappings and
        // exclusive ownership of the firmware-described controller.
        let mut controller = unsafe { Controller::bind(info, map)? };
        // SAFETY: The newly bound controller remains exclusively owned and
        // local interrupts are still masked.
        unsafe { controller.initialize(super::current_gic_affinity())? };
        Ok(Self(controller))
    }
}

impl InterruptController for Aarch64InterruptController {
    type Error = Error;

    fn enable(
        &mut self,
        interrupt: InterruptId,
    ) -> Result<(), InterruptTransitionError<Self::Error>> {
        self.0
            .enable(interrupt)
            .map_err(|error| error.map(Into::into))
    }

    fn disable(
        &mut self,
        interrupt: InterruptId,
    ) -> Result<(), InterruptTransitionError<Self::Error>> {
        self.0
            .disable(interrupt)
            .map_err(|error| error.map(Into::into))
    }

    fn acknowledge(&self) -> Option<InterruptId> {
        self.0.acknowledge()
    }

    fn end(&self, interrupt: InterruptId) {
        self.0.end(interrupt);
    }
}

impl KernelInterruptController for Aarch64InterruptController {
    type Local = Aarch64LocalInterruptController;

    fn interrupt_count(&self) -> u32 {
        self.0.interrupt_count()
    }

    fn configure(
        &mut self,
        interrupt: InterruptId,
        priority: InterruptPriority,
        trigger: InterruptTrigger,
    ) -> Result<(), Self::Error> {
        self.0
            .configure(interrupt, priority, trigger)
            .map_err(Into::into)
    }

    fn is_per_cpu(&self, interrupt: InterruptId) -> bool {
        interrupt.get() < 32
    }

    fn local_controller(&self) -> Result<Self::Local, Self::Error> {
        self.0
            .local_controller()
            .map(Aarch64LocalInterruptController)
            .map_err(Into::into)
    }

    unsafe fn initialize_local(&mut self) -> Result<Self::Local, Self::Error> {
        // SAFETY: The trait caller owns this CPU's redistributor/interface and
        // invokes local initialization with interrupts masked.
        unsafe { self.0.initialize_local(super::current_gic_affinity()) }.map_err(Error::from)?;
        self.local_controller()
    }
}
