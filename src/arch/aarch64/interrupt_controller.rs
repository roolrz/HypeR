use hyper::drivers::interrupt::gicv3::{Error as GicError, GicV3};
use hyper::hal::interrupt::{
    InterruptController, InterruptId, InterruptTrigger, KernelInterruptController,
};
use hyper::platform::InterruptControllerInfo;

use super::{Aarch64Barrier, Aarch64GicCpuInterface};

type Controller = GicV3<Aarch64GicCpuInterface, Aarch64Barrier>;

pub struct Aarch64InterruptController(Controller);

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
    /// Binds and initializes the firmware-selected GICv3 instance.
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
        let mut controller = unsafe { Controller::bind(info, map)? };
        unsafe { controller.initialize(super::current_gic_affinity())? };
        Ok(Self(controller))
    }
}

impl InterruptController for Aarch64InterruptController {
    type Error = Error;

    fn enable(&mut self, interrupt: InterruptId) -> Result<(), Self::Error> {
        self.0.enable(interrupt).map_err(Into::into)
    }

    fn disable(&mut self, interrupt: InterruptId) -> Result<(), Self::Error> {
        self.0.disable(interrupt).map_err(Into::into)
    }

    fn acknowledge(&self) -> Option<InterruptId> {
        self.0.acknowledge()
    }

    fn end(&self, interrupt: InterruptId) {
        self.0.end(interrupt);
    }
}

impl KernelInterruptController for Aarch64InterruptController {
    fn interrupt_count(&self) -> u32 {
        self.0.interrupt_count()
    }

    fn configure(
        &mut self,
        interrupt: InterruptId,
        priority: u8,
        trigger: InterruptTrigger,
    ) -> Result<(), Self::Error> {
        self.0
            .configure(interrupt, priority, trigger)
            .map_err(Into::into)
    }

    fn is_per_cpu(&self, interrupt: InterruptId) -> bool {
        interrupt.get() < 32
    }

    unsafe fn initialize_local(&mut self) -> Result<(), Self::Error> {
        unsafe { self.0.initialize_local(super::current_gic_affinity()) }.map_err(Into::into)
    }
}
