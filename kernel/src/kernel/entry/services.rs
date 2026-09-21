// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Concrete Native service adapters over borrowed Process and Thread authority.

mod vfs;

use alloc::vec::Vec;

use crate::kernel::abi::native::{
    ConsoleServiceError, ConsoleServices, HandleServices, HierarchyServices, ImmediateServices,
    InspectServices, IpcServices, MemoryServices, ObjectServiceError, ObjectServices,
    ProcessBuilderServiceError, ProcessBuilderServices, SystemInspectServices, TaskServices,
    UserMemoryServices, VmServices,
};
use crate::kernel::accounting::{
    CommittedCharge, ResourceAmount, ResourceDomainObject, ResourceKind,
};
use crate::kernel::capability::{HandleInfo, HandleValue, ResolvedWaitable, Rights};
use crate::kernel::inspect::{CpuInspector, MemoryInspector, ObjectInspector, TaskInspector};
use crate::kernel::ipc::{
    ByteChannelReadOutcome, ByteChannelServiceError, CapabilityChannelServiceError,
    CapabilityReceiveOutcome,
};
use crate::kernel::mm::user_space::UserAddress;
use crate::kernel::mm::user_space::UserSlice;
use crate::kernel::object::{
    self, Event, KernelObject, ObjectKind, SignalWaitManyOutcome, SignalWaitOutcome,
    SignalWaitRequest,
};
use crate::kernel::process::{
    Process, ProcessBuilder, ProcessError, ProcessObject, ProcessSnapshot, StartupCapability,
    TaskGroupObject, TerminalReason, UserThread, UserThreadPhase,
};
use crate::kernel::task::scheduler::CpuMask;

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

struct ChargedBuilderInput {
    bytes: Vec<u8>,
    _charge: CommittedCharge,
}

struct ResolvedWaitEntry {
    object: ResolvedWaitable,
    signals: u64,
}

impl ChargedBuilderInput {
    fn as_bytes(&self) -> &[u8] {
        &self.bytes
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

    fn copy_builder_input(
        &self,
        input: Option<UserSlice>,
    ) -> Result<ChargedBuilderInput, ProcessBuilderServiceError> {
        let length = input.map_or(0, UserSlice::length);
        let length =
            usize::try_from(length).map_err(|_| ProcessBuilderServiceError::InvalidInput)?;
        let charge = self
            .process
            .resource_domain()
            .reserve(ResourceAmount::ZERO.with(
                ResourceKind::KernelMemoryBytes,
                u64::try_from(length).map_err(|_| ProcessBuilderServiceError::InvalidInput)?,
            ))
            .map_err(ProcessError::from)?
            .commit();
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| ProcessBuilderServiceError::Process(ProcessError::Allocation))?;
        bytes.resize(length, 0);
        if let Some(input) = input {
            self.process.copy_from_user(input, &mut bytes)?;
        }
        Ok(ChargedBuilderInput {
            bytes,
            _charge: charge,
        })
    }

