/// Failure reported before issuing cache maintenance operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheError {
    AddressOverflow,
}

/// Architecture policy for cache maintenance by virtual address.
pub trait CacheMaintenance {
    fn data_line_size() -> usize;
    fn instruction_line_size() -> usize;

    /// Publishes dirty data to the platform's coherent memory domain.
    ///
    /// # Safety
    ///
    /// The complete rounded cache-line range must be mapped and readable. The
    /// caller must own the buffer and prevent concurrent CPU writes until the
    /// receiving agent has taken ownership. This primitive does not wait for
    /// a device transaction or replace a direction-aware DMA API.
    unsafe fn publish_data_range(start: usize, length: usize) -> Result<(), CacheError>;

    /// Discards cached data before observing writes from another agent.
    ///
    /// # Safety
    ///
    /// The caller must exclusively own every rounded cache line, have already
    /// established that the producing agent completed its writes, and ensure
    /// that discarding dirty data cannot corrupt adjacent objects. A barrier
    /// cannot by itself establish device completion.
    unsafe fn discard_data_range(start: usize, length: usize) -> Result<(), CacheError>;

    /// Publishes dirty data and then discards cached copies of the range.
    ///
    /// # Safety
    ///
    /// The complete rounded cache-line range must be mapped and exclusively
    /// owned for the duration of the operation.
    unsafe fn publish_and_discard_data_range(start: usize, length: usize)
    -> Result<(), CacheError>;

    /// Publishes newly written instructions to the instruction-coherence
    /// domain, but does not synchronize another CPU's execution pipeline.
    ///
    /// # Safety
    ///
    /// The range must be mapped, writable before this call, and protected from
    /// concurrent execution or modification until synchronization completes.
    /// Every CPU that can subsequently execute the range must perform
    /// `synchronize_instruction_execution` after observing publication.
    unsafe fn publish_instruction_range(start: usize, length: usize) -> Result<(), CacheError>;

    /// Performs the local context synchronization required before executing
    /// instructions published by another CPU.
    fn synchronize_instruction_execution();

    /// Invalidates instruction-cache entries throughout the kernel's
    /// instruction-coherence domain. Other CPUs still require a local context
    /// synchronization event before executing affected instructions.
    fn invalidate_instruction_all();
}
