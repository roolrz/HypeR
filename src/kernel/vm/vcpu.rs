//! vCPU interrupt-interface activation and reconciliation.

use core::convert::Infallible;

pub(super) fn create_thread(
    vm: super::registry::VmBinding,
    vcpu_id: u32,
    context: crate::arch::vm::VcpuContext,
) -> Result<crate::kernel::task::scheduler::DormantVcpuThread, crate::kernel::task::scheduler::Error>
{
    crate::kernel::task::scheduler::vcpu_create("vcpu/0", vm, vcpu_id, context, thread_entry)
}

extern "C" fn thread_entry(_argument: usize) {
    run_current()
}

fn run_current() -> ! {
    match try_run_current() {
        Ok(never) => match never {},
        Err(error) => crate::kernel::boot::fail("vCPU execution startup", error),
    }
}

fn try_run_current() -> Result<Infallible, RunError> {
    crate::arch::irq::disable_local();
    let current = crate::kernel::task::scheduler::current_vcpu().map_err(RunError::Scheduler)?;
    let stack_marker = 0usize;
    let stack_pointer = (&stack_marker as *const usize) as usize;
    if stack_pointer < current.stack.0 || stack_pointer >= current.stack.1 {
        return Err(RunError::InvalidStack(current.stack));
    }
    let execution = current.execution;
    if execution.is_null() || !execution.is_aligned() {
        return Err(RunError::InvalidExecution);
    }
    // SAFETY: The scheduler-origin pointer identifies its pinned current vCPU.
    // The binding pointer targets a field in that pinned allocation and is
    // converted to a scoped reference only before active publication below.
    let (vm, virtual_machine, vcpu_id) = unsafe {
        let execution_ref = &*execution;
        let vm = execution_ref
            .vm_binding()
            .ok_or(RunError::MissingVmBinding)?;
        (core::ptr::from_ref(vm), vm.id(), execution_ref.vcpu_id)
    };
    crate::println!(
        "HypeR: vCPU {} running as scheduler thread {} on guarded stack {:#x}-{:#x}; VM {:?}",
        vcpu_id,
        current.thread.get(),
        current.stack.0,
        current.stack.1,
        virtual_machine
    );
    // SAFETY: This current scheduler Thread exclusively owns the stopped vCPU,
    // the installed VM aggregate is pinned, and local interrupts remain masked.
    unsafe {
        super::memory::activate(&*vm).map_err(RunError::Memory)?;
        crate::kernel::task::thread::VcpuExecution::activate_virtual_hardware(execution)
            .map_err(RunError::VirtualHardware)?;
        crate::arch::vm::enable_interrupts_for_entry();
        // No Rust reference to VcpuExecution or VcpuContext is live here. VM
        // exits and asynchronous guest exceptions may therefore reconstruct
        // short-lived exclusive references from the published raw owner token.
        let context = core::ptr::addr_of_mut!((*execution).context);
        crate::arch::vm::VcpuContext::enter(context)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunError {
    InvalidExecution,
    MissingVmBinding,
    InvalidStack((usize, usize)),
    Memory(super::memory::Error),
    Scheduler(crate::kernel::task::scheduler::Error),
    VirtualHardware(VcpuInterruptError),
}

pub use crate::arch::vm::VcpuInterruptError;