    fn copy_affinity(
        &self,
        input: Option<UserSlice>,
        word_count: usize,
    ) -> Result<CpuMask, ProcessBuilderServiceError> {
        const WORD_BYTES: usize = core::mem::size_of::<u64>();
        const MAX_WORDS: usize =
            hyper::abi::native::HYPER_NATIVE_PROCESS_AFFINITY_MAX_WORDS as usize;

        if word_count > MAX_WORDS {
            return Err(ProcessBuilderServiceError::InvalidInput);
        }
        let byte_count = word_count
            .checked_mul(WORD_BYTES)
            .ok_or(ProcessBuilderServiceError::InvalidInput)?;
        let mut encoded = [0_u8; MAX_WORDS * WORD_BYTES];
        if let Some(input) = input {
            if input.length() != byte_count as u64 {
                return Err(ProcessBuilderServiceError::InvalidInput);
            }
            self.process
                .copy_from_user(input, &mut encoded[..byte_count])?;
        } else if byte_count != 0 {
            return Err(ProcessBuilderServiceError::InvalidInput);
        }

        let mut words = [0_u64; MAX_WORDS];
        for (index, destination) in words[..word_count].iter_mut().enumerate() {
            let offset = index * WORD_BYTES;
            let mut word = [0_u8; WORD_BYTES];
            word.copy_from_slice(&encoded[offset..offset + WORD_BYTES]);
            *destination = u64::from_le_bytes(word);
        }
        for cpu in hyper::cpu::MAX_CPUS..word_count * u64::BITS as usize {
            if words[cpu / u64::BITS as usize] & (1_u64 << (cpu % u64::BITS as usize)) != 0 {
                return Err(ProcessBuilderServiceError::InvalidInput);
            }
        }
        let mut affinity = CpuMask::EMPTY;
        for cpu in 0..hyper::cpu::MAX_CPUS {
            if words[cpu / u64::BITS as usize] & (1_u64 << (cpu % u64::BITS as usize)) == 0 {
                continue;
            }
            let Some(cpu) = hyper::cpu::CpuIndex::new(cpu) else {
                return Err(ProcessBuilderServiceError::InvalidInput);
            };
            affinity = affinity.with_cpu(cpu);
        }
        Ok(affinity)
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

impl HierarchyServices for DeferredProcessServices<'_> {
    fn create_resource_domain(
        &self,
        parent: HandleValue,
        limits: crate::kernel::accounting::ResourceLimits,
    ) -> Result<HandleValue, crate::kernel::process::hierarchy::Error> {
        crate::kernel::process::hierarchy::create_resource_domain(self.process, parent, limits)
    }

    fn create_task_group(
        &self,
        factory: HandleValue,
        domain: HandleValue,
    ) -> Result<HandleValue, crate::kernel::process::hierarchy::Error> {
        crate::kernel::process::hierarchy::create_task_group(self.process, factory, domain)
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

    fn start_virtual_cpu(
        &self,
        vcpu: HandleValue,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::start_vcpu(self.process, vcpu)
    }
}

impl SystemInspectServices for DeferredProcessServices<'_> {
    fn memory_observation(
        &self,
        inspector: HandleValue,
    ) -> Result<crate::kernel::inspect::MemoryObservation, crate::kernel::inspect::Error> {
        let inspector = self
            .process
            .resolve_handle::<MemoryInspector>(inspector, Rights::INSPECT)
            .map_err(crate::kernel::inspect::Error::Process)?;
        inspector.object().snapshot()
    }

    fn cpu_observation(
        &self,
        inspector: HandleValue,
    ) -> Result<crate::kernel::task::scheduler::CpuTimeSnapshot, crate::kernel::inspect::Error>
    {
        let inspector = self
            .process
            .resolve_handle::<CpuInspector>(inspector, Rights::INSPECT)
            .map_err(crate::kernel::inspect::Error::Process)?;
        Ok(inspector.object().snapshot())
    }
}

impl ObjectServices for DeferredProcessServices<'_> {
    fn current_process_id(&self) -> u64 {
        self.process.koid().get()
    }
    fn wait_set_create(&self, capacity: usize) -> Result<HandleValue, ObjectServiceError> {
        let set = object::WaitSet::try_new(capacity, &self.process.resource_domain())?;
        Ok(self.process.create_object(
            set,
            Rights::DUPLICATE
                .union(Rights::WAIT)
                .union(Rights::BIND_WAIT)
                .union(Rights::INSPECT),
        )?)
    }
    fn wait_set_add(
        &self,
        set: HandleValue,
        source: HandleValue,
        signals: u64,
    ) -> Result<u64, ObjectServiceError> {
        let set = self
            .process
            .resolve_handle::<object::WaitSet>(set, Rights::BIND_WAIT)?;
        let source = self.process.resolve_waitable(source, Rights::WAIT)?;
        Ok(set
            .object()
            .add(source, signals, &self.process.resource_domain())?)
    }
    fn wait_set_rearm(
        &self,
        set: HandleValue,
        registration: u64,
    ) -> Result<(), ObjectServiceError> {
        let set = self
            .process
            .resolve_handle::<object::WaitSet>(set, Rights::BIND_WAIT)?;
        Ok(set.object().rearm(registration)?)
    }
    fn wait_set_remove(
        &self,
        set: HandleValue,
        registration: u64,
    ) -> Result<(), ObjectServiceError> {
        let set = self
            .process
            .resolve_handle::<object::WaitSet>(set, Rights::BIND_WAIT)?;
        Ok(set.object().remove(registration)?)
    }
    fn wait_set_wait(
        &self,
        set: HandleValue,
        deadline: u64,
        output: UserSlice,
    ) -> Result<(), ObjectServiceError> {
        let set = self
            .process
            .resolve_handle::<object::WaitSet>(set, Rights::WAIT)?;
        let delivery = set
            .object()
            .wait(deadline, &self.process.resource_domain(), || {
                self.thread.snapshot().phase == UserThreadPhase::StopRequested
            })?;
        self.process.copy_to_user(output, &delivery.record())?;
        delivery.complete();
        Ok(())
    }

    fn create_event(&self) -> Result<HandleValue, ObjectServiceError> {
        let event = Event::try_new(&self.process.resource_domain())?;
        Ok(self
            .process
            .create_object(event, <Event as KernelObject>::SUPPORTED_RIGHTS)?)
    }

