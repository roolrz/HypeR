//! vCPU interrupt-interface activation and reconciliation.

use hyper::drivers::interrupt::vgic::{
    VirtualCpuId, VirtualInterruptController, VirtualInterruptId,
};

use super::VmInterruptController;
use crate::kernel::task::thread::VcpuExecution;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VcpuInterruptError {
    Active(super::active_vcpu::Error),
    Architecture(crate::arch::VgicError),
    Bridge(super::arch_timer::Error),
    Controller(hyper::drivers::interrupt::vgic::Error),
    HostInterrupt,
}

impl From<super::active_vcpu::Error> for VcpuInterruptError {
    fn from(error: super::active_vcpu::Error) -> Self {
        Self::Active(error)
    }
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

        unsafe { self.context.activate_system_registers() };
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
            unsafe { self.context.deactivate_system_registers() };
            set_host_timer_enabled(true)?;
            return Err(error.into());
        }
        if let Err(error) = unsafe { self.context.activate_vgic() } {
            unsafe { self.context.deactivate_timer() };
            unsafe { self.context.deactivate_system_registers() };
            set_host_timer_enabled(true)?;
            return Err(error.into());
        }
        if let Err(error) = unsafe { super::active_vcpu::set(self, interrupts) } {
            crate::arch::disable_vgic();
            unsafe { self.context.deactivate_timer() };
            unsafe { self.context.deactivate_system_registers() };
            set_host_timer_enabled(true)?;
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
        super::active_vcpu::clear(self)?;
        unsafe { self.context.deactivate_timer() };
        let vgic_result = unsafe { self.context.deactivate_vgic() };
        unsafe { self.context.deactivate_system_registers() };
        let state_result = (|| {
            vgic_result?;
            interrupts.with(|controller| {
                controller.synchronize(VirtualCpuId::new(self.vcpu_id), self.context.vgic.slots())
            })?;
            super::arch_timer::reconcile_saved(
                self,
                interrupts,
                crate::kernel::time::monotonic_ticks(),
            )?;
            Ok::<(), VcpuInterruptError>(())
        })();
        let host_timer_result = set_host_timer_enabled(true);
        state_result?;
        host_timer_result
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

pub(super) fn deliver_software_interrupt(
    execution: &mut VcpuExecution,
    interrupts: &VmInterruptController,
    request: u64,
) -> Result<(), VcpuInterruptError> {
    const TARGET_LIST_MASK: u64 = 0xffff;
    const AFFINITY_1_SHIFT: u32 = 16;
    const INTERRUPT_SHIFT: u32 = 24;
    const AFFINITY_2_SHIFT: u32 = 32;
    const BROADCAST: u64 = 1 << 40;
    const RANGE_SHIFT: u32 = 44;
    const AFFINITY_3_SHIFT: u32 = 48;

    let Some(interrupt) = VirtualInterruptId::new(((request >> INTERRUPT_SHIFT) & 0xf) as u32)
    else {
        return Err(VcpuInterruptError::Controller(
            hyper::drivers::interrupt::vgic::Error::NotConfigured,
        ));
    };
    let target_list = request & TARGET_LIST_MASK;
    let affinity_1 = (request >> AFFINITY_1_SHIFT) & 0xff;
    let affinity_2 = (request >> AFFINITY_2_SHIFT) & 0xff;
    let affinity_3 = (request >> AFFINITY_3_SHIFT) & 0xff;
    let range = (request >> RANGE_SHIFT) & 0xf;
    let broadcast = request & BROADCAST != 0;
    let source = execution.vcpu_id;

    // SAFETY: Guest synchronous exception entry has masked local interrupts,
    // and this is the vCPU whose hardware interface is currently loaded.
    unsafe { execution.context.deactivate_vgic()? };
    let result = interrupts.with(|controller| {
        let current = VirtualCpuId::new(source);
        controller.synchronize(current, execution.context.vgic.slots())?;
        for index in 0..interrupts.vcpu_count() {
            let aff0 = u64::from(index & 0xff);
            let aff1 = u64::from((index >> 8) & 0xff);
            let aff2 = u64::from((index >> 16) & 0xff);
            let aff3 = u64::from((index >> 24) & 0xff);
            let selected = if broadcast {
                index != source
            } else {
                aff1 == affinity_1
                    && aff2 == affinity_2
                    && aff3 == affinity_3
                    && aff0 >= range * 16
                    && aff0 < range * 16 + 16
                    && target_list & (1 << (aff0 - range * 16)) != 0
            };
            if selected {
                controller.inject(interrupt, VirtualCpuId::new(index))?;
            }
        }
        let _ = controller.refill(current, execution.context.vgic.slots_mut())?;
        Ok::<(), hyper::drivers::interrupt::vgic::Error>(())
    });
    if let Err(error) = result {
        crate::arch::disable_vgic();
        return Err(error.into());
    }
    // SAFETY: The complete current-vCPU model has been refilled into its LRs.
    unsafe { execution.context.activate_vgic()? };
    Ok(())
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
