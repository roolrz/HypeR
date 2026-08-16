//! Per-VM interrupt-controller ownership and timer PPI policy.

#[cfg(target_arch = "aarch64")]
mod backend {

    use hyper::hal::interrupt::InterruptId;
    use hyper::sync::InterruptSpinLock;
    use hyper::vm::interrupt::{
        Error as VgicError, InterruptGroup, InterruptSnapshot, InterruptTrigger, VirtualCpuId,
        VirtualInterruptController, VirtualInterruptId,
    };

    type ControllerLock = InterruptSpinLock<ControllerState, crate::arch::LocalInterruptMask>;

    const TIMER_PRIORITY: u8 = 0x80;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum Error {
        InvalidInterrupt,
        Vgic(VgicError),
    }

    impl From<VgicError> for Error {
        fn from(error: VgicError) -> Self {
            Self::Vgic(error)
        }
    }

    /// Serialized virtual interrupt state shared by all vCPUs in one VM.
    pub struct VmInterruptController {
        state: ControllerLock,
        timer_interrupt: VirtualInterruptId,
        vcpu_count: u32,
    }

    struct ControllerState {
        controller: VirtualInterruptController,
        distributor_control: u32,
    }

    impl VmInterruptController {
        pub fn new(vcpu_count: u32, timer_interrupt: InterruptId) -> Result<Self, Error> {
            let timer_interrupt =
                VirtualInterruptId::new(timer_interrupt.get()).ok_or(Error::InvalidInterrupt)?;
            let mut controller = VirtualInterruptController::new(vcpu_count)?;
            for index in 0..vcpu_count {
                let vcpu = VirtualCpuId::new(index);
                for id in 0..32 {
                    let interrupt = VirtualInterruptId::new(id).ok_or(Error::InvalidInterrupt)?;
                    controller.configure(
                        interrupt,
                        vcpu,
                        TIMER_PRIORITY,
                        InterruptGroup::Group1,
                        if id < 16 {
                            InterruptTrigger::Edge
                        } else {
                            InterruptTrigger::Level
                        },
                    )?;
                }
                controller.set_maintenance_on_eoi(timer_interrupt, vcpu, true)?;
                // GIC register emulation will eventually transfer ownership of
                // this enable bit to the guest Redistributor model.
                controller.set_enabled(timer_interrupt, vcpu, true)?;
            }
            for id in 32..64 {
                controller.configure(
                    VirtualInterruptId::new(id).ok_or(Error::InvalidInterrupt)?,
                    VirtualCpuId::new(0),
                    TIMER_PRIORITY,
                    InterruptGroup::Group1,
                    InterruptTrigger::Level,
                )?;
            }
            Ok(Self {
                state: InterruptSpinLock::new(ControllerState {
                    controller,
                    distributor_control: 0,
                }),
                timer_interrupt,
                vcpu_count,
            })
        }

        pub const fn timer_interrupt(&self) -> VirtualInterruptId {
            self.timer_interrupt
        }

        pub(crate) const fn vcpu_count(&self) -> u32 {
            self.vcpu_count
        }

        pub(crate) fn with<R>(
            &self,
            operation: impl FnOnce(&mut VirtualInterruptController) -> R,
        ) -> R {
            self.state.with(|state| operation(&mut state.controller))
        }

        pub(crate) fn timer_snapshot(
            &self,
            vcpu: VirtualCpuId,
        ) -> Result<InterruptSnapshot, VgicError> {
            self.state
                .with(|state| state.controller.snapshot(self.timer_interrupt, vcpu))
        }

        pub(crate) fn distributor_control(&self) -> u32 {
            self.state.with(|state| state.distributor_control)
        }

        pub(crate) fn set_distributor_control(&self, value: u32) {
            self.state.with(|state| {
                // Only the non-secure group enables and affinity routing are
                // meaningful in the initial one-security-state virtual GIC.
                state.distributor_control = value & ((1 << 4) | (1 << 1));
            });
        }
    }
}

#[cfg(target_arch = "aarch64")]
pub use backend::{Error, VmInterruptController};

#[cfg(target_arch = "riscv64")]
mod backend {
    use hyper::hal::interrupt::InterruptId;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum Error {
        InvalidVcpuCount,
    }

    /// RISC-V delivers virtual local interrupts through H-extension state.
    /// This object retains only VM-wide topology until an AIA virtual IMSIC
    /// backend is added; it deliberately does not expose GIC semantics.
    pub struct VmInterruptController;

    impl VmInterruptController {
        pub fn new(vcpu_count: u32, _timer_interrupt: InterruptId) -> Result<Self, Error> {
            if vcpu_count == 0 {
                return Err(Error::InvalidVcpuCount);
            }
            Ok(Self)
        }
    }
}

#[cfg(target_arch = "riscv64")]
pub use backend::{Error, VmInterruptController};

#[cfg(target_arch = "x86_64")]
mod backend {
    use hyper::hal::interrupt::InterruptId;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum Error {
        InvalidVcpuCount,
    }

    pub struct VmInterruptController;

    impl VmInterruptController {
        pub fn new(vcpu_count: u32, _timer_interrupt: InterruptId) -> Result<Self, Error> {
            if vcpu_count == 0 {
                return Err(Error::InvalidVcpuCount);
            }
            Ok(Self)
        }
    }
}

#[cfg(target_arch = "x86_64")]
pub use backend::{Error, VmInterruptController};
