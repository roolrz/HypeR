// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native-user execution session and synchronous-call policy entry.

use alloc::vec::Vec;
use core::ptr::NonNull;

use hyper::abi::native::{NativeInvocation, NativeResult};
use hyper::hal::user::{
    NativeCallAction, NativeCallHandler, NativeCallService, UserFault, UserFaultKind,
};
use hyper::sync::InterruptMaskGuard;

use crate::kernel::abi::native::{
    self, ConsoleServiceError, ConsoleServices, HandleServices, HierarchyServices, InspectServices,
    IpcServices, MemoryServices, ObjectServiceError, ObjectServices, ProcessBuilderServiceError,
    ProcessBuilderServices, SystemInspectServices, TaskServices, UserMemoryServices, VfsServices,
    VmServices,
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
    AbiFamily, ExecutionRoute, Process, ProcessBuilder, ProcessError, ProcessObject,
    ProcessSnapshot, RunAdmissionError, StartupCapability, StoppedUserRun, TaskGroupObject,
    TerminalReason, UserExecution, UserThread, UserThreadPhase,
};
use crate::kernel::task::scheduler::CpuMask;
use crate::kernel::vfs::VfsServiceError;

#[derive(Clone, Copy, Eq, PartialEq)]
enum NativeRunAction {
    Resume,
    Stop,
}

struct ProcessServices<'process> {
    process: &'process Process,
}

impl UserMemoryServices for ProcessServices<'_> {
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

impl HandleServices for ProcessServices<'_> {
    fn close_handle(&self, value: HandleValue) -> Result<(), ProcessError> {
        self.process.close_handle(value)
    }

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

struct DeferredProcessServices<'session> {
    session: &'session UserSession,
}

impl UserMemoryServices for DeferredProcessServices<'_> {
    fn copy_to_user(&self, destination: UserSlice, source: &[u8]) -> Result<(), ProcessError> {
        self.session.process.copy_to_user(destination, source)
    }

    fn copy_from_user(
        &self,
        source: UserSlice,
        destination: &mut [u8],
    ) -> Result<(), ProcessError> {
        self.session.process.copy_from_user(source, destination)
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
    fn copy_builder_input(
        &self,
        input: Option<UserSlice>,
    ) -> Result<ChargedBuilderInput, ProcessBuilderServiceError> {
        let length = input.map_or(0, UserSlice::length);
        let length =
            usize::try_from(length).map_err(|_| ProcessBuilderServiceError::InvalidInput)?;
        let charge = self
            .session
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
            self.session.process.copy_from_user(input, &mut bytes)?;
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
            self.session
                .process
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

impl HandleServices for DeferredProcessServices<'_> {
    fn close_handle(&self, value: HandleValue) -> Result<(), ProcessError> {
        self.session.process.close_handle(value)
    }

    fn handle_info(
        &self,
        value: HandleValue,
        required_rights: Rights,
    ) -> Result<HandleInfo, ProcessError> {
        self.session.process.handle_info(value, required_rights)
    }

    fn duplicate_handle(
        &self,
        value: HandleValue,
        rights: Rights,
    ) -> Result<HandleValue, ProcessError> {
        self.session.process.duplicate_handle(value, rights)
    }
    fn replace_handle(
        &self,
        value: HandleValue,
        rights: Rights,
    ) -> Result<HandleValue, ProcessError> {
        self.session.process.replace_handle(value, rights)
    }
}

impl HierarchyServices for DeferredProcessServices<'_> {
    fn create_resource_domain(
        &self,
        parent: HandleValue,
        limits: crate::kernel::accounting::ResourceLimits,
    ) -> Result<HandleValue, crate::kernel::process::hierarchy::Error> {
        crate::kernel::process::hierarchy::create_resource_domain(
            &self.session.process,
            parent,
            limits,
        )
    }

    fn create_task_group(
        &self,
        factory: HandleValue,
        domain: HandleValue,
    ) -> Result<HandleValue, crate::kernel::process::hierarchy::Error> {
        crate::kernel::process::hierarchy::create_task_group(&self.session.process, factory, domain)
    }
}

impl VmServices for DeferredProcessServices<'_> {
    fn derive_virtual_machine_creation_lease(
        &self,
        authority: HandleValue,
        domain: HandleValue,
    ) -> Result<HandleValue, crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::derive_creation_lease(&self.session.process, authority, domain)
    }

    fn create_pending_virtual_machine(
        &self,
        lease: HandleValue,
        configuration: crate::kernel::vm::objects::VirtualMachineConfiguration,
    ) -> Result<HandleValue, crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::create_pending(&self.session.process, lease, configuration)
    }

    fn set_pending_virtual_machine_memory(
        &self,
        pending: HandleValue,
        vmo: HandleValue,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::set_memory(&self.session.process, pending, vmo)
    }

    fn set_pending_virtual_machine_bootstrap(
        &self,
        pending: HandleValue,
        bootstrap: crate::kernel::vm::objects::VirtualCpuBootstrap,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::set_bootstrap(&self.session.process, pending, bootstrap)
    }

    fn set_pending_virtual_machine_virtual_serial(
        &self,
        pending: HandleValue,
        serial: HandleValue,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::set_virtual_serial(&self.session.process, pending, serial)
    }

    fn create_virtual_serial(&self) -> Result<HandleValue, crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::create_virtual_serial(&self.session.process)
    }

    fn register_virtual_serial_output(
        &self,
        serial: HandleValue,
        buffer: HandleValue,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::register_virtual_serial_output(
            &self.session.process,
            serial,
            buffer,
        )
    }
    fn acknowledge_virtual_serial_output(
        &self,
        serial: HandleValue,
        consumed: u64,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::acknowledge_virtual_serial_output(
            &self.session.process,
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
            .session
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
        let mut bytes = [0; crate::kernel::vm::virtual_serial::TRANSFER_BATCH_BYTES];
        self.session
            .process
            .copy_from_user(source, &mut bytes[..length])?;
        serial
            .object()
            .write_input(&bytes[..length])
            .map_err(crate::kernel::vm::objects::Error::from)
            .map_err(Into::into)
    }

    fn seal_pending_virtual_machine(
        &self,
        pending: HandleValue,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::seal(&self.session.process, pending)
    }

    fn install_pending_virtual_machine(
        &self,
        pending: HandleValue,
    ) -> Result<[HandleValue; 2], crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::install(&self.session.process, pending)
    }

    fn abort_pending_virtual_machine(
        &self,
        pending: HandleValue,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::abort(&self.session.process, pending)
    }

    fn request_virtual_machine_stop(
        &self,
        machine: HandleValue,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::request_stop(&self.session.process, machine)
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
        crate::kernel::vm::service::machine_info(&self.session.process, machine)
    }

    fn virtual_cpu_info(
        &self,
        vcpu: HandleValue,
    ) -> Result<crate::kernel::vm::objects::VirtualCpuSnapshot, crate::kernel::vm::service::Error>
    {
        crate::kernel::vm::service::vcpu_info(&self.session.process, vcpu)
    }

    fn start_virtual_cpu(
        &self,
        vcpu: HandleValue,
    ) -> Result<(), crate::kernel::vm::service::Error> {
        crate::kernel::vm::service::start_vcpu(&self.session.process, vcpu)
    }
}

