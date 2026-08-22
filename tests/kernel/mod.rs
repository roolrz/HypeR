// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Bare-metal integration tests compiled only by `kernel-self-test`.

mod guest_memory_access;
mod scheduler_sync;
mod stack_model;
mod startup_readiness;
mod user_memory_access;
mod vm_registry;

pub(crate) fn run() {
    crate::arch::irq::enable_local();
    let result = scheduler_sync::run();
    let stack_result = stack_model::run();
    let readiness_result = startup_readiness::run();
    let guest_execution = crate::arch::vm::guest_execution_available();
    let guest_memory_result = guest_execution.then(guest_memory_access::run);
    let user_memory_result = user_memory_access::run();
    let vm_registry_result = vm_registry::run();
    crate::arch::irq::disable_local();
    if let Err(error) = result {
        crate::kernel::boot::fail("kernel scheduler/sync tests", error);
    }
    if let Err(error) = stack_result {
        crate::kernel::boot::fail("kernel stack-model tests", error);
    }
    if let Err(error) = readiness_result {
        crate::kernel::boot::fail("kernel startup-readiness tests", error);
    }
    if let Some(Err(error)) = guest_memory_result {
        crate::kernel::boot::fail("kernel guest-memory access tests", error);
    }
    if let Err(error) = user_memory_result {
        crate::kernel::boot::fail("kernel user-memory access tests", error);
    }
    if let Err(error) = vm_registry_result {
        crate::kernel::boot::fail("kernel VM registry tests", error);
    }
    crate::println!("HypeR test: scheduler ready/wait queues and sleeping sync passed");
    crate::println!("HypeR test: guarded thread, IRQ, and emergency stacks passed");
    crate::println!("HypeR test: fatal-path readiness contract passed");
    if guest_execution {
        crate::println!("HypeR test: checked stage-2 guest-memory copies passed");
    } else {
        crate::println!("HypeR test: stage-2 guest-memory copies skipped (no virtualization)");
    }
    crate::println!("HypeR test: checked application-memory copies passed");
    if guest_execution {
        crate::println!("HypeR test: VM and dormant-vCPU rollback passed");
    } else {
        crate::println!("HypeR test: VM reservation rollback passed");
        crate::println!("HypeR test: dormant-vCPU rollback skipped (no virtualization)");
    }
}
