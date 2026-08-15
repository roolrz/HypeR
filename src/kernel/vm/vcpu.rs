//! vCPU interrupt-interface activation and reconciliation.

use hyper::drivers::interrupt::vgic::{VirtualCpuId, VirtualInterruptController};

use super::VmInterruptController;
use crate::kernel::task::thread::VcpuExecution;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VcpuInterruptError {
    Architecture(crate::arch::VgicError),
    Bridge(super::arch_timer::Error),
    Controller(hyper::drivers::interrupt::vgic::Error),
    HostInterrupt,
}

impl From<crate::arch::VgicError> for VcpuInterruptError {
    fn from(error: crate::arch::VgicError) -> Self {
        Self::Architecture(error)
    }
}

impl From<hyper::drivers::interrupt::vgic::Error> for VcpuInterruptError {
    fn from(error: hyper::drivers::interrupt::vgic::Error) -> Self {
        Self::Controller(error)
    }
}

impl From<super::arch_timer::Error> for VcpuInterruptError {
    fn from(error: super::arch_timer::Error) -> Self {
        Self::Bridge(error)
    }
}

impl VcpuExecution {
    /// Activates the timer and interrupt hardware used by a guest vCPU.
    ///
    /// # Safety
    ///
    /// The caller must keep both objects pinned, own this stopped vCPU
    /// exclusively, and keep local IRQs masked until entering the guest.
    pub unsafe fn activate_virtual_hardware(
        &mut self,
        interrupts: &VmInterruptController,
    ) -> Result<(), VcpuInterruptError> {
        let now = crate::kernel::time::monotonic_ticks();
        super::arch_timer::reconcile_saved(self, interrupts, now)?;
        let timer_asserted = self.context.virtual_timer_interrupt_asserted_at(now);
        set_host_timer_enabled(!timer_asserted)?;

        // The timer is restored before the vGIC so an already-expired level is
        // represented in the model before virtual delivery is enabled.
        unsafe { self.context.activate_timer() };
        let result = interrupts.with(|controller| {
            let vcpu = VirtualCpuId::new(self.vcpu_id);
            controller.synchronize(vcpu, self.context.vgic.slots())?;
            let _ = controller.refill(vcpu, self.context.vgic.slots_mut())?;
            Ok::<(), hyper::drivers::interrupt::vgic::Error>(())
        });
        if let Err(error) = result {
            unsafe { self.context.deactivate_timer() };
            let _ = set_host_timer_enabled(true);
            return Err(error.into());
        }
        if let Err(error) = unsafe { self.context.activate_vgic() } {
            unsafe { self.context.deactivate_timer() };
            let _ = set_host_timer_enabled(true);
            return Err(error.into());
        }
        if let Err(error) = unsafe { super::arch_timer::set_active(self, interrupts) } {
            crate::arch::disable_vgic();
            unsafe { self.context.deactivate_timer() };
            let _ = set_host_timer_enabled(true);
            return Err(error.into());
        }
        Ok(())
    }

    /// Saves guest timer/vGIC state and removes the local active binding.
    ///
    /// # Safety
    ///
    /// Local IRQs must be masked and this must be the active local vCPU paired
    /// with the same VM interrupt controller used for activation.
    pub unsafe fn deactivate_virtual_hardware(
        &mut self,
        interrupts: &VmInterruptController,
    ) -> Result<(), VcpuInterruptError> {
        super::arch_timer::clear_active(self)?;
        unsafe { self.context.deactivate_timer() };
        unsafe { self.context.deactivate_vgic()? };
        let result = interrupts.with(|controller| {
            controller.synchronize(VirtualCpuId::new(self.vcpu_id), self.context.vgic.slots())
        });
        result?;
        super::arch_timer::reconcile_saved(
            self,
            interrupts,
            crate::kernel::time::monotonic_ticks(),
        )?;
        set_host_timer_enabled(true)?;
        Ok(())
    }

    /// Loads the hardware-assisted architectural virtual timer.
    ///
    /// # Safety
    ///
    /// The caller must own this stopped vCPU exclusively and must not enable
    /// guest execution until all remaining architectural state is restored.
    pub unsafe fn activate_timer(&self) {
        unsafe { self.context.activate_timer() };
    }

    /// Saves and disables the hardware-assisted architectural virtual timer.
    ///
    /// # Safety
    ///
    /// Local IRQs must be masked and this must be the active local vCPU.
    pub unsafe fn deactivate_timer(&mut self) {
        unsafe { self.context.deactivate_timer() };
    }

    /// Refills hardware list-register state and enables virtual delivery.
    ///
    /// # Safety
    ///
    /// The caller must own this stopped vCPU and the matching VM interrupt
    /// controller exclusively until the corresponding deactivation.
    pub unsafe fn activate_vgic(
        &mut self,
        controller: &mut VirtualInterruptController,
    ) -> Result<(), VcpuInterruptError> {
        let vcpu = VirtualCpuId::new(self.vcpu_id);
        controller.synchronize(vcpu, self.context.vgic.slots())?;
        let _ = controller.refill(vcpu, self.context.vgic.slots_mut())?;
        unsafe { self.context.activate_vgic()? };
        Ok(())
    }

    /// Saves hardware list registers and reconciles guest interrupt progress.
    ///
    /// # Safety
    ///
    /// This must be the vCPU whose virtual interface is active locally.
    pub unsafe fn deactivate_vgic(
        &mut self,
        controller: &mut VirtualInterruptController,
    ) -> Result<(), VcpuInterruptError> {
        unsafe { self.context.deactivate_vgic()? };
        let vcpu = VirtualCpuId::new(self.vcpu_id);
        controller.synchronize(vcpu, self.context.vgic.slots())?;
        Ok(())
    }
}

fn set_host_timer_enabled(enabled: bool) -> Result<(), VcpuInterruptError> {
    let interrupt = crate::kernel::irq::timer::guest_virtual_host_interrupt()
        .ok_or(VcpuInterruptError::HostInterrupt)?;
    let result = if enabled {
        crate::kernel::irq::interrupt::enable_local(interrupt)
    } else {
        crate::kernel::irq::interrupt::disable_local(interrupt)
    };
    result.map_err(|_| VcpuInterruptError::HostInterrupt)
}
