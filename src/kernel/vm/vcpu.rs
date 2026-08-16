//! vCPU interrupt-interface activation and reconciliation.

#[cfg(target_arch = "aarch64")]
use hyper::vm::interrupt::{VirtualCpuId, VirtualInterruptController, VirtualInterruptId};

use super::VmInterruptController;
use crate::kernel::task::thread::{ThreadId, VcpuExecution, VirtualMachineId};

pub(super) fn create_thread(
    virtual_machine: VirtualMachineId,
    vcpu_id: u32,
    context: crate::arch::VcpuContext,
) -> Result<ThreadId, crate::kernel::task::scheduler::Error> {
    crate::kernel::task::scheduler::vcpu_create(
        "vcpu/0",
        virtual_machine,
        vcpu_id,
        context,
        thread_entry,
    )
}

extern "C" fn thread_entry(_argument: usize) {
    run_current()
}

fn run_current() -> ! {
    crate::arch::disable_local_interrupts();
    let current = match crate::kernel::task::scheduler::current_vcpu() {
        Ok(current) => current,
        Err(error) => crate::kernel::boot::fail("current vCPU lookup", error),
    };
    let stack_marker = 0usize;
    let stack_pointer = (&stack_marker as *const usize) as usize;
    if stack_pointer < current.stack.0 || stack_pointer >= current.stack.1 {
        crate::kernel::boot::fail("vCPU kernel-stack validation", current.stack);
    }
    // SAFETY: The scheduler pins the current Thread and grants its vCPU
    // payload exclusively to this non-returning run loop.
    let execution = unsafe { &mut *current.execution };
    let interrupts = match super::runtime::interrupts(execution.virtual_machine) {
        Ok(interrupts) => interrupts,
        Err(error) => crate::kernel::boot::fail("vCPU runtime lookup", error),
    };
    crate::println!(
        "HypeR: vCPU {} running as scheduler thread {} on guarded stack {:#x}-{:#x}",
        execution.vcpu_id,
        current.thread.get(),
        current.stack.0,
        current.stack.1
    );
    // SAFETY: This current scheduler Thread exclusively owns the stopped vCPU,
    // runtime objects are pinned, and local interrupts remain masked.
    unsafe {
        if let Err(error) = execution.activate_virtual_hardware(interrupts) {
            crate::kernel::boot::fail("vCPU virtual-hardware activation", error);
        }
        if let Err(error) = super::memory::activate(execution.virtual_machine) {
            crate::kernel::boot::fail("vCPU stage-2 activation", error);
        }
        // x86 hardware-virtualization entry owns IF/GIF and dispatches host
        // interrupts through its VM-exit path. Other architectures consume
        // ordinary lower-EL interrupts while the guest is running.
        #[cfg(not(target_arch = "x86_64"))]
        crate::arch::enable_local_irq();
        execution.context.enter()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VcpuInterruptError {
    Active(super::active_vcpu::Error),
    #[cfg(target_arch = "aarch64")]
    Architecture(crate::arch::VgicError),
    #[cfg(target_arch = "aarch64")]
    Bridge(super::arch_timer::Error),
    #[cfg(target_arch = "aarch64")]
    Controller(hyper::vm::interrupt::Error),
    #[cfg(target_arch = "aarch64")]
    HostInterrupt,
}

impl From<super::active_vcpu::Error> for VcpuInterruptError {
    fn from(error: super::active_vcpu::Error) -> Self {
        Self::Active(error)
    }
}

#[cfg(target_arch = "aarch64")]
impl From<crate::arch::VgicError> for VcpuInterruptError {
    fn from(error: crate::arch::VgicError) -> Self {
        Self::Architecture(error)
    }
}

#[cfg(target_arch = "aarch64")]
impl From<hyper::vm::interrupt::Error> for VcpuInterruptError {
    fn from(error: hyper::vm::interrupt::Error) -> Self {
        Self::Controller(error)
    }
}

#[cfg(target_arch = "aarch64")]
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
    #[cfg(target_arch = "aarch64")]
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
            Ok::<(), hyper::vm::interrupt::Error>(())
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
    #[cfg(target_arch = "aarch64")]
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
    #[cfg(target_arch = "aarch64")]
    pub unsafe fn activate_timer(&self) {
        unsafe { self.context.activate_timer() };
    }

    /// Saves and disables the hardware-assisted architectural virtual timer.
    ///
    /// # Safety
    ///
    /// Local IRQs must be masked and this must be the active local vCPU.
    #[cfg(target_arch = "aarch64")]
    pub unsafe fn deactivate_timer(&mut self) {
        unsafe { self.context.deactivate_timer() };
    }

    /// Refills hardware list-register state and enables virtual delivery.
    ///
    /// # Safety
    ///
    /// The caller must own this stopped vCPU and the matching VM interrupt
    /// controller exclusively until the corresponding deactivation.
    #[cfg(target_arch = "aarch64")]
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
    #[cfg(target_arch = "aarch64")]
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

#[cfg(target_arch = "riscv64")]
impl VcpuExecution {
    /// Publishes the local active-vCPU binding before entering virtual mode.
    ///
    /// # Safety
    ///
    /// The caller must exclusively own this pinned vCPU and keep local
    /// interrupts masked until guest entry completes.
    pub unsafe fn activate_virtual_hardware(
        &mut self,
        interrupts: &VmInterruptController,
    ) -> Result<(), VcpuInterruptError> {
        unsafe { self.context.activate_system_registers() };
        unsafe { super::active_vcpu::set(self, interrupts)? };
        Ok(())
    }

    /// Removes the local active-vCPU binding after leaving virtual mode.
    ///
    /// # Safety
    ///
    /// This must be the vCPU currently bound to the calling CPU, with local
    /// interrupts masked throughout deactivation.
    pub unsafe fn deactivate_virtual_hardware(
        &mut self,
        _interrupts: &VmInterruptController,
    ) -> Result<(), VcpuInterruptError> {
        super::active_vcpu::clear(self)?;
        unsafe { self.context.deactivate_system_registers() };
        Ok(())
    }
}

#[cfg(target_arch = "x86_64")]
impl VcpuExecution {
    /// Publishes the local active-vCPU binding before VMX guest entry.
    ///
    /// # Safety
    ///
    /// The caller must exclusively own this pinned vCPU and keep interrupts
    /// masked until the VMCS has been activated.
    pub unsafe fn activate_virtual_hardware(
        &mut self,
        interrupts: &VmInterruptController,
    ) -> Result<(), VcpuInterruptError> {
        unsafe { super::active_vcpu::set(self, interrupts)? };
        Ok(())
    }

    /// Removes the local active-vCPU binding after VMX guest execution.
    ///
    /// # Safety
    ///
    /// This must be the vCPU currently bound to the calling physical CPU.
    pub unsafe fn deactivate_virtual_hardware(
        &mut self,
        _interrupts: &VmInterruptController,
    ) -> Result<(), VcpuInterruptError> {
        super::active_vcpu::clear(self)?;
        Ok(())
    }
}

#[cfg(target_arch = "aarch64")]
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
            hyper::vm::interrupt::Error::NotConfigured,
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
        Ok::<(), hyper::vm::interrupt::Error>(())
    });
    if let Err(error) = result {
        crate::arch::disable_vgic();
        return Err(error.into());
    }
    // SAFETY: The complete current-vCPU model has been refilled into its LRs.
    unsafe { execution.context.activate_vgic()? };
    Ok(())
}