impl SystemInspectServices for DeferredProcessServices<'_> {
    fn memory_observation(
        &self,
        inspector: HandleValue,
    ) -> Result<crate::kernel::inspect::MemoryObservation, crate::kernel::inspect::Error> {
        let inspector = self
            .session
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
            .session
            .process
            .resolve_handle::<CpuInspector>(inspector, Rights::INSPECT)
            .map_err(crate::kernel::inspect::Error::Process)?;
        Ok(inspector.object().snapshot())
    }
}

impl ObjectServices for DeferredProcessServices<'_> {
    fn current_process_id(&self) -> u64 {
        self.session.process.koid().get()
    }
    fn wait_set_create(&self, capacity: usize) -> Result<HandleValue, ObjectServiceError> {
        let set = object::WaitSet::try_new(capacity, &self.session.process.resource_domain())?;
        Ok(self.session.process.create_object(
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
            .session
            .process
            .resolve_handle::<object::WaitSet>(set, Rights::BIND_WAIT)?;
        let source = self
            .session
            .process
            .resolve_waitable(source, Rights::WAIT)?;
        Ok(set
            .object()
            .add(source, signals, &self.session.process.resource_domain())?)
    }
    fn wait_set_rearm(
        &self,
        set: HandleValue,
        registration: u64,
    ) -> Result<(), ObjectServiceError> {
        let set = self
            .session
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
            .session
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
            .session
            .process
            .resolve_handle::<object::WaitSet>(set, Rights::WAIT)?;
        let delivery =
            set.object()
                .wait(deadline, &self.session.process.resource_domain(), || {
                    self.session.thread.snapshot().phase == UserThreadPhase::StopRequested
                })?;
        self.session
            .process
            .copy_to_user(output, &delivery.record())?;
        delivery.complete();
        Ok(())
    }

    fn create_event(&self) -> Result<HandleValue, ObjectServiceError> {
        let event = Event::try_new(&self.session.process.resource_domain())?;
        Ok(self
            .session
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
            .session
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
        let resolved = self.session.process.resolve_waitable(value, Rights::WAIT)?;
        let domain = self.session.process.resource_domain();
        Ok(object::wait_one(
            resolved.source(),
            &domain,
            requested,
            deadline,
            || self.session.thread.snapshot().phase == UserThreadPhase::StopRequested,
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
            .session
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
        self.session.process.copy_from_user(items, &mut encoded)?;

        let mut resolved = Vec::new();
        resolved
            .try_reserve_exact(item_count)
            .map_err(|_| ProcessError::Allocation)?;
        for record in encoded.chunks_exact(record_size) {
            let (raw_handle, signals) = decode_wait_item(record)?;
            let handle = HandleValue::try_from_raw(raw_handle).map_err(ProcessError::from)?;
            resolved.push(ResolvedWaitEntry {
                object: self
                    .session
                    .process
                    .resolve_waitable(handle, Rights::WAIT)?,
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
        let domain = self.session.process.resource_domain();
        Ok(object::wait_many(&requests, &domain, deadline, || {
            self.session.thread.snapshot().phase == UserThreadPhase::StopRequested
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
        let process = &self.session.process;
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
            .session
            .process
            .resolve_user_thread_handle(value, Rights::START)?;
        thread.ready()?;
        Ok(())
    }
    fn stop_thread(&self, value: HandleValue) -> Result<(), ProcessError> {
        let thread = self
            .session
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
        crate::kernel::process::atomic_wait::wait(
            &self.session.process,
            address,
            expected,
            deadline,
            || self.session.thread.snapshot().phase == UserThreadPhase::StopRequested,
        )
        .map_err(atomic_wait_error)
    }
    fn atomic_wake(&self, address: u64, count: u32) -> Result<u64, ObjectServiceError> {
        crate::kernel::process::atomic_wait::wake(&self.session.process, address, count)
            .map_err(atomic_wait_error)
    }
    fn sleep_thread(
        &self,
        deadline: u64,
    ) -> Result<crate::kernel::task::WaitOutcome, ObjectServiceError> {
        crate::kernel::process::atomic_wait::sleep(&self.session.process, deadline, || {
            self.session.thread.snapshot().phase == UserThreadPhase::StopRequested
        })
        .map_err(atomic_wait_error)
    }

    fn process_info(&self, process: HandleValue) -> Result<ProcessSnapshot, ProcessError> {
        Ok(self
            .session
            .process
            .resolve_handle::<ProcessObject>(process, Rights::INSPECT)?
            .object()
            .snapshot())
    }

    fn request_process_stop(&self, process: HandleValue) -> Result<(), ProcessError> {
        let process = self
            .session
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
    ) -> Result<
        crate::kernel::inspect::Page<
            ProcessSnapshot,
            { crate::kernel::inspect::PROCESS_PAGE_CAPACITY },
        >,
        crate::kernel::inspect::Error,
    > {
        let inspector = self
            .session
            .process
            .resolve_handle::<TaskInspector>(inspector, Rights::INSPECT)
            .map_err(crate::kernel::inspect::Error::Process)?;
        inspector.object().scan_processes(cursor)
    }

    fn scan_threads(
        &self,
        inspector: HandleValue,
        cursor: u64,
    ) -> Result<
        crate::kernel::inspect::Page<
            crate::kernel::inspect::TaskThreadSnapshot,
            { crate::kernel::inspect::THREAD_PAGE_CAPACITY },
        >,
        crate::kernel::inspect::Error,
    > {
        let inspector = self
            .session
            .process
            .resolve_handle::<TaskInspector>(inspector, Rights::INSPECT)
            .map_err(crate::kernel::inspect::Error::Process)?;
        inspector.object().scan_threads(cursor)
    }

    fn scan_objects(
        &self,
        inspector: HandleValue,
        cursor: u64,
    ) -> Result<
        crate::kernel::inspect::Page<
            crate::kernel::object::ObjectSnapshot,
            { crate::kernel::inspect::OBJECT_PAGE_CAPACITY },
        >,
        crate::kernel::inspect::Error,
    > {
        let inspector = self
            .session
            .process
            .resolve_handle::<ObjectInspector>(inspector, Rights::INSPECT)
            .map_err(crate::kernel::inspect::Error::Process)?;
        inspector.object().scan_objects(cursor)
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
            .session
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
            .session
            .process
            .resolve_handle::<TaskInspector>(
                inspector,
                <TaskInspector as KernelObject>::SUPPORTED_RIGHTS,
            )
            .map_err(crate::kernel::inspect::Error::Process)?;
        let target = self
            .session
            .process
            .resolve_handle::<ProcessObject>(process, Rights::INSPECT)
            .map_err(crate::kernel::inspect::Error::Process)?;
        let derived = inspector
            .object()
            .try_derive_process(target.object(), &self.session.process.resource_domain())?;
        self.session
            .process
            .create_object(derived, <TaskInspector as KernelObject>::SUPPORTED_RIGHTS)
            .map_err(crate::kernel::inspect::Error::Process)
    }

    fn derive_object_inspector(
        &self,
        inspector: HandleValue,
        process: HandleValue,
    ) -> Result<HandleValue, crate::kernel::inspect::Error> {
        let inspector = self
            .session
            .process
            .resolve_handle::<ObjectInspector>(
                inspector,
                <ObjectInspector as KernelObject>::SUPPORTED_RIGHTS,
            )
            .map_err(crate::kernel::inspect::Error::Process)?;
        let target = self
            .session
            .process
            .resolve_handle::<ProcessObject>(process, Rights::INSPECT)
            .map_err(crate::kernel::inspect::Error::Process)?;
        let derived = inspector
            .object()
            .try_derive_process(target.object(), &self.session.process.resource_domain())?;
        self.session
            .process
            .create_object(derived, <ObjectInspector as KernelObject>::SUPPORTED_RIGHTS)
            .map_err(crate::kernel::inspect::Error::Process)
    }

    fn derive_task_inspector_for_task_group(
        &self,
        inspector: HandleValue,
        group: HandleValue,
    ) -> Result<HandleValue, crate::kernel::inspect::Error> {
        let inspector = self
            .session
            .process
            .resolve_handle::<TaskInspector>(
                inspector,
                <TaskInspector as KernelObject>::SUPPORTED_RIGHTS,
            )
            .map_err(crate::kernel::inspect::Error::Process)?;
        let target = self
            .session
            .process
            .resolve_handle::<TaskGroupObject>(group, Rights::INSPECT)
            .map_err(crate::kernel::inspect::Error::Process)?;
        let derived = inspector
            .object()
            .try_derive_task_group(target.object(), &self.session.process.resource_domain())?;
        self.session
            .process
            .create_object(derived, <TaskInspector as KernelObject>::SUPPORTED_RIGHTS)
            .map_err(crate::kernel::inspect::Error::Process)
    }

    fn derive_object_inspector_for_task_group(
        &self,
        inspector: HandleValue,
        group: HandleValue,
    ) -> Result<HandleValue, crate::kernel::inspect::Error> {
        let inspector = self
            .session
            .process
            .resolve_handle::<ObjectInspector>(
                inspector,
                <ObjectInspector as KernelObject>::SUPPORTED_RIGHTS,
            )
            .map_err(crate::kernel::inspect::Error::Process)?;
        let target = self
            .session
            .process
            .resolve_handle::<TaskGroupObject>(group, Rights::INSPECT)
            .map_err(crate::kernel::inspect::Error::Process)?;
        let derived = inspector
            .object()
            .try_derive_task_group(target.object(), &self.session.process.resource_domain())?;
        self.session
            .process
            .create_object(derived, <ObjectInspector as KernelObject>::SUPPORTED_RIGHTS)
            .map_err(crate::kernel::inspect::Error::Process)
    }

    fn derive_task_inspector_for_resource_domain(
        &self,
        inspector: HandleValue,
        domain: HandleValue,
    ) -> Result<HandleValue, crate::kernel::inspect::Error> {
        let inspector = self
            .session
            .process
            .resolve_handle::<TaskInspector>(
                inspector,
                <TaskInspector as KernelObject>::SUPPORTED_RIGHTS,
            )
            .map_err(crate::kernel::inspect::Error::Process)?;
        let target = self
            .session
            .process
            .resolve_handle::<ResourceDomainObject>(domain, Rights::INSPECT)
            .map_err(crate::kernel::inspect::Error::Process)?;
        let derived = inspector
            .object()
            .try_derive_resource_domain(target.object(), &self.session.process.resource_domain())?;
        self.session
            .process
            .create_object(derived, <TaskInspector as KernelObject>::SUPPORTED_RIGHTS)
            .map_err(crate::kernel::inspect::Error::Process)
    }

    fn derive_object_inspector_for_resource_domain(
        &self,
        inspector: HandleValue,
        domain: HandleValue,
    ) -> Result<HandleValue, crate::kernel::inspect::Error> {
        let inspector = self
            .session
            .process
            .resolve_handle::<ObjectInspector>(
                inspector,
                <ObjectInspector as KernelObject>::SUPPORTED_RIGHTS,
            )
            .map_err(crate::kernel::inspect::Error::Process)?;
        let target = self
            .session
            .process
            .resolve_handle::<ResourceDomainObject>(domain, Rights::INSPECT)
            .map_err(crate::kernel::inspect::Error::Process)?;
        let derived = inspector
            .object()
            .try_derive_resource_domain(target.object(), &self.session.process.resource_domain())?;
        self.session
            .process
            .create_object(derived, <ObjectInspector as KernelObject>::SUPPORTED_RIGHTS)
            .map_err(crate::kernel::inspect::Error::Process)
    }
}

impl IpcServices for DeferredProcessServices<'_> {
    fn create_byte_channel(&self) -> Result<[HandleValue; 2], ByteChannelServiceError> {
        crate::kernel::ipc::byte_channel_create(&self.session.process)
    }

    fn create_capability_channel(&self) -> Result<[HandleValue; 2], CapabilityChannelServiceError> {
        crate::kernel::ipc::capability_channel_create(&self.session.process)
    }

    fn write_byte_channel(
        &self,
        endpoint: HandleValue,
        bytes: Option<UserSlice>,
    ) -> Result<(), ByteChannelServiceError> {
        crate::kernel::ipc::byte_channel_write(&self.session.process, endpoint, bytes)
    }

    fn read_byte_channel(
        &self,
        endpoint: HandleValue,
        bytes: Option<UserSlice>,
    ) -> Result<ByteChannelReadOutcome, ByteChannelServiceError> {
        crate::kernel::ipc::byte_channel_read(&self.session.process, endpoint, bytes)
    }

    fn try_send_capability_channel(
        &self,
        endpoint: HandleValue,
        bytes: Option<UserSlice>,
        dispositions: Option<UserSlice>,
    ) -> Result<(), CapabilityChannelServiceError> {
        crate::kernel::ipc::capability_channel_try_send(
            &self.session.process,
            endpoint,
            bytes,
            dispositions,
        )
    }

    fn receive_capability_channel(
        &self,
        endpoint: HandleValue,
        deadline: u64,
        bytes: Option<UserSlice>,
        slots: Option<UserSlice>,
    ) -> Result<CapabilityReceiveOutcome, CapabilityChannelServiceError> {
        crate::kernel::ipc::capability_channel_receive(
            &self.session.process,
            endpoint,
            deadline,
            bytes,
            slots,
            || self.session.thread.snapshot().phase == UserThreadPhase::StopRequested,
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
            .session
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
        let write = self.session.process.reserve_user_write(destination)?;
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
            .session
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
        self.session
            .process
            .copy_from_user(source, &mut bytes[..length])?;
        Ok(console.object().try_write(&bytes[..length])?)
    }
}

impl VfsServices for DeferredProcessServices<'_> {
    fn directory_scope_create(
        &self,
        root: HandleValue,
        start: HandleValue,
        rights: Rights,
    ) -> Result<HandleValue, VfsServiceError> {
        crate::kernel::vfs::service::directory_scope_create(
            &self.session.process,
            root,
            start,
            rights,
        )
    }
    fn directory_get_metadata(
        &self,
        directory: HandleValue,
        path: UserSlice,
        follow: bool,
    ) -> Result<crate::kernel::vfs::Metadata, VfsServiceError> {
        crate::kernel::vfs::service::directory_get_metadata(
            &self.session.process,
            directory,
            path,
            follow,
        )
    }
    fn file_get_metadata(
        &self,
        file: HandleValue,
    ) -> Result<crate::kernel::vfs::Metadata, VfsServiceError> {
        crate::kernel::vfs::service::file_get_metadata(&self.session.process, file)
    }
    fn directory_get_self_metadata(
        &self,
        directory: HandleValue,
    ) -> Result<crate::kernel::vfs::Metadata, VfsServiceError> {
        crate::kernel::vfs::service::directory_get_self_metadata(&self.session.process, directory)
    }
    fn directory_set_metadata(
        &self,
        directory: HandleValue,
        path: UserSlice,
        follow: bool,
        update: crate::kernel::vfs::MetadataUpdate,
    ) -> Result<(), VfsServiceError> {
        crate::kernel::vfs::service::directory_set_metadata(
            &self.session.process,
            directory,
            path,
            follow,
            update,
        )
    }
    fn file_set_metadata(
        &self,
        file: HandleValue,
        update: crate::kernel::vfs::MetadataUpdate,
    ) -> Result<(), VfsServiceError> {
        crate::kernel::vfs::service::file_set_metadata(&self.session.process, file, update)
    }
    fn directory_rename(
        &self,
        source: HandleValue,
        path: UserSlice,
        destination: HandleValue,
        new_path: UserSlice,
    ) -> Result<(), VfsServiceError> {
        crate::kernel::vfs::service::directory_rename(
            &self.session.process,
            source,
            path,
            destination,
            new_path,
        )
    }
    fn directory_link(
        &self,
        source: HandleValue,
        path: UserSlice,
        destination: HandleValue,
        new_path: UserSlice,
    ) -> Result<(), VfsServiceError> {
        crate::kernel::vfs::service::directory_link(
            &self.session.process,
            source,
            path,
            destination,
            new_path,
        )
    }
    fn directory_symlink(
        &self,
        directory: HandleValue,
        path: UserSlice,
        target: UserSlice,
    ) -> Result<(), VfsServiceError> {
        crate::kernel::vfs::service::directory_symlink(
            &self.session.process,
            directory,
            path,
            target,
        )
    }
    fn directory_read_link(
        &self,
        directory: HandleValue,
        path: UserSlice,
    ) -> Result<crate::kernel::vfs::ScratchVec<u8>, VfsServiceError> {
        crate::kernel::vfs::service::directory_read_link(&self.session.process, directory, path)
    }
    fn directory_canonicalize(
        &self,
        directory: HandleValue,
        path: UserSlice,
    ) -> Result<crate::kernel::vfs::ScratchString, VfsServiceError> {
        crate::kernel::vfs::service::directory_canonicalize(&self.session.process, directory, path)
    }
    fn directory_remove_if(
        &self,
        directory: HandleValue,
        path: UserSlice,
        is_directory: bool,
        expected: u64,
    ) -> Result<(), VfsServiceError> {
        crate::kernel::vfs::service::directory_remove_if(
            &self.session.process,
            directory,
            path,
            is_directory,
            expected,
        )
    }
    fn directory_open_directory_nofollow(
        &self,
        directory: HandleValue,
        path: UserSlice,
        rights: Rights,
    ) -> Result<HandleValue, VfsServiceError> {
        crate::kernel::vfs::service::directory_open_directory_nofollow(
            &self.session.process,
            directory,
            path,
            rights,
        )
    }
    fn file_sync(&self, file: HandleValue, scope: u64) -> Result<(), VfsServiceError> {
        crate::kernel::vfs::service::file_sync(&self.session.process, file, scope)
    }
    fn file_lock(
        &self,
        file: HandleValue,
        mode: crate::kernel::vfs::locks::LockMode,
        deadline: u64,
    ) -> Result<(), VfsServiceError> {
        crate::kernel::vfs::service::file_lock(&self.session.process, file, mode, deadline, || {
            self.session.thread.snapshot().phase == UserThreadPhase::StopRequested
        })
    }
    fn file_unlock(&self, file: HandleValue) -> Result<(), VfsServiceError> {
        crate::kernel::vfs::service::file_unlock(&self.session.process, file)
    }
    fn directory_open_file_with_options(
        &self,
        directory: HandleValue,
        path: UserSlice,
        rights: Rights,
        options: u64,
        mode: u32,
    ) -> Result<HandleValue, VfsServiceError> {
        crate::kernel::vfs::service::directory_open_file_with_options(
            &self.session.process,
            directory,
            path,
            rights,
            options,
            mode,
        )
    }

    fn create_file(
        &self,
        directory: HandleValue,
        path: UserSlice,
        rights: Rights,
        mode: u32,
    ) -> Result<HandleValue, VfsServiceError> {
        crate::kernel::vfs::create_file(&self.session.process, directory, path, rights, mode)
    }

    fn create_directory(
        &self,
        directory: HandleValue,
        path: UserSlice,
        mode: u32,
    ) -> Result<(), VfsServiceError> {
        crate::kernel::vfs::create_directory(&self.session.process, directory, path, mode)
    }

    fn remove_entry(
        &self,
        directory: HandleValue,
        path: UserSlice,
        is_directory: bool,
    ) -> Result<(), VfsServiceError> {
        crate::kernel::vfs::remove_entry(&self.session.process, directory, path, is_directory)
    }

    fn resize_file(&self, file: HandleValue, length: u64) -> Result<(), VfsServiceError> {
        crate::kernel::vfs::resize_file(&self.session.process, file, length)
    }

    fn write_file_at(
        &self,
        file: HandleValue,
        offset: Option<u64>,
        input: Option<UserSlice>,
    ) -> Result<(u64, u64), VfsServiceError> {
        crate::kernel::vfs::write_file_at(&self.session.process, file, offset, input)
    }

    fn open_file(
        &self,
        root: HandleValue,
        path: UserSlice,
        rights: Rights,
    ) -> Result<HandleValue, VfsServiceError> {
        crate::kernel::vfs::open_file(&self.session.process, root, path, rights)
    }

    fn open_directory(
        &self,
        root: HandleValue,
        path: UserSlice,
        rights: Rights,
    ) -> Result<HandleValue, VfsServiceError> {
        crate::kernel::vfs::open_directory(&self.session.process, root, path, rights)
    }

    fn read_directory(
        &self,
        directory: HandleValue,
        cookie: u64,
    ) -> Result<crate::kernel::vfs::DirectoryPage, VfsServiceError> {
        crate::kernel::vfs::read_directory(&self.session.process, directory, cookie)
    }

    fn read_file_at(
        &self,
        file: HandleValue,
        offset: u64,
        output: Option<UserSlice>,
    ) -> Result<(u64, u64), VfsServiceError> {
        crate::kernel::vfs::read_file_at(&self.session.process, file, offset, output)
    }

    fn file_info(
        &self,
        file: HandleValue,
    ) -> Result<crate::kernel::vfs::FileInfo, VfsServiceError> {
        crate::kernel::vfs::file_info(&self.session.process, file)
    }

    fn directory_info(
        &self,
        directory: HandleValue,
    ) -> Result<crate::kernel::vfs::DirectoryInfo, VfsServiceError> {
        crate::kernel::vfs::directory_info(&self.session.process, directory)
    }
}

impl MemoryServices for DeferredProcessServices<'_> {
    fn create_vmo(
        &self,
        size: u64,
    ) -> Result<HandleValue, crate::kernel::mm::user_space::MemoryServiceError> {
        crate::kernel::mm::user_space::create_vmo(&self.session.process, size)
    }

    fn create_file_executable_vmo(
        &self,
        file: HandleValue,
    ) -> Result<HandleValue, crate::kernel::mm::user_space::MemoryServiceError> {
        crate::kernel::mm::user_space::create_file_executable_vmo(&self.session.process, file)
    }

    fn read_vmo(
        &self,
        vmo: HandleValue,
        offset: u64,
        output: Option<UserSlice>,
    ) -> Result<(), crate::kernel::mm::user_space::MemoryServiceError> {
        crate::kernel::mm::user_space::read_vmo(&self.session.process, vmo, offset, output)
    }

    fn write_vmo(
        &self,
        vmo: HandleValue,
        offset: u64,
        input: Option<UserSlice>,
    ) -> Result<(), crate::kernel::mm::user_space::MemoryServiceError> {
        crate::kernel::mm::user_space::write_vmo(&self.session.process, vmo, offset, input)
    }

    fn allocate_vmar(
        &self,
        parent: HandleValue,
        address: u64,
        size: u64,
    ) -> Result<HandleValue, crate::kernel::mm::user_space::MemoryServiceError> {
        crate::kernel::mm::user_space::allocate_vmar(&self.session.process, parent, address, size)
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
            &self.session.process,
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
        crate::kernel::mm::user_space::protect(
            &self.session.process,
            vmar,
            address,
            size,
            permissions,
        )
    }

    fn unmap_vmar(
        &self,
        vmar: HandleValue,
        address: u64,
        size: u64,
    ) -> Result<(), crate::kernel::mm::user_space::MemoryServiceError> {
        crate::kernel::mm::user_space::unmap(&self.session.process, vmar, address, size)
    }

    fn destroy_vmar(
        &self,
        vmar: HandleValue,
    ) -> Result<(), crate::kernel::mm::user_space::MemoryServiceError> {
        crate::kernel::mm::user_space::destroy_vmar(&self.session.process, vmar)
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
            &self.session.process,
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
            .session
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
            .session
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
            .session
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
            .session
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
            .session
            .process
            .resolve_handle::<ProcessBuilder>(builder, Rights::WRITE)?;
        builder.object().add_startup_capability(
            &self.session.process,
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
            .session
            .process
            .resolve_handle::<ProcessBuilder>(builder, Rights::WRITE)?;
        builder.object().seal()?;
        Ok(())
    }

    fn start_process_builder(
        &self,
        builder: HandleValue,
    ) -> Result<HandleValue, ProcessBuilderServiceError> {
        let started = crate::kernel::process::start_process_builder(&self.session.process, builder)
            .map_err(ProcessBuilderServiceError::Start)?;
        Ok(started.supervisor_handle())
    }

    fn abort_process_builder(
        &self,
        builder: HandleValue,
    ) -> Result<(), ProcessBuilderServiceError> {
        crate::kernel::process::abort_process_builder(&self.session.process, builder)?;
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

struct NativeProcessCalls<'process> {
    process: &'process Process,
}

impl NativeProcessCalls<'_> {
    fn dispatch_immediate(&self, invocation: NativeInvocation) -> NativeResult {
        native::dispatch_immediate(
            &ProcessServices {
                process: self.process,
            },
            invocation,
        )
    }
}

// SAFETY: The current Native ABI dispatcher contains only Never-blocking
// operations. Its Process services use non-sleeping spinlocked transactions,
// fallible nonblocking allocation, and resident-page copies; none enters the
// scheduler or retains the invocation. A future blocking ABI operation must
// return NativeCallAction::Unwind before it performs any work.
unsafe impl NativeCallHandler for NativeProcessCalls<'_> {
    unsafe fn dispatch(&self, invocation: NativeInvocation) -> NativeCallAction {
        if native::is_immediate(invocation.number()) {
            NativeCallAction::Return(self.dispatch_immediate(invocation))
        } else {
            NativeCallAction::Unwind
        }
    }
}

/// Stable ownership established once for the scheduler Thread's entire entry.
struct UserSession {
    thread: UserThread,
    process: Process,
}

impl UserSession {
    fn attach(
        current: crate::kernel::task::scheduler::CurrentUser,
        pin: &crate::kernel::task::scheduler::UserRunGuard,
    ) -> (Self, NonNull<UserExecution>) {
        validate_current_stack(current.stack);
        let execution = current.execution;
        // SAFETY: `current_user` returned the scheduler-owned payload of this
        // pinned current Thread. This borrow ends before `pin` is consumed by
        // run admission; later machine runs obtain a fresh pointer and pin.
        let thread = current.object;
        if thread.scheduler_id() != Some(current.thread) {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: native-user scheduler identity is inconsistent"
            ));
        }
        // SAFETY: the scheduler pin keeps the execution payload resident, and
        // publication arms its Process membership before this entry can run.
        let process = unsafe { execution.as_ref() }.process().clone();
        if process.id() != thread.process_id() {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: native-user Process ownership is inconsistent"
            ));
        }
        if process.image().family() != AbiFamily::Native
            || process.image().route() != ExecutionRoute::NativeKernel
        {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: native-user Thread has an invalid execution route"
            ));
        }
        let _ = pin;
        (Self { thread, process }, execution)
    }

    fn refresh_execution(
        &self,
        current: crate::kernel::task::scheduler::CurrentUser,
        pin: &crate::kernel::task::scheduler::UserRunGuard,
    ) -> NonNull<UserExecution> {
        if self.thread.scheduler_id() != Some(current.thread) {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: resumed native-user scheduler identity is inconsistent"
            ));
        }
        let execution = current.execution;
        if current.object.object_snapshot().koid != self.thread.object_snapshot().koid {
            crate::kernel::crash::fatal(format_args!(
                "HypeR: resumed native-user execution owner is inconsistent"
            ));
        }
        let _ = pin;
        execution
    }
}

