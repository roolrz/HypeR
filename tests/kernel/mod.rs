// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Bare-metal integration tests compiled only by `kernel-self-test`.

#[cfg(CONFIG_ARCH_AARCH64)]
mod guest_entry_irq;
mod guest_memory_access;
#[cfg(CONFIG_ARCH_AARCH64)]
mod irq_tail_preemption;
#[cfg(any(CONFIG_ARCH_AARCH64, CONFIG_ARCH_X86_64))]
mod reschedule_ipi;
mod scheduler_sync;
mod stack_model;
mod startup_readiness;
mod thread_migration;
mod thread_sleep;
mod user_memory_access;
mod vm_registry;
mod wait_arbitration;

pub(crate) fn run() {
    crate::hal::irq::enable_local();
    #[cfg(any(CONFIG_ARCH_AARCH64, CONFIG_ARCH_X86_64))]
    let reschedule_ipi_result = reschedule_ipi::run();
    let result = scheduler_sync::run();
    let stack_result = stack_model::run();
    let sleep_result = thread_sleep::run();
    let migration_result = thread_migration::run();
    let wait_arbitration_result = wait_arbitration::run();
    let readiness_result = startup_readiness::run();
    let guest_execution = crate::hal::vm::guest_execution_available();
    let guest_memory_result = guest_execution.then(guest_memory_access::run);
    let user_memory_result = user_memory_access::run();
    let vm_registry_result = vm_registry::run();
    #[cfg(CONFIG_ARCH_AARCH64)]
    let irq_tail_probe_result = irq_tail_preemption::install();
    crate::hal::irq::mask_local();
    #[cfg(CONFIG_ARCH_AARCH64)]
    let guest_entry_irq_result = guest_entry_irq::run();
    #[cfg(any(CONFIG_ARCH_AARCH64, CONFIG_ARCH_X86_64))]
    if let Err(error) = reschedule_ipi_result {
        crate::kernel::boot::fail("reschedule IPI runtime proof", error);
    }
    if let Err(error) = result {
        crate::kernel::boot::fail("kernel scheduler/sync tests", error);
    }
    if let Err(error) = stack_result {
        crate::kernel::boot::fail("kernel stack-model tests", error);
    }
    if let Err(error) = sleep_result {
        crate::kernel::boot::fail("kernel thread-sleep tests", error);
    }
    if let Err(error) = migration_result {
        crate::kernel::boot::fail("kernel thread-migration tests", error);
    }
    if let Err(error) = wait_arbitration_result {
        crate::kernel::boot::fail("kernel wait-arbitration tests", error);
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
    #[cfg(CONFIG_ARCH_AARCH64)]
    if let Err(error) = irq_tail_probe_result {
        crate::kernel::boot::fail("AArch64 IRQ-tail preemption probe installation", error);
    }
    #[cfg(CONFIG_ARCH_AARCH64)]
    if let Err(error) = guest_entry_irq_result {
        crate::kernel::boot::fail("AArch64 guest-entry IRQ mask contract", error);
    }
    crate::println!("HypeR test: scheduler ready/wait queues and sleeping sync passed");
    crate::println!("HypeR test: guarded thread, IRQ, and emergency stacks passed");
    crate::println!("HypeR test: deadline-based thread sleep passed");
    crate::println!("HypeR test: race-safe wait arbitration passed");
    crate::println!("HypeR test: fatal-path readiness contract passed");
    #[cfg(CONFIG_ARCH_AARCH64)]
    crate::println!("HypeR test: AArch64 guest-entry IRQ mask contract passed");
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
