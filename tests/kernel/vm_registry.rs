// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Exercises rollback of unpublished VM and scheduler reservations.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Error {
    Device(crate::kernel::vm::device::Error),
    Interrupts(crate::hal::vm::InterruptError),
    Memory(crate::kernel::vm::memory::Error),
    Registry(crate::kernel::vm::registry::Error),
    Scheduler(crate::kernel::task::scheduler::Error),
    InitialContext,
    SchedulerThreadLeaked,
}

pub(super) fn run() -> Result<(), Error> {
    crate::kernel::vm::registry::verify_reservation_rollback().map_err(Error::Registry)?;
    if !crate::hal::vm::guest_execution_available() {
        return Ok(());
    }

    let before = crate::kernel::task::scheduler::statistics().map_err(Error::Scheduler)?;
    let mut reservation = crate::kernel::vm::registry::reserve().map_err(Error::Registry)?;
    let identifier = reservation.take_hardware_vmid().map_err(Error::Registry)?;
    let (ram_base, timer_interrupt) = crate::kernel::vm::linux::test_abi();
    let address_space = crate::kernel::vm::memory::GuestAddressSpace::new(
        identifier,
        ram_base,
        2 * hyper::mm::PAGE_SIZE,
    )
    .map_err(Error::Memory)?;
    let interrupts = crate::hal::vm::create_interrupt_controller(1, timer_interrupt)
        .map_err(Error::Interrupts)?;
    let devices = crate::kernel::vm::device::prepare().map_err(Error::Device)?;
    let builder = crate::kernel::vm::registry::VmBuilder::new(
        reservation,
        address_space,
        interrupts,
        devices,
        1,
    )
    .map_err(Error::Registry)?;
    let prepared = builder
        .prepare_boot_vcpu(
            0,
            crate::kernel::vm::linux::test_boot_context().ok_or(Error::InitialContext)?,
        )
        .map_err(Error::Scheduler)?;
    drop(prepared);
    let after = crate::kernel::task::scheduler::statistics().map_err(Error::Scheduler)?;
    if after.threads != before.threads {
        return Err(Error::SchedulerThreadLeaked);
    }

    if crate::hal::vm::try_administrative_stop().is_ok() {
        verify_dormant_vcpu_stop()?;
    }
    Ok(())
}

fn verify_dormant_vcpu_stop() -> Result<(), Error> {
    let mut reservation = crate::kernel::vm::registry::reserve().map_err(Error::Registry)?;
    let identifier = reservation.take_hardware_vmid().map_err(Error::Registry)?;
    let (ram_base, timer_interrupt) = crate::kernel::vm::linux::test_abi();
    let address_space = crate::kernel::vm::memory::GuestAddressSpace::new(
        identifier,
        ram_base,
        2 * hyper::mm::PAGE_SIZE,
    )
    .map_err(Error::Memory)?;
    let interrupts = crate::hal::vm::create_interrupt_controller(1, timer_interrupt)
        .map_err(Error::Interrupts)?;
    let devices = crate::kernel::vm::device::prepare().map_err(Error::Device)?;
    let builder = crate::kernel::vm::registry::VmBuilder::new(
        reservation,
        address_space,
        interrupts,
        devices,
        1,
    )
    .map_err(Error::Registry)?;
    let installed = builder
        .prepare_boot_vcpu(
            0,
            crate::kernel::vm::linux::test_boot_context().ok_or(Error::InitialContext)?,
        )
        .map_err(Error::Scheduler)?
        .install()
        .map_err(Error::Registry)?;
    crate::kernel::vm::registry::verify_dormant_vcpu_quiesce(installed).map_err(Error::Registry)
}
