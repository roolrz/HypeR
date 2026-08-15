//! Thread representation and scheduling policy.

pub mod scheduler;
pub mod thread;

/// Creates the bootstrap scheduling context and initial run queue.
pub(crate) fn initialize() {
    let capabilities = match scheduler::initialize() {
        Ok(capabilities) => capabilities,
        Err(error) => super::boot::fail("scheduler initialization", error),
    };
    crate::println!(
        "HypeR: scheduler active on bootstrap thread {}",
        capabilities.bootstrap_thread.get()
    );
}
