// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Exercises rollback of unpublished VM and scheduler reservations.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Error {
    Device(crate::kernel::vm::device::Error),
    Interrupts(crate::arch::vm::InterruptError),
    Memory(crate::kernel::vm::memory::Error),
    Registry(crate::kernel::vm::registry::Error),
    Scheduler(crate::kernel::task::scheduler::Error),
    SchedulerThreadLeaked,
}

pub(super) fn run() -> Result<(), Error> {
    crate::kernel::vm::registry::verify_reservation_rollback().map_err(Error::Registry)?;
    if !crate::arch::vm::guest_execution_available() {
        return Ok(());
    }

    let before = crate::kernel::task::scheduler::statistics().map_err(Error::Scheduler)?;
    let reservation = crate::kernel::vm::registry::reserve().map_err(Error::Registry)?;
    let address_space = crate::kernel::vm::memory::GuestAddressSpace::new(
        reservation.hardware_vmid(),
        crate::arch::guest::linux_abi().ram_base().get(),
        2 * hyper::mm::PAGE_SIZE,
    )
    .map_err(Error::Memory)?;
    let interrupts = crate::kernel::vm::VmInterruptController::new(
        1,
        crate::arch::guest::linux_abi().timer_interrupt(),
    )
    .map_err(Error::Interrupts)?;
    let devices = crate::kernel::vm::device::prepare().map_err(Error::Device)?;
    let builder = crate::kernel::vm::registry::VmBuilder::new(
        reservation,
        address_space,
        interrupts,
        devices,
    )
    .map_err(Error::Registry)?;
    let prepared = builder
        .prepare_boot_vcpu(0, crate::arch::guest::prepare_linux_vcpu_context())
        .map_err(Error::Scheduler)?;
    drop(prepared);
    let after = crate::kernel::task::scheduler::statistics().map_err(Error::Scheduler)?;
    if after.threads != before.threads {
        return Err(Error::SchedulerThreadLeaked);
    }
    Ok(())
}
