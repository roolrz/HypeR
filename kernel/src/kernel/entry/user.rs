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
    self, AllocatingServices, ConsoleServiceError, DeferredServices, ImmediateServices,
    ObjectServiceError, ProcessBuilderServiceError,
};
use crate::kernel::accounting::{CommittedCharge, ResourceAmount, ResourceKind};
use crate::kernel::capability::{HandleInfo, HandleValue, Rights};
use crate::kernel::fs::BootFsServiceError;
use crate::kernel::ipc::{
    ByteChannelReadOutcome, ByteChannelServiceError, CapabilityChannelServiceError,
    CapabilityReceiveOutcome,
};
use crate::kernel::mm::user_space::UserSlice;
use crate::kernel::object::{self, Event, KernelObject, ObjectKind, SignalWaitOutcome};
use crate::kernel::process::{
    AbiFamily, ExecutionRoute, Process, ProcessBuilder, ProcessError, ProcessObject,
    RunAdmissionError, StartupCapability, StoppedUserRun, TerminalReason, UserExecution,
    UserThread, UserThreadPhase,
};
use crate::kernel::task::scheduler::CpuMask;

#[derive(Clone, Copy, Eq, PartialEq)]
enum NativeRunAction {
    Resume,
    Stop,
}

struct ProcessServices<'process> {
    process: &'process Process,
}

impl ImmediateServices for ProcessServices<'_> {
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

    fn copy_to_user(&self, destination: UserSlice, source: &[u8]) -> Result<(), ProcessError> {
        self.process.copy_to_user(destination, source)
    }
}

impl AllocatingServices for ProcessServices<'_> {
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
    fn create_event(&self) -> Result<HandleValue, ObjectServiceError> {
        let event = Event::try_new(&self.process.resource_domain())?;
        Ok(self
            .process
            .create_object(event, <Event as KernelObject>::SUPPORTED_RIGHTS)?)
    }
    fn create_byte_channel(&self) -> Result<[HandleValue; 2], ByteChannelServiceError> {
        crate::kernel::ipc::byte_channel_create(self.process)
    }
    fn create_capability_channel(&self) -> Result<[HandleValue; 2], CapabilityChannelServiceError> {
        crate::kernel::ipc::capability_channel_create(self.process)
    }
}

struct DeferredProcessServices<'session> {
    session: &'session UserSession,
}

struct ChargedBuilderInput {
    bytes: Vec<u8>,
    _charge: CommittedCharge,
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

impl AllocatingServices for DeferredProcessServices<'_> {
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
    fn create_event(&self) -> Result<HandleValue, ObjectServiceError> {
        ProcessServices {
            process: &self.session.process,
        }
        .create_event()
    }
    fn create_byte_channel(&self) -> Result<[HandleValue; 2], ByteChannelServiceError> {
        ProcessServices {
            process: &self.session.process,
        }
        .create_byte_channel()
    }
    fn create_capability_channel(&self) -> Result<[HandleValue; 2], CapabilityChannelServiceError> {
        ProcessServices {
            process: &self.session.process,
        }
        .create_capability_channel()
    }
}

impl DeferredServices for DeferredProcessServices<'_> {
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

    fn open_bootfs(
        &self,
        root: HandleValue,
        path: UserSlice,
        rights: Rights,
    ) -> Result<HandleValue, BootFsServiceError> {
        crate::kernel::fs::open_bootfs(&self.session.process, root, path, rights)
    }

    fn read_boot_file(
        &self,
        file: HandleValue,
        offset: u64,
        output: Option<UserSlice>,
    ) -> Result<(u64, u64), BootFsServiceError> {
        crate::kernel::fs::read_boot_file(&self.session.process, file, offset, output)
    }

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

    fn request_process_stop(&self, process: HandleValue) -> Result<(), ProcessError> {
        let process = self
            .session
            .process
            .resolve_handle::<ProcessObject>(process, Rights::REQUEST_STOP)?;
        process
            .object()
            .process()
            .request_stop(TerminalReason::Requested);
        Ok(())
    }
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