/// Fixed scheduler entry for every native user Thread.
///
/// Process construction supplies no arbitrary kernel callback. Returning from
/// this function transfers to the ordinary scheduler thread-exit trampoline.
pub(in crate::kernel) extern "C" fn thread_entry(_argument: usize) {
    run_current()
}

fn run_current() {
    let pin = acquire_pin();
    let current = match crate::kernel::task::scheduler::current_user(&pin) {
        Ok(current) => current,
        Err(error) => fail_run("failed to identify current native-user Thread", error),
    };
    let (session, execution) = UserSession::attach(current, &pin);
    run_session(&session, pin, execution)
}

fn run_session(
    session: &UserSession,
    mut pin: crate::kernel::task::scheduler::UserRunGuard,
    mut execution: NonNull<UserExecution>,
) {
    loop {
        // SAFETY: `execution` was obtained with this run's `pin`. The prepared
        // and active run tokens below consume that same pin and retain its
        // no-migration/no-reclamation guarantee until machine exit.
        let execution_owner = unsafe { execution.as_ref() };
        if session.thread.snapshot().phase == UserThreadPhase::StopRequested {
            finish_pin(pin);
            return;
        }

        let prepared = match session
            .thread
            .prepare_run(pin, session.process.image_generation())
        {
            Ok(prepared) => prepared,
            Err((pin, RunAdmissionError::AdmissionClosed)) => {
                finish_pin(pin);
                return;
            }
            Err((_pin, error)) => fail_run("failed to reserve native-user generation", error),
        };

        // Masking closes the final pending-check-to-ERET window. Assembly
        // restores this masked state after capture; it is released before
        // address-space leave so local replacement IPIs can make progress.
        // SAFETY: The guard remains on this pinned continuation and is dropped
        // before any scheduling point.
        let interrupt_mask = unsafe { InterruptMaskGuard::<crate::hal::irq::LocalMask>::acquire() };
        let cpu = match crate::kernel::cpu::current_index() {
            Some(cpu) => cpu,
            None => crate::kernel::crash::fatal(format_args!(
                "HypeR: native-user Thread lost its CPU identity"
            )),
        };
        match crate::kernel::task::preempt::pending(cpu) {
            Ok(true) => {
                let aborted_pin = prepared.abort();
                drop(interrupt_mask);
                finish_pin(aborted_pin);
                (pin, execution) = reacquire_execution(session);
                continue;
            }
            Ok(false) => {}
            Err(error) => fail_run("failed to inspect native-user preemption state", error),
        }
        let kernel_access = match crate::hal::user::prepare_kernel_access(&interrupt_mask) {
            Ok(access) => access,
            Err(error) => {
                let pin = prepared.abort();
                drop(interrupt_mask);
                finish_pin(pin);
                fail_run("failed to establish native-user access isolation", error);
            }
        };

        let active_run = match prepared.commit() {
            Ok(active) => active,
            Err((pin, error)) => {
                drop(interrupt_mask);
                finish_pin(pin);
                if error == RunAdmissionError::AdmissionClosed {
                    return;
                }
                fail_run("failed to publish native-user generation", error);
            }
        };
        let active_address = match execution_owner
            .address_space()
            .activate(active_run.pin(), &kernel_access)
        {
            Ok(active) => active,
            Err(error) => fail_run("failed to activate native-user address space", error),
        };
        let binding = active_run.binding();
        // SAFETY: The scheduler pin, admitted generation, and active address
        // root uniquely own the stopped context until its return capability is
        // consumed. UserExecution uses UnsafeCell solely for this seam.
        let context = unsafe { &mut *execution_owner.context_ptr() };
        let stopped = {
            // Keep the CPU-affine service wholly inside this pinned machine
            // run. Architecture exit closes its publication before returning,
            // so both borrowed values are gone before the pin can be released.
            let calls = NativeProcessCalls {
                process: &session.process,
            };
            let service = NativeCallService::new(&calls);
            active_address.run_user(context, binding, kernel_access, &service)
        };

        drop(interrupt_mask);
        let (exit, proof) = stopped.leave();
        let (stopped_run, returned_pin) = active_run.stop_after_machine_exit(proof);
        if let Err(error) = crate::kernel::task::scheduler::finish_user_run(returned_pin) {
            fail_run("failed to release native-user execution pin", error);
        }

        let stop_requested = session.thread.snapshot().phase == UserThreadPhase::StopRequested;
        match exit {
            crate::hal::user::UserExit::NativeCall {
                invocation,
                completion,
            } => {
                if finish_native_call(
                    session,
                    invocation,
                    completion,
                    binding,
                    stopped_run,
                    stop_requested,
                ) == NativeRunAction::Stop
                {
                    return;
                }
            }
            crate::hal::user::UserExit::Interrupted { completion } => {
                let result = if stop_requested {
                    completion.discard(binding)
                } else {
                    completion.resume_interrupted(binding)
                };
                if let Err(failure) = result {
                    fail_completion(failure);
                }
                stopped_run.acknowledge_architecture_exit();
                if stop_requested {
                    return;
                }
            }
            crate::hal::user::UserExit::Fault { fault, completion } => {
                session.process.request_stop(fault_reason(fault));
                if let Err(failure) = completion.discard(binding) {
                    fail_completion(failure);
                }
                stopped_run.acknowledge_architecture_exit();
                return;
            }
        }
        (pin, execution) = reacquire_execution(session);
    }
}