    fn signal_event(
        &self,
        value: HandleValue,
        clear: u64,
        set: u64,
    ) -> Result<(), ObjectServiceError> {
        let event = self
            .process
            .resolve_handle::<Event>(value, Rights::SIGNAL)?;
        event.object().signal(clear, set)?;
        Ok(())
    }

    fn wait_one(
        &self,
        value: HandleValue,
        requested: u64,
        deadline: u64,
    ) -> Result<SignalWaitOutcome, ObjectServiceError> {
        let resolved = self.process.resolve_waitable(value, Rights::WAIT)?;
        let domain = self.process.resource_domain();
        Ok(object::wait_one(
            resolved.source(),
            &domain,
            requested,
            deadline,
            || self.thread.snapshot().phase == UserThreadPhase::StopRequested,
        )?)
    }

    fn wait_many(
        &self,
        items: UserSlice,
        item_count: usize,
        deadline: u64,
    ) -> Result<SignalWaitManyOutcome, ObjectServiceError> {
        let record_size = core::mem::size_of::<hyper::abi::native::HyperNativeObjectWaitItem>();
        let input_bytes = item_count
            .checked_mul(record_size)
            .ok_or(ObjectServiceError::InvalidInput)?;
        if items.length() != u64::try_from(input_bytes).map_err(|_| ProcessError::Allocation)? {
            return Err(ObjectServiceError::InvalidInput);
        }
        let scratch_bytes = input_bytes
            .checked_add(
                item_count
                    .checked_mul(
                        core::mem::size_of::<ResolvedWaitEntry>()
                            + core::mem::size_of::<SignalWaitRequest<'_>>(),
                    )
                    .ok_or(ProcessError::Allocation)?,
            )
            .ok_or(ProcessError::Allocation)?;
        let _scratch_charge = self
            .process
            .resource_domain()
            .reserve(ResourceAmount::ZERO.with(
                ResourceKind::KernelMemoryBytes,
                u64::try_from(scratch_bytes).map_err(|_| ProcessError::Allocation)?,
            ))
            .map_err(ProcessError::from)?
            .commit();

        let mut encoded = Vec::new();
        encoded
            .try_reserve_exact(input_bytes)
            .map_err(|_| ProcessError::Allocation)?;
        encoded.resize(input_bytes, 0);
        self.process.copy_from_user(items, &mut encoded)?;

        let mut resolved = Vec::new();
        resolved
            .try_reserve_exact(item_count)
            .map_err(|_| ProcessError::Allocation)?;
        for record in encoded.chunks_exact(record_size) {
            let (raw_handle, signals) = decode_wait_item(record)?;
            let handle = HandleValue::try_from_raw(raw_handle).map_err(ProcessError::from)?;
            resolved.push(ResolvedWaitEntry {
                object: self.process.resolve_waitable(handle, Rights::WAIT)?,
                signals,
            });
        }

        let mut requests = Vec::new();
        requests
            .try_reserve_exact(item_count)
            .map_err(|_| ProcessError::Allocation)?;
        for entry in &resolved {
            requests.push(SignalWaitRequest::new(entry.object.source(), entry.signals));
        }
        let domain = self.process.resource_domain();
        Ok(object::wait_many(&requests, &domain, deadline, || {
            self.thread.snapshot().phase == UserThreadPhase::StopRequested
        })?)
    }
}

