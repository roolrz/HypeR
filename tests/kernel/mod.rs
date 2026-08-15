//! Bare-metal integration tests compiled only by `kernel-self-test`.

mod scheduler_sync;

pub(crate) fn run() {
    crate::arch::enable_local_irq();
    let result = scheduler_sync::run();
    crate::arch::disable_local_interrupts();
    if let Err(error) = result {
        crate::kernel::boot::fail("kernel scheduler/sync tests", error);
    }
    crate::println!("HypeR test: scheduler ready/wait queues and sleeping sync passed");
}
