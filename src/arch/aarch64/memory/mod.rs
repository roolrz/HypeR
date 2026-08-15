mod address_space;
mod layout;
mod page_table;

pub use address_space::{ActivationContext, Error, PreparedAddressSpace, StackMapping, prepare};
pub use layout::Aarch64AddressTranslation;
pub(super) use layout::KERNEL_BASE;

/// Returns the permanent bootstrap-stack bounds when `stack_pointer` lies in it.
pub fn bootstrap_stack_bounds(stack_pointer: u64) -> Option<(usize, usize)> {
    let bottom =
        usize::try_from(layout::KERNEL_STACK_BASE.checked_add(hyper::mm::PAGE_SIZE)?).ok()?;
    let size = page_table::KERNEL_STACK_PAGES.checked_mul(hyper::mm::PAGE_SIZE as usize)?;
    let top = bottom.checked_add(size)?;
    (bottom <= stack_pointer as usize && stack_pointer as usize <= top).then_some((bottom, top))
}