impl TaskServices for DeferredProcessServices<'_> {
    fn create_thread(
        &self,
        entry: u64,
        stack: u64,
        tls: u64,
        argument: u64,
    ) -> Result<HandleValue, ObjectServiceError> {
        let start = crate::kernel::process::UserThreadStart::try_new(
            UserAddress::new(entry),
            UserAddress::new(stack),
            UserAddress::new(tls),
        )
        .map_err(|_| ObjectServiceError::InvalidInput)?
        .with_argument(argument);
        let process = self.process;
        let thread = process.create_user_thread("native-worker", start, CpuMask::ALL)?;
        let rights = Rights::DUPLICATE
            .union(Rights::WAIT)
            .union(Rights::INSPECT)
            .union(Rights::START)
            .union(Rights::REQUEST_STOP);
        match process.publish_thread_handle(&thread, rights) {
            Ok(handle) => Ok(handle),
            Err(error) => {
                if let Some(id) = thread.scheduler_id() {
                    crate::kernel::task::scheduler::request_user_thread_stop(
                        id,
                        TerminalReason::Requested,
                    )
                    .map_err(ProcessError::from)?;
                }
                Err(error.into())
            }
        }
    }
    fn start_thread(&self, value: HandleValue) -> Result<(), ProcessError> {
        let thread = self
            .process
            .resolve_user_thread_handle(value, Rights::START)?;
        thread.ready()?;
        Ok(())
    }
    fn stop_thread(&self, value: HandleValue) -> Result<(), ProcessError> {
        let thread = self
            .process
            .resolve_user_thread_handle(value, Rights::REQUEST_STOP)?;
        if thread.snapshot().phase == UserThreadPhase::Detached {
            return Ok(());
        }
        if let Some(id) = thread.scheduler_id() {
            match crate::kernel::task::scheduler::request_user_thread_stop(
                id,
                TerminalReason::Requested,
            ) {
                Ok(()) => {}
                Err(crate::kernel::task::scheduler::Error::ThreadNotFound)
                    if thread.snapshot().phase == UserThreadPhase::Detached => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }
    fn atomic_wait(
        &self,
        address: u64,
        expected: u32,
        deadline: u64,
    ) -> Result<crate::kernel::task::WaitOutcome, ObjectServiceError> {
        crate::kernel::process::atomic_wait::wait(self.process, address, expected, deadline, || {
            self.thread.snapshot().phase == UserThreadPhase::StopRequested
        })
        .map_err(atomic_wait_error)
    }
    fn atomic_wake(&self, address: u64, count: u32) -> Result<u64, ObjectServiceError> {
        crate::kernel::process::atomic_wait::wake(self.process, address, count)
            .map_err(atomic_wait_error)
    }
    fn sleep_thread(
        &self,
        deadline: u64,
    ) -> Result<crate::kernel::task::WaitOutcome, ObjectServiceError> {
        crate::kernel::process::atomic_wait::sleep(self.process, deadline, || {
            self.thread.snapshot().phase == UserThreadPhase::StopRequested
        })
        .map_err(atomic_wait_error)
    }

    fn process_info(&self, process: HandleValue) -> Result<ProcessSnapshot, ProcessError> {
        Ok(self
            .process
            .resolve_handle::<ProcessObject>(process, Rights::INSPECT)?
            .object()
            .snapshot())
    }

    fn request_process_stop(&self, process: HandleValue) -> Result<(), ProcessError> {
        let process = self
            .process
            .resolve_handle::<ProcessObject>(process, Rights::REQUEST_STOP)?;
        process.object().request_stop(TerminalReason::Requested);
        Ok(())
    }
}

impl InspectServices for DeferredProcessServices<'_> {
    fn scan_processes(
        &self,
        inspector: HandleValue,
        cursor: u64,
        output: &mut crate::kernel::inspect::Page<
            ProcessSnapshot,
            { crate::kernel::inspect::PROCESS_PAGE_CAPACITY },
        >,
    ) -> Result<(), crate::kernel::inspect::Error> {
        let inspector = self
            .process
            .resolve_handle::<TaskInspector>(inspector, Rights::INSPECT)
            .map_err(crate::kernel::inspect::Error::Process)?;
        inspector.object().scan_processes(cursor, output)
    }

    fn scan_threads(
        &self,
        inspector: HandleValue,
        cursor: u64,
        output: &mut crate::kernel::inspect::Page<
            crate::kernel::inspect::TaskThreadSnapshot,
            { crate::kernel::inspect::THREAD_PAGE_CAPACITY },
        >,
    ) -> Result<(), crate::kernel::inspect::Error> {
        let inspector = self
            .process
            .resolve_handle::<TaskInspector>(inspector, Rights::INSPECT)
            .map_err(crate::kernel::inspect::Error::Process)?;
        inspector.object().scan_threads(cursor, output)
    }

    fn scan_objects(
        &self,
        inspector: HandleValue,
        cursor: u64,
        output: &mut crate::kernel::inspect::Page<
            crate::kernel::object::ObjectSnapshot,
            { crate::kernel::inspect::OBJECT_PAGE_CAPACITY },
        >,
    ) -> Result<(), crate::kernel::inspect::Error> {
        let inspector = self
            .process
            .resolve_handle::<ObjectInspector>(inspector, Rights::INSPECT)
            .map_err(crate::kernel::inspect::Error::Process)?;
        inspector.object().scan_objects(cursor, output)
    }

    fn scan_process_handles(
        &self,
        inspector: HandleValue,
        process_koid: u64,
        cursor: u64,
    ) -> Result<
        crate::kernel::inspect::Page<
            crate::kernel::inspect::ProcessHandleSnapshot,
            { crate::kernel::inspect::HANDLE_PAGE_CAPACITY },
        >,
        crate::kernel::inspect::Error,
    > {
        let inspector = self
            .process
            .resolve_handle::<ObjectInspector>(inspector, Rights::INSPECT)
            .map_err(crate::kernel::inspect::Error::Process)?;
        inspector
            .object()
            .scan_process_handles(process_koid, cursor)
    }

    fn derive_task_inspector(
        &self,
        inspector: HandleValue,
        process: HandleValue,
    ) -> Result<HandleValue, crate::kernel::inspect::Error> {
        let inspector = self
            .process
            .resolve_handle::<TaskInspector>(
                inspector,
                <TaskInspector as KernelObject>::SUPPORTED_RIGHTS,
            )
            .map_err(crate::kernel::inspect::Error::Process)?;
        let target = self
            .process
            .resolve_handle::<ProcessObject>(process, Rights::INSPECT)
            .map_err(crate::kernel::inspect::Error::Process)?;
        let derived = inspector
            .object()
            .try_derive_process(target.object(), &self.process.resource_domain())?;
        self.process
            .create_object(derived, <TaskInspector as KernelObject>::SUPPORTED_RIGHTS)
            .map_err(crate::kernel::inspect::Error::Process)
    }

    fn derive_object_inspector(
        &self,
        inspector: HandleValue,
        process: HandleValue,
    ) -> Result<HandleValue, crate::kernel::inspect::Error> {
        let inspector = self
            .process
            .resolve_handle::<ObjectInspector>(
                inspector,
                <ObjectInspector as KernelObject>::SUPPORTED_RIGHTS,
            )
            .map_err(crate::kernel::inspect::Error::Process)?;
        let target = self
            .process
            .resolve_handle::<ProcessObject>(process, Rights::INSPECT)
            .map_err(crate::kernel::inspect::Error::Process)?;
        let derived = inspector
            .object()
            .try_derive_process(target.object(), &self.process.resource_domain())?;
        self.process
            .create_object(derived, <ObjectInspector as KernelObject>::SUPPORTED_RIGHTS)
            .map_err(crate::kernel::inspect::Error::Process)
    }

    fn derive_task_inspector_for_task_group(
        &self,
        inspector: HandleValue,
        group: HandleValue,
    ) -> Result<HandleValue, crate::kernel::inspect::Error> {
        let inspector = self
            .process
            .resolve_handle::<TaskInspector>(
                inspector,
                <TaskInspector as KernelObject>::SUPPORTED_RIGHTS,
            )
            .map_err(crate::kernel::inspect::Error::Process)?;
        let target = self
            .process
            .resolve_handle::<TaskGroupObject>(group, Rights::INSPECT)
            .map_err(crate::kernel::inspect::Error::Process)?;
        let derived = inspector
            .object()
            .try_derive_task_group(target.object(), &self.process.resource_domain())?;
        self.process
            .create_object(derived, <TaskInspector as KernelObject>::SUPPORTED_RIGHTS)
            .map_err(crate::kernel::inspect::Error::Process)
    }

    fn derive_object_inspector_for_task_group(
        &self,
        inspector: HandleValue,
        group: HandleValue,
    ) -> Result<HandleValue, crate::kernel::inspect::Error> {
        let inspector = self
            .process
            .resolve_handle::<ObjectInspector>(
                inspector,
                <ObjectInspector as KernelObject>::SUPPORTED_RIGHTS,
            )
            .map_err(crate::kernel::inspect::Error::Process)?;
        let target = self
            .process
            .resolve_handle::<TaskGroupObject>(group, Rights::INSPECT)
            .map_err(crate::kernel::inspect::Error::Process)?;
        let derived = inspector
            .object()
            .try_derive_task_group(target.object(), &self.process.resource_domain())?;
        self.process
            .create_object(derived, <ObjectInspector as KernelObject>::SUPPORTED_RIGHTS)
            .map_err(crate::kernel::inspect::Error::Process)
    }

    fn derive_task_inspector_for_resource_domain(
        &self,
        inspector: HandleValue,
        domain: HandleValue,
    ) -> Result<HandleValue, crate::kernel::inspect::Error> {
        let inspector = self
            .process
            .resolve_handle::<TaskInspector>(
                inspector,
                <TaskInspector as KernelObject>::SUPPORTED_RIGHTS,
            )
            .map_err(crate::kernel::inspect::Error::Process)?;
        let target = self
            .process
            .resolve_handle::<ResourceDomainObject>(domain, Rights::INSPECT)
            .map_err(crate::kernel::inspect::Error::Process)?;
        let derived = inspector
            .object()
            .try_derive_resource_domain(target.object(), &self.process.resource_domain())?;
        self.process
            .create_object(derived, <TaskInspector as KernelObject>::SUPPORTED_RIGHTS)
            .map_err(crate::kernel::inspect::Error::Process)
    }

    fn derive_object_inspector_for_resource_domain(
        &self,
        inspector: HandleValue,
        domain: HandleValue,
    ) -> Result<HandleValue, crate::kernel::inspect::Error> {
        let inspector = self
            .process
            .resolve_handle::<ObjectInspector>(
                inspector,
                <ObjectInspector as KernelObject>::SUPPORTED_RIGHTS,
            )
            .map_err(crate::kernel::inspect::Error::Process)?;
        let target = self
            .process
            .resolve_handle::<ResourceDomainObject>(domain, Rights::INSPECT)
            .map_err(crate::kernel::inspect::Error::Process)?;
        let derived = inspector
            .object()
            .try_derive_resource_domain(target.object(), &self.process.resource_domain())?;
        self.process
            .create_object(derived, <ObjectInspector as KernelObject>::SUPPORTED_RIGHTS)
            .map_err(crate::kernel::inspect::Error::Process)
    }
}

