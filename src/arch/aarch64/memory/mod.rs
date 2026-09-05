// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

mod address_space;
mod layout;
mod page_table;

#[cfg(CONFIG_CRASH_CONSOLE)]
pub use address_space::inspect_mapping;
pub use address_space::{
    ActivationContext, Error, PreparedAddressSpace, SecondaryActivationContext, StackMapping,
    prepare,
};
pub use layout::Aarch64AddressTranslation;

pub(super) fn kernel_region_base() -> u64 {
    layout::selected().kernel_base
}

pub(super) fn linear_mapping_base() -> u64 {
    layout::selected().linear_base
}

/// Returns the permanent bootstrap-stack bounds when `stack_pointer` lies in it.
pub fn bootstrap_stack_bounds(stack_pointer: u64) -> Option<(usize, usize)> {
    let bottom = usize::try_from(
        layout::selected()
            .kernel_stack_base
            .checked_add(hyper::mm::PAGE_SIZE)?,
    )
    .ok()?;
    let size = page_table::KERNEL_STACK_PAGES.checked_mul(hyper::mm::PAGE_SIZE as usize)?;
    let top = bottom.checked_add(size)?;
    (bottom <= stack_pointer as usize && stack_pointer as usize <= top).then_some((bottom, top))
}