fn finish_native_call(
    session: &UserSession,
    invocation: NativeInvocation,
    completion: crate::hal::user::ReturnCapability<'_>,
    binding: hyper::hal::user::UserRunBinding,
    stopped_run: StoppedUserRun,
    stop_requested: bool,
) -> NativeRunAction {
    if stop_requested {
        consume_discarded_call(completion, binding, stopped_run);
        return NativeRunAction::Stop;
    }

    let deferred = DeferredProcessServices { session };
    match native::dispatch_deferred(&deferred, invocation) {
        native::DeferredAction::Return(result) => {
            if session.thread.snapshot().phase == UserThreadPhase::StopRequested {
                consume_discarded_call(completion, binding, stopped_run);
                return NativeRunAction::Stop;
            }
            consume_returning_call(completion, binding, stopped_run, result);
            NativeRunAction::Resume
        }
        native::DeferredAction::Yield(result) => {
            consume_returning_call(completion, binding, stopped_run, result);
            if let Err(error) = crate::kernel::task::scheduler::yield_now() {
                fail_run("failed to yield native-user Thread", error);
            }
            NativeRunAction::Resume
        }
        native::DeferredAction::ExitThread { status } => {
            session
                .thread
                .request_stop(TerminalReason::ThreadExited { status });
            consume_discarded_call(completion, binding, stopped_run);
            NativeRunAction::Stop
        }
        native::DeferredAction::ExitProcess { status } => {
            session
                .process
                .request_stop(TerminalReason::ProcessExited { status });
            consume_discarded_call(completion, binding, stopped_run);
            NativeRunAction::Stop
        }
    }
}