impl IpcServices for DeferredProcessServices<'_> {
    fn create_byte_channel(&self) -> Result<[HandleValue; 2], ByteChannelServiceError> {
        crate::kernel::ipc::byte_channel_create(self.process)
    }

    fn create_capability_channel(&self) -> Result<[HandleValue; 2], CapabilityChannelServiceError> {
        crate::kernel::ipc::capability_channel_create(self.process)
    }

    fn write_byte_channel(
        &self,
        endpoint: HandleValue,
        bytes: Option<UserSlice>,
    ) -> Result<(), ByteChannelServiceError> {
        crate::kernel::ipc::byte_channel_write(self.process, endpoint, bytes)
    }

    fn read_byte_channel(
        &self,
        endpoint: HandleValue,
        bytes: Option<UserSlice>,
    ) -> Result<ByteChannelReadOutcome, ByteChannelServiceError> {
        crate::kernel::ipc::byte_channel_read(self.process, endpoint, bytes)
    }

    fn try_send_capability_channel(
        &self,
        endpoint: HandleValue,
        bytes: Option<UserSlice>,
        dispositions: Option<UserSlice>,
    ) -> Result<(), CapabilityChannelServiceError> {
        crate::kernel::ipc::capability_channel_try_send(self.process, endpoint, bytes, dispositions)
    }

    fn receive_capability_channel(
        &self,
        endpoint: HandleValue,
        deadline: u64,
        bytes: Option<UserSlice>,
        slots: Option<UserSlice>,
    ) -> Result<CapabilityReceiveOutcome, CapabilityChannelServiceError> {
        crate::kernel::ipc::capability_channel_receive(
            self.process,
            endpoint,
            deadline,
            bytes,
            slots,
            || self.thread.snapshot().phase == UserThreadPhase::StopRequested,
        )
    }
}

