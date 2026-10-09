// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native vm service calls under borrowed Process authority.

use super::DeferredProcessServices;
use super::affinity::AffinityInputError;
use crate::kernel::abi::native::VmServices;
use crate::kernel::capability::HandleValue;
use crate::kernel::mm::user_space::UserSlice;

impl VmServices for DeferredProcessServices<'_> {
    fn virtual_machine_platform_info(
        &self,
        lease: HandleValue,
        profile: u32,
    ) -> Result<
        crate::kernel::vm::service::VirtualMachinePlatformInfo,
        crate::kernel::vm::service::Error,
    > {
        crate::kernel::vm::service::platform_info(self.process, lease, profile)
    }

    fn derive_virtual_machine_creation_lease(
        &self,
        authority: HandleValue,
        domain: HandleValue,
    ) -> Result<HandleValue, crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::derive_creation_lease(self.process, authority, domain)
    }

    fn create_pending_virtual_machine(
        &self,
        lease: HandleValue,
        configuration: crate::kernel::vm::objects::VirtualMachineConfiguration,
    ) -> Result<HandleValue, crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::create_pending(self.process, lease, configuration)
    }

    fn set_pending_virtual_machine_memory(
        &self,
        pending: HandleValue,
        vmo: HandleValue,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::set_memory(self.process, pending, vmo)
    }

    fn set_pending_virtual_machine_bootstrap(
        &self,
        pending: HandleValue,
        bootstrap: crate::kernel::vm::objects::VirtualCpuBootstrap,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::set_bootstrap(self.process, pending, bootstrap)
    }

    fn seal_pending_virtual_machine(
        &self,
        pending: HandleValue,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::seal(self.process, pending)
    }

    fn install_pending_virtual_machine(
        &self,
        pending: HandleValue,
    ) -> Result<[HandleValue; 2], crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::install(self.process, pending)
    }

    fn abort_pending_virtual_machine(
        &self,
        pending: HandleValue,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::abort(self.process, pending)
    }

    fn request_virtual_machine_stop(
        &self,
        machine: HandleValue,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::request_stop(self.process, machine)
    }

    fn register_mmio(
        &self,
        machine: HandleValue,
        base: u64,
        length: u64,
        device: u64,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::register_mmio(self.process, machine, base, length, device)
    }
    fn register_mmio_event(
        &self,
        machine: HandleValue,
        base: u64,
        length: u64,
        device: u64,
        event: HandleValue,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::register_mmio_event(
            self.process,
            machine,
            base,
            length,
            device,
            event,
        )
    }
    fn bind_firmware_console(
        &self,
        machine: HandleValue,
        device: u64,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::bind_firmware_console(self.process, machine, device)
    }
    fn set_device_interrupt(
        &self,
        machine: HandleValue,
        interrupt: u32,
        asserted: bool,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::set_device_interrupt(self.process, machine, interrupt, asserted)
    }
    fn pending_mmio(
        &self,
        vcpu: HandleValue,
        device: u64,
    ) -> Result<Option<hyper::vm::device::mmio::Request>, crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::pending_mmio(self.process, vcpu, device)
    }
    fn complete_mmio(
        &self,
        vcpu: HandleValue,
        id: u64,
        action: hyper::vm::exit::MmioAction,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::complete_mmio(self.process, vcpu, id, action)
    }
    fn create_guest_memory(
        &self,
        vmo: HandleValue,
    ) -> Result<HandleValue, crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::create_guest_memory(self.process, vmo)
    }
    fn map_guest_memory(
        &self,
        pending: HandleValue,
        memory: HandleValue,
        guest_offset: u64,
        source_offset: u64,
        length: u64,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::map_guest_memory(
            self.process,
            pending,
            memory,
            guest_offset,
            source_offset,
            length,
        )
    }
    fn pending_power_request(
        &self,
        machine: HandleValue,
    ) -> Result<Option<hyper::vm::arm::psci::Request>, crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::pending_power_request(self.process, machine)
    }
    fn complete_power_request(
        &self,
        machine: HandleValue,
        request_id: u64,
        accept: bool,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::complete_power_request(
            self.process,
            machine,
            request_id,
            accept,
        )
    }
    fn open_vcpu(
        &self,
        machine: HandleValue,
        vcpu: u32,
    ) -> Result<HandleValue, crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::open_vcpu(self.process, machine, vcpu)
    }
    fn virtual_machine_info(
        &self,
        machine: HandleValue,
    ) -> Result<
        (
            crate::kernel::vm::objects::VirtualMachineConfiguration,
            crate::kernel::vm::objects::VirtualMachineSnapshot,
        ),
        crate::kernel::vm::service::Error,
    > {
        crate::kernel::vm::service::machine_info(self.process, machine)
    }

    fn virtual_cpu_info(
        &self,
        vcpu: HandleValue,
    ) -> Result<crate::kernel::vm::objects::VirtualCpuSnapshot, crate::kernel::vm::service::Error>
    {
        crate::kernel::vm::service::vcpu_info(self.process, vcpu)
    }

    fn set_virtual_cpu_affinity(
        &self,
        vcpu: HandleValue,
        words: Option<UserSlice>,
        word_count: usize,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        let affinity = self
            .copy_affinity(words, word_count)
            .map_err(|error| match error {
                AffinityInputError::Invalid => crate::kernel::vm::service::Error::InvalidArgument,
                AffinityInputError::Memory(error) => crate::kernel::vm::service::Error::from(error),
            })?;
        crate::kernel::vm::service::set_vcpu_affinity(self.process, vcpu, affinity)
    }

    fn start_virtual_cpu(
        &self,
        vcpu: HandleValue,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::start_vcpu(self.process, vcpu)
    }
}
