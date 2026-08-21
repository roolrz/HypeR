use hyper::drivers::interrupt::plic::{Error as PlicError, Plic};
use hyper::hal::interrupt::{
    InterruptController, InterruptId, InterruptPriority, InterruptTrigger,
    KernelInterruptController,
};
use hyper::platform::InterruptControllerInfo;

pub struct Riscv64InterruptController(Plic<super::barrier::Riscv64Barrier>);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Plic(PlicError),
    Unsupported,
}

impl From<PlicError> for Error {
    fn from(error: PlicError) -> Self {
        Self::Plic(error)
    }
}

impl Riscv64InterruptController {
    /// Binds the PLIC selected by early architecture discovery.
    ///
    /// # Safety
    ///
    /// `map` must return a permanent MMIO mapping owned by the controller.
    pub unsafe fn bind(
        info: InterruptControllerInfo,
        map: impl FnMut(u64) -> Option<usize>,
    ) -> Result<Self, Error> {
        let InterruptControllerInfo::Plic(info) = info else {
            return Err(Error::Unsupported);
        };
        // SAFETY: This function forwards its permanent-mapping contract to PLIC.
        Ok(Self(unsafe {
            Plic::bind(info, map, super::current_hardware_id)?
        }))
    }
}

impl InterruptController for Riscv64InterruptController {
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

impl KernelInterruptController for Riscv64InterruptController {
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
        self.0.is_per_cpu(interrupt)
    }

    unsafe fn initialize_local(&mut self) -> Result<(), Self::Error> {
        // SAFETY: The trait contract guarantees exclusive local-controller setup.
        unsafe { self.0.initialize_local() }.map_err(Error::from)?;
        super::interrupts::enable_kernel_sources();
        Ok(())
    }
}