impl ConsoleServices for DeferredProcessServices<'_> {
    fn read_console(
        &self,
        value: HandleValue,
        destination: Option<UserSlice>,
    ) -> Result<usize, ConsoleServiceError> {
        let console = self
            .process
            .resolve_handle::<crate::kernel::device::console::SystemConsole>(value, Rights::READ)?;
        let Some(destination) = destination else {
            return Ok(0);
        };
        let capacity =
            usize::try_from(destination.length()).map_err(|_| ProcessError::Allocation)?;
        let claim = console.object().claim_read(capacity)?;
        let actual = claim.bytes().len();
        let actual_bytes = u64::try_from(actual).map_err(|_| ProcessError::Allocation)?;
        let destination = UserSlice::new(destination.base(), actual_bytes)
            .map_err(|error| ProcessError::UserMemory(error.into()))?;
        let write = self.process.reserve_user_write(destination)?;
        write
            .copy_from(claim.bytes())
            .map_err(ProcessError::UserMemory)?;
        write.complete();
        claim.commit();
        Ok(actual)
    }

    fn write_console(
        &self,
        value: HandleValue,
        source: Option<UserSlice>,
    ) -> Result<usize, ConsoleServiceError> {
        let console = self
            .process
            .resolve_handle::<crate::kernel::device::console::SystemConsole>(
                value,
                Rights::WRITE,
            )?;
        let Some(source) = source else {
            return Ok(0);
        };
        let length = usize::try_from(source.length())
            .map_err(|_| ProcessError::Allocation)?
            .min(crate::kernel::device::console::TRANSFER_BATCH_BYTES);
        let length_bytes = u64::try_from(length).map_err(|_| ProcessError::Allocation)?;
        let source = UserSlice::new(source.base(), length_bytes)
            .map_err(|error| ProcessError::UserMemory(error.into()))?;
        let mut bytes = [0; crate::kernel::device::console::TRANSFER_BATCH_BYTES];
        self.process.copy_from_user(source, &mut bytes[..length])?;
        Ok(console.object().try_write(&bytes[..length])?)
    }
}

