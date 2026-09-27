// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Concrete Native service adapters over borrowed Process and Thread authority.

mod affinity;
mod console;
mod inspect;
mod ipc;
mod memory;
mod object;
mod process_builder;
mod task;
mod vfs;

use crate::kernel::abi::native::{
    HandleServices, ImmediateServices, UserMemoryServices, VmServices,
};
use crate::kernel::capability::{HandleInfo, HandleValue, Rights};
use crate::kernel::mm::user_space::UserSlice;
use crate::kernel::process::{Process, ProcessError, UserThread};

use self::affinity::AffinityInputError;

/// Borrowed syscall authority; contains no machine-run or return-token state.
pub(super) struct DeferredProcessServices<'process> {
    process: &'process Process,
    thread: &'process UserThread,
}

impl<'process> DeferredProcessServices<'process> {
    pub(super) fn new(process: &'process Process, thread: &'process UserThread) -> Self {
        Self { process, thread }
    }
}

impl UserMemoryServices for DeferredProcessServices<'_> {
    fn copy_to_user(&self, destination: UserSlice, source: &[u8]) -> Result<(), ProcessError> {
        self.process.copy_to_user(destination, source)
    }

    fn copy_from_user(
        &self,
        source: UserSlice,
        destination: &mut [u8],
    ) -> Result<(), ProcessError> {
        self.process.copy_from_user(source, destination)
    }
}

impl DeferredProcessServices<'_> {
    // Keep the full batch buffer off the interactive input call path. Both
    // variants copy once before publishing any input and kick the guest once.
    // A full 4 KiB write still needs its complete scratch buffer: shortening
    // that batch would change partial-write behavior and increase syscall cost.
    #[inline(never)]
    fn copy_virtual_serial_input<const CAPACITY: usize>(
        &self,
        serial: &crate::kernel::vm::virtual_serial::VirtualSerial,
        source: UserSlice,
        length: usize,
    ) -> Result<usize, crate::kernel::vm::service::Error> {
        let mut bytes = [0; CAPACITY];
        let bytes = bytes
            .get_mut(..length)
            .ok_or(crate::kernel::vm::service::Error::InvalidArgument)?;
        self.process.copy_from_user(source, bytes)?;
        serial
            .write_input(bytes)
            .map_err(crate::kernel::vm::objects::Error::from)
            .map_err(Into::into)
    }
}

impl ImmediateServices for DeferredProcessServices<'_> {
    fn close_handle(&self, value: HandleValue) -> Result<(), ProcessError> {
        self.process.close_handle(value)
    }
}

