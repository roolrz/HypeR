//! Bare-metal integration tests compiled only by `kernel-self-test`.

mod scheduler_sync;
mod stack_model;

pub(crate) fn run() {
    crate::arch::enable_local_irq();
    let result = scheduler_sync::run();
    let stack_result = stack_model::run();
    crate::arch::disable_local_interrupts();
    if let Err(error) = result {
        crate::kernel::boot::fail("kernel scheduler/sync tests", error);
    }
    if let Err(error) = stack_result {
        crate::kernel::boot::fail("kernel stack-model tests", error);
    }
    crate::println!("HypeR test: scheduler ready/wait queues and sleeping sync passed");
    crate::println!("HypeR test: guarded thread, IRQ, and emergency stacks passed");
}