impl MemoryServices for DeferredProcessServices<'_> {
    fn create_contiguous_vmo(
        &self,
        size: u64,
    ) -> Result<HandleValue, crate::kernel::mm::user_space::MemoryServiceError> {
        crate::kernel::mm::user_space::create_contiguous_vmo(self.process, size)
    }

    fn create_vmo(
        &self,
        size: u64,
    ) -> Result<HandleValue, crate::kernel::mm::user_space::MemoryServiceError> {
        crate::kernel::mm::user_space::create_vmo(self.process, size)
    }

    fn create_file_executable_vmo(
        &self,
        value: HandleValue,
    ) -> Result<(HandleValue, u64), crate::kernel::mm::user_space::MemoryServiceError> {
        crate::kernel::mm::user_space::create_file_executable_vmo(self.process, value)
    }
    fn create_file_snapshot(
        &self,
        value: HandleValue,
    ) -> Result<(HandleValue, u64), crate::kernel::mm::user_space::MemoryServiceError> {
        crate::kernel::mm::user_space::create_file_snapshot(self.process, value)
    }
    fn create_vmo_snapshot(
        &self,
        value: HandleValue,
    ) -> Result<(HandleValue, u64), crate::kernel::mm::user_space::MemoryServiceError> {
        crate::kernel::mm::user_space::create_vmo_snapshot(self.process, value)
    }
    fn map_private(
        &self,
        vmar: HandleValue,
        snapshot: HandleValue,
        request: crate::kernel::mm::user_space::PrivateMappingRequest,
    ) -> Result<(), crate::kernel::mm::user_space::MemoryServiceError> {
        crate::kernel::mm::user_space::map_private(self.process, vmar, snapshot, request)
    }
    fn read_vmo(
        &self,
        vmo: HandleValue,
        offset: u64,
        output: Option<UserSlice>,
    ) -> Result<(), crate::kernel::mm::user_space::MemoryServiceError> {
        crate::kernel::mm::user_space::read_vmo(self.process, vmo, offset, output)
    }

    fn write_vmo(
        &self,
        vmo: HandleValue,
        offset: u64,
        input: Option<UserSlice>,
    ) -> Result<(), crate::kernel::mm::user_space::MemoryServiceError> {
        crate::kernel::mm::user_space::write_vmo(self.process, vmo, offset, input)
    }

    fn allocate_vmar(
        &self,
        parent: HandleValue,
        address: u64,
        size: u64,
    ) -> Result<HandleValue, crate::kernel::mm::user_space::MemoryServiceError> {
        crate::kernel::mm::user_space::allocate_vmar(self.process, parent, address, size)
    }

    fn map_vmo(
        &self,
        vmar: HandleValue,
        vmo: HandleValue,
        vmo_offset: u64,
        address: u64,
        size: u64,
        permissions: crate::kernel::mm::user_space::Permissions,
    ) -> Result<(), crate::kernel::mm::user_space::MemoryServiceError> {
        crate::kernel::mm::user_space::map_vmo(
            self.process,
            vmar,
            vmo,
            vmo_offset,
            address,
            size,
            permissions,
        )
    }

    fn protect_vmar(
        &self,
        vmar: HandleValue,
        address: u64,
        size: u64,
        permissions: crate::kernel::mm::user_space::Permissions,
    ) -> Result<(), crate::kernel::mm::user_space::MemoryServiceError> {
        crate::kernel::mm::user_space::protect(self.process, vmar, address, size, permissions)
    }

    fn unmap_vmar(
        &self,
        vmar: HandleValue,
        address: u64,
        size: u64,
    ) -> Result<(), crate::kernel::mm::user_space::MemoryServiceError> {
        crate::kernel::mm::user_space::unmap(self.process, vmar, address, size)
    }

    fn destroy_vmar(
        &self,
        vmar: HandleValue,
    ) -> Result<(), crate::kernel::mm::user_space::MemoryServiceError> {
        crate::kernel::mm::user_space::destroy_vmar(self.process, vmar)
    }
}