#[cfg(target_arch = "aarch64")]
pub(super) fn update_active_device_interrupt(
    interrupt: VirtualInterruptId,
    asserted: bool,
) -> Result<bool, VcpuInterruptError> {
    let active = super::active_vcpu::with(|execution, interrupts| {
        update_device_interrupt(execution, interrupts, interrupt, asserted)
    })?;
    match active {
        Some(result) => result.map(|()| true),
        None => Ok(false),
    }
}

#[cfg(target_arch = "aarch64")]
fn update_device_interrupt(
    execution: &mut VcpuExecution,
    interrupts: &VmInterruptController,
    interrupt: VirtualInterruptId,
    asserted: bool,
) -> Result<(), VcpuInterruptError> {
    // SAFETY: Guest exception and physical IRQ entry both mask local IRQs, and
    // active_vcpu proves this is the vCPU whose virtual interface is loaded.
    unsafe { execution.context.deactivate_vgic()? };
    let result = interrupts.with(|controller| {
        let vcpu = VirtualCpuId::new(execution.vcpu_id);
        controller.synchronize(vcpu, execution.context.vgic.slots())?;
        if asserted {
            controller.inject(interrupt, vcpu)?;
        } else {
            controller.clear_pending(interrupt, vcpu)?;
        }
        let _ = controller.refill(vcpu, execution.context.vgic.slots_mut())?;
        Ok::<(), hyper::vm::interrupt::Error>(())
    });
    if let Err(error) = result {
        crate::arch::disable_vgic();
        return Err(error.into());
    }
    // SAFETY: The model and complete LR snapshot have just been reconciled.
    unsafe { execution.context.activate_vgic()? };
    Ok(())
}

#[cfg(target_arch = "aarch64")]
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