impl HandleServices for DeferredProcessServices<'_> {
    fn handle_info(
        &self,
        value: HandleValue,
        required_rights: Rights,
    ) -> Result<HandleInfo, ProcessError> {
        self.process.handle_info(value, required_rights)
    }

    fn duplicate_handle(
        &self,
        value: HandleValue,
        rights: Rights,
    ) -> Result<HandleValue, ProcessError> {
        self.process.duplicate_handle(value, rights)
    }
    fn replace_handle(
        &self,
        value: HandleValue,
        rights: Rights,
    ) -> Result<HandleValue, ProcessError> {
        self.process.replace_handle(value, rights)
    }
}

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

    fn set_pending_virtual_machine_virtual_serial(
        &self,
        pending: HandleValue,
        serial: HandleValue,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::set_virtual_serial(self.process, pending, serial)
    }

    fn create_virtual_serial(&self) -> Result<HandleValue, crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::create_virtual_serial(self.process)
    }

    fn register_virtual_serial_output(
        &self,
        serial: HandleValue,
        buffer: HandleValue,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::register_virtual_serial_output(self.process, serial, buffer)
    }
    fn acknowledge_virtual_serial_output(
        &self,
        serial: HandleValue,
        consumed: u64,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::acknowledge_virtual_serial_output(
            self.process,
            serial,
            consumed,
        )
    }
    fn write_virtual_serial(
        &self,
        value: HandleValue,
        source: Option<UserSlice>,
    ) -> Result<usize, crate::kernel::vm::service::Error> {
        let serial = self
            .process
            .resolve_handle::<crate::kernel::vm::virtual_serial::VirtualSerial>(
                value,
                Rights::WRITE,
            )?;
        let Some(source) = source else {
            return Ok(0);
        };
        let length = usize::try_from(source.length())
            .map_err(|_| crate::kernel::vm::service::Error::InvalidArgument)?
            .min(crate::kernel::vm::virtual_serial::TRANSFER_BATCH_BYTES);
        let length_bytes =
            u64::try_from(length).map_err(|_| crate::kernel::vm::service::Error::Internal)?;
        let source = UserSlice::new(source.base(), length_bytes)
            .map_err(|_| crate::kernel::vm::service::Error::Fault)?;
        if length <= 128 {
            self.copy_virtual_serial_input::<128>(serial.object(), source, length)
        } else {
            self.copy_virtual_serial_input::<
                { crate::kernel::vm::virtual_serial::TRANSFER_BATCH_BYTES },
            >(serial.object(), source, length)
        }
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
    fn pending_mmio(
        &self,
        vcpu: HandleValue,
    ) -> Result<Option<hyper::vm::device::mmio::Request>, crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::pending_mmio(self.process, vcpu)
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

impl crate::kernel::abi::native::DeviceServices for DeferredProcessServices<'_> {
    fn device_firmware_read(
        &self,
        authority: HandleValue,
        node: u32,
        field: u32,
        name: &str,
    ) -> Result<alloc::vec::Vec<u8>, crate::kernel::device::assigned::service::MatchError> {
        crate::kernel::device::assigned::service::firmware_read(
            self.process,
            authority,
            node,
            field,
            name,
        )
    }
    fn device_claim_bundle(
        &self,
        authority: HandleValue,
        entries: &[(u32, u32, u64)],
        irq_node: u32,
    ) -> Result<HandleValue, crate::kernel::vm::service::Error> {
        crate::kernel::device::assigned::service::claim_bundle(
            self.process,
            authority,
            entries,
            irq_node,
        )
    }
    fn device_mmio(
        &self,
        device: HandleValue,
        offset: u64,
        width: u32,
        write: bool,
        value: u64,
    ) -> Result<u64, crate::kernel::vm::service::Error> {
        crate::kernel::device::assigned::service::mmio(
            self.process,
            device,
            offset,
            width,
            write,
            value,
        )
    }
    fn device_irq_pending(
        &self,
        device: HandleValue,
    ) -> Result<u64, crate::kernel::vm::service::Error> {
        crate::kernel::device::assigned::service::irq_pending(self.process, device)
    }
    fn device_irq_complete(
        &self,
        device: HandleValue,
        sequence: u64,
        asserted: bool,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::device::assigned::service::irq_complete(
            self.process,
            device,
            sequence,
            asserted,
        )
    }

    fn device_profile_info(
        &self,
        device: HandleValue,
    ) -> Result<[u8; 32], crate::kernel::vm::service::Error> {
        crate::kernel::device::assigned::service::profile_info(self.process, device)
    }
    fn device_resource_info(
        &self,
        device: HandleValue,
        index: u32,
    ) -> Result<[u8; 32], crate::kernel::vm::service::Error> {
        crate::kernel::device::assigned::service::resource_info(self.process, device, index)
    }
    fn claim_device_matching(
        &self,
        authority: HandleValue,
        profile: u32,
        identity_kind: u32,
        identity: &str,
    ) -> Result<HandleValue, crate::kernel::device::assigned::service::MatchError> {
        crate::kernel::device::assigned::service::claim_matching(
            self.process,
            authority,
            profile,
            identity_kind,
            identity,
        )
    }
    fn claim_device(
        &self,
        authority: HandleValue,
        index: u32,
    ) -> Result<HandleValue, crate::kernel::vm::service::Error> {
        crate::kernel::device::assigned::service::claim(self.process, authority, index)
    }
    fn physical_device_info(
        &self,
        device: HandleValue,
    ) -> Result<crate::kernel::device::assigned::Info, crate::kernel::vm::service::Error> {
        crate::kernel::device::assigned::service::info(self.process, device)
    }
    fn vmo_dma_extent(
        &self,
        authority: HandleValue,
        vmo: HandleValue,
        offset: u64,
        length: u64,
    ) -> Result<crate::kernel::device::assigned::DmaExtent, crate::kernel::vm::service::Error> {
        crate::kernel::device::assigned::service::dma_extent(
            self.process,
            authority,
            vmo,
            offset,
            length,
        )
    }
    fn assign_physical_device(
        &self,
        pending: HandleValue,
        device: HandleValue,
        base: u64,
        irq: u32,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::device::assigned::service::assign(self.process, pending, device, base, irq)
    }
}
impl crate::kernel::abi::native::GuestIoServices for DeferredProcessServices<'_> {
    fn create_guest_mapping(
        &self,
        backend: HandleValue,
        memory: HandleValue,
        frontend: u64,
    ) -> Result<(HandleValue, u64), crate::kernel::vm::service::Error> {
        crate::kernel::vm::create_guest_mapping(self.process, backend, memory, frontend)
    }
    fn release_guest_mapping(
        &self,
        mapping: HandleValue,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::release_guest_mapping(self.process, mapping)
    }
    fn create_native_block(
        &self,
        memory: HandleValue,
        backend: HandleValue,
        guest_base: u64,
        notification_base: u64,
        notification_irq: u32,
    ) -> Result<HandleValue, crate::kernel::vm::service::Error> {
        crate::kernel::block::service::create(
            self.process,
            memory,
            backend,
            guest_base,
            notification_base,
            notification_irq,
        )
    }
    fn activate_native_block(
        &self,
        block: HandleValue,
        readonly: bool,
    ) -> Result<u64, crate::kernel::block::service::ActivationError> {
        crate::kernel::block::service::activate(self.process, block, readonly)
    }
    fn mount_native_block(
        &self,
        block: HandleValue,
        directory: HandleValue,
        path: UserSlice,
    ) -> Result<(), crate::kernel::vfs::VfsServiceError> {
        crate::kernel::vfs::service::mount_block(self.process, block, directory, path)
    }

    fn create_guest_mailbox(
        &self,
        machine: HandleValue,
        base: u64,
        irq: u32,
    ) -> Result<HandleValue, crate::kernel::vm::service::Error> {
        crate::kernel::vm::io::service::create_mailbox(self.process, machine, base, irq)
    }
    fn send_guest_mailbox(
        &self,
        mailbox: HandleValue,
        bytes: &[u8],
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::io::service::send_mailbox(self.process, mailbox, bytes)
    }
    fn receive_guest_mailbox(
        &self,
        mailbox: HandleValue,
        copy: &mut dyn FnMut(&[u8]) -> Result<(), crate::kernel::vm::service::Error>,
    ) -> Result<usize, crate::kernel::vm::service::Error> {
        crate::kernel::vm::io::service::receive_mailbox(self.process, mailbox, copy)
    }
    fn create_guest_notification(
        &self,
        frontend: HandleValue,
        backend: HandleValue,
        frontend_base: u64,
        backend_base: u64,
        frontend_irq: u32,
        backend_irq: u32,
    ) -> Result<HandleValue, crate::kernel::vm::service::Error> {
        crate::kernel::vm::io::service::create_notification(
            self.process,
            frontend,
            backend,
            frontend_base,
            backend_base,
            frontend_irq,
            backend_irq,
        )
    }
    fn control_guest_notification(
        &self,
        notification: HandleValue,
        operation: u32,
    ) -> Result<u32, crate::kernel::vm::service::Error> {
        crate::kernel::vm::io::service::control_notification(self.process, notification, operation)
    }
}