impl ProcessBuilderServices for DeferredProcessServices<'_> {
    fn create_process_builder(
        &self,
        factory: HandleValue,
        group: HandleValue,
        domain: HandleValue,
        executable: HandleValue,
    ) -> Result<HandleValue, ProcessBuilderServiceError> {
        crate::kernel::process::create_process_builder(
            self.process,
            factory,
            group,
            domain,
            executable,
        )
        .map_err(ProcessBuilderServiceError::Builder)
    }

    fn set_process_builder_name(
        &self,
        builder: HandleValue,
        name: Option<UserSlice>,
    ) -> Result<(), ProcessBuilderServiceError> {
        let builder = self
            .process
            .resolve_handle::<ProcessBuilder>(builder, Rights::WRITE)?;
        let bytes = self.copy_builder_input(name)?;
        let name = core::str::from_utf8(bytes.as_bytes())
            .map_err(|_| ProcessBuilderServiceError::InvalidInput)?;
        builder.object().set_name(name)?;
        Ok(())
    }

    fn add_process_builder_argument(
        &self,
        builder: HandleValue,
        argument: Option<UserSlice>,
    ) -> Result<(), ProcessBuilderServiceError> {
        let builder = self
            .process
            .resolve_handle::<ProcessBuilder>(builder, Rights::WRITE)?;
        let bytes = self.copy_builder_input(argument)?;
        let argument = core::str::from_utf8(bytes.as_bytes())
            .map_err(|_| ProcessBuilderServiceError::InvalidInput)?;
        builder.object().add_argument(argument)?;
        Ok(())
    }

    fn add_process_builder_environment(
        &self,
        builder: HandleValue,
        environment: Option<UserSlice>,
    ) -> Result<(), ProcessBuilderServiceError> {
        let builder = self
            .process
            .resolve_handle::<ProcessBuilder>(builder, Rights::WRITE)?;
        let bytes = self.copy_builder_input(environment)?;
        let environment = core::str::from_utf8(bytes.as_bytes())
            .map_err(|_| ProcessBuilderServiceError::InvalidInput)?;
        builder.object().add_environment(environment)?;
        Ok(())
    }

    fn set_process_builder_affinity(
        &self,
        builder: HandleValue,
        words: Option<UserSlice>,
        word_count: usize,
    ) -> Result<(), ProcessBuilderServiceError> {
        let builder = self
            .process
            .resolve_handle::<ProcessBuilder>(builder, Rights::WRITE)?;
        let affinity = self.copy_affinity(words, word_count)?;
        builder.object().set_affinity(affinity)?;
        Ok(())
    }

    fn add_process_builder_handle(
        &self,
        builder: HandleValue,
        source: HandleValue,
        purpose: u32,
        expected_kind: ObjectKind,
        requested_rights: Option<Rights>,
        operation: crate::kernel::capability::HandleTransferOperation,
    ) -> Result<(), ProcessBuilderServiceError> {
        let builder = self
            .process
            .resolve_handle::<ProcessBuilder>(builder, Rights::WRITE)?;
        builder.object().add_startup_capability(
            self.process,
            StartupCapability::new(
                purpose,
                source,
                requested_rights,
                Some(expected_kind),
                operation,
            ),
        )?;
        Ok(())
    }

    fn seal_process_builder(&self, builder: HandleValue) -> Result<(), ProcessBuilderServiceError> {
        let builder = self
            .process
            .resolve_handle::<ProcessBuilder>(builder, Rights::WRITE)?;
        builder.object().seal()?;
        Ok(())
    }

    fn start_process_builder(
        &self,
        builder: HandleValue,
    ) -> Result<HandleValue, ProcessBuilderServiceError> {
        let started = crate::kernel::process::start_process_builder(self.process, builder)
            .map_err(ProcessBuilderServiceError::Start)?;
        Ok(started.supervisor_handle())
    }

    fn abort_process_builder(
        &self,
        builder: HandleValue,
    ) -> Result<(), ProcessBuilderServiceError> {
        crate::kernel::process::abort_process_builder(self.process, builder)?;
        Ok(())
    }
}

fn decode_wait_item(record: &[u8]) -> Result<(u64, u64), ObjectServiceError> {
    type AbiWaitItem = hyper::abi::native::HyperNativeObjectWaitItem;
    let handle = read_u64_field(record, core::mem::offset_of!(AbiWaitItem, handle))
        .ok_or(ObjectServiceError::InvalidInput)?;
    let signals = read_u64_field(record, core::mem::offset_of!(AbiWaitItem, signals))
        .ok_or(ObjectServiceError::InvalidInput)?;
    Ok((handle, signals))
}

fn read_u64_field(record: &[u8], offset: usize) -> Option<u64> {
    let end = offset.checked_add(core::mem::size_of::<u64>())?;
    let bytes: &[u8; core::mem::size_of::<u64>()] = record.get(offset..end)?.try_into().ok()?;
    Some(u64::from_ne_bytes(*bytes))
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

fn atomic_wait_error(error: crate::kernel::process::atomic_wait::Error) -> ObjectServiceError {
    use crate::kernel::process::atomic_wait::Error;
    match error {
        Error::Process(error) => ObjectServiceError::Process(error),
        Error::Wait(error) => ObjectServiceError::Wait(error),
        Error::InvalidInput => ObjectServiceError::InvalidInput,
    }
}