fn consume_returning_call(
    completion: crate::hal::user::ReturnCapability<'_>,
    binding: hyper::hal::user::UserRunBinding,
    stopped_run: StoppedUserRun,
    result: NativeResult,
) {
    if let Err(failure) = completion.complete_native(binding, result) {
        fail_completion(failure);
    }
    stopped_run.acknowledge_architecture_exit();
}

fn consume_discarded_call(
    completion: crate::hal::user::ReturnCapability<'_>,
    binding: hyper::hal::user::UserRunBinding,
    stopped_run: StoppedUserRun,
) {
    if let Err(failure) = completion.discard(binding) {
        fail_completion(failure);
    }
    stopped_run.acknowledge_architecture_exit();
}

fn reacquire_execution(
    session: &UserSession,
) -> (
    crate::kernel::task::scheduler::UserRunGuard,
    NonNull<UserExecution>,
) {
    let pin = acquire_pin();
    let current = match crate::kernel::task::scheduler::current_user(&pin) {
        Ok(current) => current,
        Err(error) => fail_run("failed to refresh current native-user Thread", error),
    };
    let execution = session.refresh_execution(current, &pin);
    (pin, execution)
}

fn acquire_pin() -> crate::kernel::task::scheduler::UserRunGuard {
    match crate::kernel::task::scheduler::user_run_guard() {
        Ok(pin) => pin,
        Err(error) => fail_run("failed to pin native-user execution", error),
    }
}

fn validate_current_stack(stack: (usize, usize)) {
    let marker = 0usize;
    let pointer = core::ptr::from_ref(&marker).expose_provenance();
    if pointer < stack.0 || pointer >= stack.1 {
        crate::kernel::crash::fatal(format_args!(
            "HypeR: native-user runner is outside its scheduler stack"
        ));
    }
}

fn finish_pin(pin: crate::kernel::task::scheduler::UserRunGuard) {
    if let Err(error) = crate::kernel::task::scheduler::finish_user_run(pin) {
        fail_run("failed to finish native-user pin", error);
    }
}

fn fault_reason(fault: UserFault) -> TerminalReason {
    let class = match fault.kind() {
        UserFaultKind::InstructionAbort => 1,
        UserFaultKind::DataAbort => 2,
        UserFaultKind::Alignment => 3,
        UserFaultKind::IllegalInstruction => 4,
        UserFaultKind::SystemAccess => 5,
        UserFaultKind::Breakpoint => 6,
        UserFaultKind::OtherSynchronous => 7,
    };
    TerminalReason::Fault {
        class,
        code: fault.syndrome(),
    }
}

fn fail_completion(failure: crate::hal::user::CompletionFailure<'_>) -> ! {
    failure.abandon_with(|error| fail_run("native-user return ownership is inconsistent", error))
}

fn fail_run(context: &str, error: impl core::fmt::Debug) -> ! {
    crate::kernel::crash::fatal(format_args!("HypeR: {context}: {error:?}"))
}

fn atomic_wait_error(error: crate::kernel::process::atomic_wait::Error) -> ObjectServiceError {
    use crate::kernel::process::atomic_wait::Error;
    match error {
        Error::Process(error) => ObjectServiceError::Process(error),
        Error::Wait(error) => ObjectServiceError::Wait(error),
        Error::InvalidInput => ObjectServiceError::InvalidInput,
    }
}
