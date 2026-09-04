// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! `HypeR` Native syscall validation and dispatch.

use hyper::abi::native::{
    HYPER_NATIVE_ABI_REVISION, HYPER_NATIVE_FEATURE_CORE, HYPER_NATIVE_STATUS_ACCESS_DENIED,
    HYPER_NATIVE_STATUS_BAD_HANDLE, HYPER_NATIVE_STATUS_BAD_STATE,
    HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL, HYPER_NATIVE_STATUS_BUSY, HYPER_NATIVE_STATUS_CANCELLED,
    HYPER_NATIVE_STATUS_FAULT, HYPER_NATIVE_STATUS_INTERNAL, HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
    HYPER_NATIVE_STATUS_NO_MEMORY, HYPER_NATIVE_STATUS_NOT_SUPPORTED,
    HYPER_NATIVE_STATUS_PEER_CLOSED, HYPER_NATIVE_STATUS_RESOURCE_LIMIT,
    HYPER_NATIVE_STATUS_TIMED_OUT, HYPER_NATIVE_STATUS_WOULD_BLOCK, HYPER_NATIVE_SYS_ABI_QUERY,
    HYPER_NATIVE_SYS_CHANNEL_CREATE, HYPER_NATIVE_SYS_CHANNEL_READ, HYPER_NATIVE_SYS_CHANNEL_WRITE,
    HYPER_NATIVE_SYS_EVENT_CREATE, HYPER_NATIVE_SYS_EVENT_SIGNAL, HYPER_NATIVE_SYS_HANDLE_CLOSE,
    HYPER_NATIVE_SYS_HANDLE_DUPLICATE, HYPER_NATIVE_SYS_HANDLE_GET_INFO,
    HYPER_NATIVE_SYS_HANDLE_REPLACE, HYPER_NATIVE_SYS_OBJECT_GET_BASIC_INFO,
    HYPER_NATIVE_SYS_OBJECT_WAIT_ONE, HYPER_NATIVE_SYS_PROCESS_EXIT, HYPER_NATIVE_SYS_THREAD_EXIT,
    HYPER_NATIVE_SYS_THREAD_YIELD, HyperNativeHandleInfo, HyperNativeObjectBasicInfo,
    HyperNativeStatus, NativeInvocation, NativeResult,
};

use crate::kernel::accounting::ResourceError;
use crate::kernel::capability::{HandleError, HandleInfo, HandleValue, Rights};
use crate::kernel::ipc::{ChannelError, ChannelReadOutcome, ChannelServiceError, ReadBuffers};
use crate::kernel::mm::user_space::{
    AddressError, AddressSpaceError, MachineError, UserAddress, UserSlice,
};
use crate::kernel::object::{
    EventError, ObjectCreationError, ObjectWaitError, SignalWaitError, SignalWaitOutcome,
};
use crate::kernel::process::ProcessError;
use crate::kernel::task::TimedWaitError;

const HANDLE_INFO_SIZE: usize = core::mem::size_of::<HyperNativeHandleInfo>();
const OBJECT_BASIC_INFO_SIZE: usize = core::mem::size_of::<HyperNativeObjectBasicInfo>();
type Arguments = [u64; hyper::abi::native::HYPER_NATIVE_SYSCALL_ARGUMENT_REGISTERS];

/// Reports whether the current implementation is audited for masked entry.
///
/// New syscall numbers default to the deferred Thread path until their full
/// implementation is proven nonblocking and added deliberately here.
pub(in crate::kernel) const fn is_immediate(number: u64) -> bool {
    matches!(
        number,
        HYPER_NATIVE_SYS_ABI_QUERY
            | HYPER_NATIVE_SYS_HANDLE_CLOSE
            | HYPER_NATIVE_SYS_HANDLE_DUPLICATE
            | HYPER_NATIVE_SYS_HANDLE_REPLACE
            | HYPER_NATIVE_SYS_HANDLE_GET_INFO
            | HYPER_NATIVE_SYS_OBJECT_GET_BASIC_INFO
            | HYPER_NATIVE_SYS_EVENT_CREATE
            | HYPER_NATIVE_SYS_CHANNEL_CREATE
    )
}

/// Narrow Process service boundary consumed by the Native ABI adapter.
///
/// Implementations finish every handle-table operation before copying user
/// memory. Keeping those two phases separate prevents faults or backend work
/// from extending the Process lock graph.
pub(in crate::kernel) trait ImmediateServices {
    fn close_handle(&self, value: HandleValue) -> Result<(), ProcessError>;
    fn duplicate_handle(
        &self,
        value: HandleValue,
        rights: Rights,
    ) -> Result<HandleValue, ProcessError>;
    fn replace_handle(
        &self,
        value: HandleValue,
        rights: Rights,
    ) -> Result<HandleValue, ProcessError>;
    fn handle_info(
        &self,
        value: HandleValue,
        required_rights: Rights,
    ) -> Result<HandleInfo, ProcessError>;
    fn copy_to_user(&self, destination: UserSlice, source: &[u8]) -> Result<(), ProcessError>;
    fn create_event(&self) -> Result<HandleValue, ObjectServiceError>;
    fn create_channel(&self) -> Result<[HandleValue; 2], ChannelServiceError>;
}

/// Sleepable object services invoked only after architecture entry unwinds.
pub(in crate::kernel) trait DeferredServices {
    fn signal_event(
        &self,
        value: HandleValue,
        clear: u64,
        set: u64,
    ) -> Result<(), ObjectServiceError>;

    fn wait_one(
        &self,
        value: HandleValue,
        requested: u64,
        deadline: u64,
    ) -> Result<SignalWaitOutcome, ObjectServiceError>;

    fn write_channel(
        &self,
        endpoint: HandleValue,
        bytes: Option<UserSlice>,
        dispositions: Option<UserSlice>,
        disposition_count: usize,
    ) -> Result<(), ChannelServiceError>;

    fn read_channel(
        &self,
        endpoint: HandleValue,
        buffers: ReadBuffers,
    ) -> Result<ChannelReadOutcome, ChannelServiceError>;
}

#[derive(Debug)]
pub(in crate::kernel) enum ObjectServiceError {
    Process(ProcessError),
    Event(EventError),
    Wait(ObjectWaitError),
}

impl From<ProcessError> for ObjectServiceError {
    fn from(error: ProcessError) -> Self {
        Self::Process(error)
    }
}

impl From<EventError> for ObjectServiceError {
    fn from(error: EventError) -> Self {
        Self::Event(error)
    }
}

impl From<ObjectWaitError> for ObjectServiceError {
    fn from(error: ObjectWaitError) -> Self {
        Self::Wait(error)
    }
}

/// Policy action produced after architecture entry has fully unwound.
///
/// This value owns no architecture frame, CPU pin, or address-space guard, so
/// the caller may safely execute its scheduling or lifecycle effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::kernel) enum DeferredAction {
    Return(NativeResult),
    Yield(NativeResult),
    ExitThread { status: i64 },
    ExitProcess { status: i64 },
}

/// Dispatches one owned, nonblocking invocation.
///
/// Architecture entry retains its private raw frame but passes no frame borrow
/// or architecture offset into this adapter. Unknown numbers and malformed
/// fixed-width values fail closed. Operations that may block must use a
/// separate deferred dispatcher.
pub(in crate::kernel) fn dispatch_immediate(
    services: &impl ImmediateServices,
    invocation: NativeInvocation,
) -> NativeResult {
    let arguments = invocation.arguments();
    match invocation.number() {
        HYPER_NATIVE_SYS_ABI_QUERY => sys_abi_query(),
        HYPER_NATIVE_SYS_HANDLE_CLOSE => sys_handle_close(services, arguments),
        HYPER_NATIVE_SYS_HANDLE_DUPLICATE => sys_handle_duplicate(services, arguments),
        HYPER_NATIVE_SYS_HANDLE_REPLACE => sys_handle_replace(services, arguments),
        HYPER_NATIVE_SYS_HANDLE_GET_INFO => sys_handle_get_info(services, arguments),
        HYPER_NATIVE_SYS_OBJECT_GET_BASIC_INFO => sys_object_get_basic_info(services, arguments),
        HYPER_NATIVE_SYS_EVENT_CREATE => sys_event_create(services, arguments),
        HYPER_NATIVE_SYS_CHANNEL_CREATE => sys_channel_create(services, arguments),
        _ => sys_not_supported(),
    }
}

/// Executes one syscall after the machine context has returned to its Thread.
///
/// The returned action separates ABI decoding from scheduler and Process
/// policy. A blocking service may park the current Thread before producing the
/// action. Unknown calls remain ordinary returning failures even though they
/// conservatively use the deferred path.
pub(in crate::kernel) fn dispatch_deferred(
    services: &impl DeferredServices,
    invocation: NativeInvocation,
) -> DeferredAction {
    match invocation.number() {
        HYPER_NATIVE_SYS_THREAD_YIELD => sys_thread_yield(),
        HYPER_NATIVE_SYS_THREAD_EXIT => sys_thread_exit(invocation.arguments()),
        HYPER_NATIVE_SYS_PROCESS_EXIT => sys_process_exit(invocation.arguments()),
        HYPER_NATIVE_SYS_EVENT_SIGNAL => sys_event_signal(services, invocation.arguments()),
        HYPER_NATIVE_SYS_OBJECT_WAIT_ONE => sys_object_wait_one(services, invocation.arguments()),
        HYPER_NATIVE_SYS_CHANNEL_WRITE => sys_channel_write(services, invocation.arguments()),
        HYPER_NATIVE_SYS_CHANNEL_READ => sys_channel_read(services, invocation.arguments()),
        _ => DeferredAction::Return(sys_not_supported()),
    }
}

// Keep each syscall as a distinct machine frame. The routing match must not
// inherit the largest handler's stack requirement, and crash traces should
// identify the operation which was active at the fault boundary.

#[inline(never)]
fn sys_abi_query() -> NativeResult {
    success([HYPER_NATIVE_ABI_REVISION, HYPER_NATIVE_FEATURE_CORE])
}

#[inline(never)]
fn sys_handle_close(services: &impl ImmediateServices, arguments: &Arguments) -> NativeResult {
    let result = parse_handle(arguments[0]).and_then(|value| {
        services
            .close_handle(value)
            .map_err(status_from_process_error)
    });
    status_only(result)
}

#[inline(never)]
fn sys_handle_duplicate(services: &impl ImmediateServices, arguments: &Arguments) -> NativeResult {
    let result = parse_handle_and_rights(arguments[0], arguments[1]).and_then(|(value, rights)| {
        services
            .duplicate_handle(value, rights)
            .map_err(status_from_process_error)
    });
    handle_result(result)
}

#[inline(never)]
fn sys_handle_replace(services: &impl ImmediateServices, arguments: &Arguments) -> NativeResult {
    let result = parse_handle_and_rights(arguments[0], arguments[1]).and_then(|(value, rights)| {
        services
            .replace_handle(value, rights)
            .map_err(status_from_process_error)
    });
    handle_result(result)
}

#[inline(never)]
fn sys_handle_get_info(services: &impl ImmediateServices, arguments: &Arguments) -> NativeResult {
    let result =
        prepare_info_request(arguments, HANDLE_INFO_SIZE).and_then(|(value, destination)| {
            let info = services
                .handle_info(value, Rights::NONE)
                .map_err(status_from_process_error)?;
            let record = encode_handle_info(info);
            services
                .copy_to_user(destination, &record)
                .map_err(status_from_process_error)
        });
    status_only(result)
}

#[inline(never)]
fn sys_object_get_basic_info(
    services: &impl ImmediateServices,
    arguments: &Arguments,
) -> NativeResult {
    let result =
        prepare_info_request(arguments, OBJECT_BASIC_INFO_SIZE).and_then(|(value, destination)| {
            let info = services
                .handle_info(value, Rights::INSPECT)
                .map_err(status_from_process_error)?;
            let record = encode_object_basic_info(info);
            services
                .copy_to_user(destination, &record)
                .map_err(status_from_process_error)
        });
    status_only(result)
}

#[inline(never)]
fn sys_event_create(services: &impl ImmediateServices, arguments: &Arguments) -> NativeResult {
    if arguments[0] != 0 {
        return failure(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
    }
    handle_result(
        services
            .create_event()
            .map_err(status_from_object_service_error),
    )
}

#[inline(never)]
fn sys_channel_create(services: &impl ImmediateServices, arguments: &Arguments) -> NativeResult {
    if arguments[0] != 0 {
        return failure(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
    }
    match services
        .create_channel()
        .map_err(status_from_channel_service_error)
    {
        Ok([first, second]) => success([first.get(), second.get()]),
        Err(status) => failure(status),
    }
}

#[inline(never)]
fn sys_event_signal(services: &impl DeferredServices, arguments: &Arguments) -> DeferredAction {
    let result = parse_handle(arguments[0]).and_then(|value| {
        services
            .signal_event(value, arguments[1], arguments[2])
            .map_err(status_from_object_service_error)
    });
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
fn sys_not_supported() -> NativeResult {
    failure(HYPER_NATIVE_STATUS_NOT_SUPPORTED)
}

#[inline(never)]
fn sys_thread_yield() -> DeferredAction {
    DeferredAction::Yield(success([0, 0]))
}

#[inline(never)]
fn sys_thread_exit(arguments: &Arguments) -> DeferredAction {
    DeferredAction::ExitThread {
        status: arguments[0] as i64,
    }
}

#[inline(never)]
fn sys_process_exit(arguments: &Arguments) -> DeferredAction {
    DeferredAction::ExitProcess {
        status: arguments[0] as i64,
    }
}

#[inline(never)]
fn sys_object_wait_one(services: &impl DeferredServices, arguments: &Arguments) -> DeferredAction {
    let result = parse_handle(arguments[0]).and_then(|value| {
        services
            .wait_one(value, arguments[1], arguments[2])
            .map_err(status_from_object_service_error)
    });
    let result = match result {
        Ok(SignalWaitOutcome::Observed(snapshot)) => success([snapshot.signals().bits(), 0]),
        Ok(SignalWaitOutcome::TimedOut) => failure(HYPER_NATIVE_STATUS_TIMED_OUT),
        Ok(SignalWaitOutcome::Cancelled) => failure(HYPER_NATIVE_STATUS_CANCELLED),
        Err(status) => failure(status),
    };
    DeferredAction::Return(result)
}

#[inline(never)]
fn sys_channel_write(services: &impl DeferredServices, arguments: &Arguments) -> DeferredAction {
    let result = parse_channel_write(arguments).and_then(
        |(endpoint, bytes, dispositions, disposition_count)| {
            services
                .write_channel(endpoint, bytes, dispositions, disposition_count)
                .map_err(status_from_channel_service_error)
        },
    );
    DeferredAction::Return(status_only(result))
}

#[inline(never)]
fn sys_channel_read(services: &impl DeferredServices, arguments: &Arguments) -> DeferredAction {
    let result = parse_channel_read(arguments).and_then(|(endpoint, buffers)| {
        services
            .read_channel(endpoint, buffers)
            .map_err(status_from_channel_service_error)
    });
    let result = match result {
        Ok(ChannelReadOutcome::Received { bytes, handles }) => success([bytes, handles]),
        Ok(ChannelReadOutcome::BufferTooSmall { bytes, handles }) => NativeResult::for_syscall(
            HYPER_NATIVE_SYS_CHANNEL_READ,
            HYPER_NATIVE_STATUS_BUFFER_TOO_SMALL,
            [bytes, handles],
        ),
        Err(status) => failure(status),
    };
    DeferredAction::Return(result)
}

fn parse_handle(raw: u64) -> Result<HandleValue, HyperNativeStatus> {
    HandleValue::try_from_raw(raw).map_err(status_from_handle_error)
}

fn parse_handle_and_rights(
    raw_handle: u64,
    raw_rights: u64,
) -> Result<(HandleValue, Rights), HyperNativeStatus> {
    let value = parse_handle(raw_handle)?;
    let rights = Rights::from_bits(raw_rights).ok_or(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
    Ok((value, rights))
}

fn parse_channel_write(
    arguments: &Arguments,
) -> Result<(HandleValue, Option<UserSlice>, Option<UserSlice>, usize), HyperNativeStatus> {
    if arguments[1] != 0
        || arguments[3] > hyper::abi::native::HYPER_NATIVE_CHANNEL_MAX_MESSAGE_BYTES
        || arguments[5] > hyper::abi::native::HYPER_NATIVE_CHANNEL_MAX_MESSAGE_HANDLES
    {
        return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
    }
    let endpoint = parse_handle(arguments[0])?;
    let bytes = optional_user_slice(arguments[2], arguments[3])?;
    let disposition_bytes = arguments[5]
        .checked_mul(
            core::mem::size_of::<hyper::abi::native::HyperNativeChannelDisposition>() as u64,
        )
        .ok_or(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
    let dispositions = optional_user_slice(arguments[4], disposition_bytes)?;
    let disposition_count =
        usize::try_from(arguments[5]).map_err(|_| HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
    Ok((endpoint, bytes, dispositions, disposition_count))
}

fn parse_channel_read(
    arguments: &Arguments,
) -> Result<(HandleValue, ReadBuffers), HyperNativeStatus> {
    if arguments[1] != 0
        || arguments[3] > hyper::abi::native::HYPER_NATIVE_CHANNEL_MAX_MESSAGE_BYTES
        || arguments[5] > hyper::abi::native::HYPER_NATIVE_CHANNEL_MAX_MESSAGE_HANDLES
    {
        return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
    }
    let endpoint = parse_handle(arguments[0])?;
    let bytes = optional_user_slice(arguments[2], arguments[3])?;
    let handle_bytes = arguments[5]
        .checked_mul(core::mem::size_of::<u64>() as u64)
        .ok_or(HYPER_NATIVE_STATUS_INVALID_ARGUMENT)?;
    let handles = optional_user_slice(arguments[4], handle_bytes)?;
    Ok((endpoint, ReadBuffers { bytes, handles }))
}

fn optional_user_slice(
    raw_address: u64,
    length: u64,
) -> Result<Option<UserSlice>, HyperNativeStatus> {
    if length == 0 {
        return Ok(None);
    }
    UserSlice::new(UserAddress::new(raw_address), length)
        .map(Some)
        .map_err(status_from_address_error)
}

fn prepare_info_request(
    arguments: &Arguments,
    record_size: usize,
) -> Result<(HandleValue, UserSlice), HyperNativeStatus> {
    let record_size = u64::try_from(record_size).map_err(|_| HYPER_NATIVE_STATUS_INTERNAL)?;
    if arguments[2] != record_size {
        return Err(HYPER_NATIVE_STATUS_INVALID_ARGUMENT);
    }
    let value = parse_handle(arguments[0])?;
    let destination = UserSlice::new(UserAddress::new(arguments[1]), record_size)
        .map_err(status_from_address_error)?;
    Ok((value, destination))
}

fn encode_handle_info(info: HandleInfo) -> [u8; HANDLE_INFO_SIZE] {
    encode_handle_info_fields(info.kind.get(), info.flags.bits(), info.rights.bits())
}

fn encode_handle_info_fields(object_kind: u32, flags: u32, rights: u64) -> [u8; HANDLE_INFO_SIZE] {
    const KIND: usize = core::mem::offset_of!(HyperNativeHandleInfo, object_kind);
    const FLAGS: usize = core::mem::offset_of!(HyperNativeHandleInfo, flags);
    const RIGHTS: usize = core::mem::offset_of!(HyperNativeHandleInfo, rights);
    let mut record = [0_u8; HANDLE_INFO_SIZE];
    record[KIND..KIND + 4].copy_from_slice(&object_kind.to_ne_bytes());
    record[FLAGS..FLAGS + 4].copy_from_slice(&flags.to_ne_bytes());
    record[RIGHTS..RIGHTS + 8].copy_from_slice(&rights.to_ne_bytes());
    record
}

fn encode_object_basic_info(info: HandleInfo) -> [u8; OBJECT_BASIC_INFO_SIZE] {
    encode_object_basic_info_fields(info.koid.get(), info.kind.get())
}

fn encode_object_basic_info_fields(koid: u64, object_kind: u32) -> [u8; OBJECT_BASIC_INFO_SIZE] {
    const KOID: usize = core::mem::offset_of!(HyperNativeObjectBasicInfo, koid);
    const KIND: usize = core::mem::offset_of!(HyperNativeObjectBasicInfo, object_kind);
    let mut record = [0_u8; OBJECT_BASIC_INFO_SIZE];
    record[KOID..KOID + 8].copy_from_slice(&koid.to_ne_bytes());
    record[KIND..KIND + 4].copy_from_slice(&object_kind.to_ne_bytes());
    // The generated record's remaining bytes are the reserved-zero field.
    record
}

fn handle_result(result: Result<HandleValue, HyperNativeStatus>) -> NativeResult {
    match result {
        Ok(value) => success([value.get(), 0]),
        Err(status) => failure(status),
    }
}

fn status_only(result: Result<(), HyperNativeStatus>) -> NativeResult {
    match result {
        Ok(()) => success([0, 0]),
        Err(status) => failure(status),
    }
}

const fn success(values: [u64; 2]) -> NativeResult {
    NativeResult::new(hyper::abi::native::HYPER_NATIVE_STATUS_OK, values)
}

const fn failure(status: HyperNativeStatus) -> NativeResult {
    NativeResult::new(status, [0, 0])
}

fn status_from_process_error(error: ProcessError) -> HyperNativeStatus {
    match error {
        ProcessError::Allocation => HYPER_NATIVE_STATUS_NO_MEMORY,
        ProcessError::Handle(error) => status_from_handle_error(error),
        ProcessError::Object(error) => status_from_object_creation_error(error),
        ProcessError::Lifecycle(_) | ProcessError::AddressSpaceReferenced => {
            HYPER_NATIVE_STATUS_BAD_STATE
        }
        ProcessError::Resource(error) => status_from_resource_error(error),
        ProcessError::Scheduler(_) | ProcessError::TaskGroup(_) => HYPER_NATIVE_STATUS_INTERNAL,
        ProcessError::UserEntry(_) => HYPER_NATIVE_STATUS_NOT_SUPPORTED,
        ProcessError::UserMemory(error) => status_from_machine_error(error),
    }
}

const fn status_from_object_creation_error(error: ObjectCreationError) -> HyperNativeStatus {
    match error {
        ObjectCreationError::Allocation => HYPER_NATIVE_STATUS_NO_MEMORY,
        ObjectCreationError::KoidExhausted | ObjectCreationError::RegistrationExhausted => {
            HYPER_NATIVE_STATUS_RESOURCE_LIMIT
        }
    }
}

fn status_from_object_service_error(error: ObjectServiceError) -> HyperNativeStatus {
    match error {
        ObjectServiceError::Process(error) => status_from_process_error(error),
        ObjectServiceError::Event(error) => status_from_event_error(error),
        ObjectServiceError::Wait(error) => status_from_object_wait_error(error),
    }
}

fn status_from_channel_service_error(error: ChannelServiceError) -> HyperNativeStatus {
    match error {
        ChannelServiceError::InvalidDisposition => HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
        ChannelServiceError::Process(error) => status_from_process_error(error),
        ChannelServiceError::Channel(error) => status_from_channel_error(error),
        ChannelServiceError::Resource(error) => status_from_resource_error(error),
    }
}

const fn status_from_channel_error(error: ChannelError) -> HyperNativeStatus {
    match error {
        ChannelError::Allocation => HYPER_NATIVE_STATUS_NO_MEMORY,
        ChannelError::AllocationSize => HYPER_NATIVE_STATUS_INTERNAL,
        ChannelError::MessageTooLarge => HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
        ChannelError::EndpointClosed => HYPER_NATIVE_STATUS_BAD_STATE,
        ChannelError::PeerClosed => HYPER_NATIVE_STATUS_PEER_CLOSED,
        ChannelError::WouldBlock => HYPER_NATIVE_STATUS_WOULD_BLOCK,
        ChannelError::Busy | ChannelError::StaleMessage => HYPER_NATIVE_STATUS_BUSY,
        ChannelError::SequenceExhausted => HYPER_NATIVE_STATUS_RESOURCE_LIMIT,
        ChannelError::Resource(error) => status_from_resource_error(error),
    }
}

const fn status_from_event_error(error: EventError) -> HyperNativeStatus {
    match error {
        EventError::InvalidSignals => HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
        EventError::AllocationSize => HYPER_NATIVE_STATUS_INTERNAL,
        EventError::Resource(error) => status_from_resource_error(error),
        EventError::SignalWait(error) => status_from_signal_wait_error(error),
    }
}

const fn status_from_object_wait_error(error: ObjectWaitError) -> HyperNativeStatus {
    match error {
        ObjectWaitError::AllocationSize => HYPER_NATIVE_STATUS_INTERNAL,
        ObjectWaitError::Deadline(error) => status_from_deadline_error(error),
        ObjectWaitError::InvalidSignals => HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
        ObjectWaitError::Resource(error) => status_from_resource_error(error),
        ObjectWaitError::Signal(error) => status_from_signal_wait_error(error),
        ObjectWaitError::Timer(error) => status_from_timed_wait_error(error),
    }
}

const fn status_from_deadline_error(error: crate::kernel::time::Error) -> HyperNativeStatus {
    match error {
        crate::kernel::time::Error::Conversion(_) | crate::kernel::time::Error::DeadlineTooFar => {
            HYPER_NATIVE_STATUS_INVALID_ARGUMENT
        }
        _ => HYPER_NATIVE_STATUS_INTERNAL,
    }
}

const fn status_from_timed_wait_error(error: TimedWaitError) -> HyperNativeStatus {
    match error {
        TimedWaitError::Allocation
        | TimedWaitError::Time(crate::kernel::time::Error::TimerQueue(
            hyper::time::TimerQueueError::Allocation,
        )) => HYPER_NATIVE_STATUS_NO_MEMORY,
        TimedWaitError::Scheduler(_)
        | TimedWaitError::Time(_)
        | TimedWaitError::TimerCleanup(_) => HYPER_NATIVE_STATUS_INTERNAL,
    }
}

const fn status_from_signal_wait_error(error: SignalWaitError) -> HyperNativeStatus {
    match error {
        SignalWaitError::Allocation => HYPER_NATIVE_STATUS_NO_MEMORY,
        SignalWaitError::SequenceExhausted => HYPER_NATIVE_STATUS_RESOURCE_LIMIT,
        SignalWaitError::Scheduler(_) => HYPER_NATIVE_STATUS_INTERNAL,
    }
}

const fn status_from_handle_error(error: HandleError) -> HyperNativeStatus {
    match error {
        HandleError::Allocation => HYPER_NATIVE_STATUS_NO_MEMORY,
        HandleError::InvalidHandle | HandleError::WrongObjectType => HYPER_NATIVE_STATUS_BAD_HANDLE,
        HandleError::Busy => HYPER_NATIVE_STATUS_BUSY,
        HandleError::AccessDenied => HYPER_NATIVE_STATUS_ACCESS_DENIED,
        HandleError::UnsupportedRights | HandleError::UnsupportedFlags => {
            HYPER_NATIVE_STATUS_INVALID_ARGUMENT
        }
        HandleError::UnsupportedTransfer => HYPER_NATIVE_STATUS_NOT_SUPPORTED,
        HandleError::ObjectRetired | HandleError::TableRetired => HYPER_NATIVE_STATUS_BAD_STATE,
        HandleError::ActiveHandleLimit
        | HandleError::ReservationIdExhausted
        | HandleError::ReservationTooLarge
        | HandleError::TableFull => HYPER_NATIVE_STATUS_RESOURCE_LIMIT,
        HandleError::OutstandingReservation => HYPER_NATIVE_STATUS_BUSY,
        HandleError::ObjectAlreadyActive | HandleError::EmptyReservation => {
            HYPER_NATIVE_STATUS_INTERNAL
        }
    }
}

const fn status_from_resource_error(error: ResourceError) -> HyperNativeStatus {
    match error {
        ResourceError::Allocation => HYPER_NATIVE_STATUS_NO_MEMORY,
        ResourceError::HierarchyTooDeep
        | ResourceError::LimitExceeded { .. }
        | ResourceError::UsageOverflow { .. }
        | ResourceError::ChildCountExhausted
        | ResourceError::DomainIdExhausted => HYPER_NATIVE_STATUS_RESOURCE_LIMIT,
        ResourceError::DomainInactive(_)
        | ResourceError::OutstandingUsage
        | ResourceError::ActiveChildren
        | ResourceError::RetirementNotStarted => HYPER_NATIVE_STATUS_BAD_STATE,
        ResourceError::EmptyCharge | ResourceError::LimitBelowUsage { .. } => {
            HYPER_NATIVE_STATUS_INTERNAL
        }
    }
}

const fn status_from_machine_error(error: MachineError) -> HyperNativeStatus {
    match error {
        MachineError::Allocation | MachineError::Page(_) => HYPER_NATIVE_STATUS_NO_MEMORY,
        MachineError::Address(error) => status_from_address_error(error),
        MachineError::Logical(error) => status_from_logical_error(error),
        MachineError::Resource(error) => status_from_resource_error(error),
        MachineError::Residency(_) | MachineError::Transport => HYPER_NATIVE_STATUS_BUSY,
        MachineError::Unsupported => HYPER_NATIVE_STATUS_NOT_SUPPORTED,
        MachineError::Hal(_) | MachineError::Identifier(_) | MachineError::Vmo(_) => {
            HYPER_NATIVE_STATUS_INTERNAL
        }
        MachineError::SizeOverflow => HYPER_NATIVE_STATUS_FAULT,
    }
}

const fn status_from_address_error(_: AddressError) -> HyperNativeStatus {
    HYPER_NATIVE_STATUS_FAULT
}

const fn status_from_logical_error(
    error: AddressSpaceError<crate::kernel::mm::user_space::KernelPageError, ResourceError>,
) -> HyperNativeStatus {
    match error {
        AddressSpaceError::Account(error) => status_from_resource_error(error),
        AddressSpaceError::Allocation => HYPER_NATIVE_STATUS_NO_MEMORY,
        AddressSpaceError::Busy => HYPER_NATIVE_STATUS_BUSY,
        AddressSpaceError::Backend(error) => status_from_page_error(error),
        AddressSpaceError::BackingNotResident
        | AddressSpaceError::EmptyRange
        | AddressSpaceError::InvalidAddressSpace
        | AddressSpaceError::InvalidPermissions
        | AddressSpaceError::InvalidRange
        | AddressSpaceError::NotMapped
        | AddressSpaceError::Overlap
        | AddressSpaceError::ReadDenied
        | AddressSpaceError::SizeMismatch
        | AddressSpaceError::SizeOverflow
        | AddressSpaceError::StaleMapping
        | AddressSpaceError::StaleTransaction
        | AddressSpaceError::StaleVmar
        | AddressSpaceError::WriteDenied
        | AddressSpaceError::WritableExecutableBacking => HYPER_NATIVE_STATUS_FAULT,
        AddressSpaceError::IdentityExhausted => HYPER_NATIVE_STATUS_RESOURCE_LIMIT,
    }
}

const fn status_from_page_error(
    error: crate::kernel::mm::user_space::KernelPageError,
) -> HyperNativeStatus {
    match error {
        crate::kernel::mm::user_space::KernelPageError::Allocation(_) => {
            HYPER_NATIVE_STATUS_NO_MEMORY
        }
        crate::kernel::mm::user_space::KernelPageError::AddressOverflow
        | crate::kernel::mm::user_space::KernelPageError::Range => HYPER_NATIVE_STATUS_FAULT,
        crate::kernel::mm::user_space::KernelPageError::MissingLinearMap
        | crate::kernel::mm::user_space::KernelPageError::Unsupported => {
            HYPER_NATIVE_STATUS_INTERNAL
        }
    }
}

#[cfg(feature = "kernel-self-test")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SelfTestError {
    AbiQuery,
    UnknownNumber,
    InvalidHandle,
    InvalidRights,
    InvalidRecordSize,
    ChannelValidation,
    ObjectErrorMapping,
    ChannelErrorMapping,
    RecordEncoding,
    ValidationReachedService,
    DeferredDispatch,
}

#[cfg(feature = "kernel-self-test")]
pub(crate) fn run_self_test() -> Result<(), SelfTestError> {
    use core::cell::Cell;

    struct RejectingServices {
        calls: Cell<usize>,
    }

    impl RejectingServices {
        fn reached(&self) -> Result<(), SelfTestError> {
            if self.calls.get() == 0 {
                Ok(())
            } else {
                Err(SelfTestError::ValidationReachedService)
            }
        }
    }

    impl ImmediateServices for RejectingServices {
        fn close_handle(&self, _: HandleValue) -> Result<(), ProcessError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessError::Allocation)
        }

        fn duplicate_handle(&self, _: HandleValue, _: Rights) -> Result<HandleValue, ProcessError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessError::Allocation)
        }

        fn replace_handle(&self, _: HandleValue, _: Rights) -> Result<HandleValue, ProcessError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessError::Allocation)
        }

        fn handle_info(&self, _: HandleValue, _: Rights) -> Result<HandleInfo, ProcessError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessError::Allocation)
        }

        fn copy_to_user(&self, _: UserSlice, _: &[u8]) -> Result<(), ProcessError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessError::Allocation)
        }

        fn create_event(&self) -> Result<HandleValue, ObjectServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessError::Allocation.into())
        }

        fn create_channel(&self) -> Result<[HandleValue; 2], ChannelServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ChannelServiceError::Process(ProcessError::Allocation))
        }
    }

    impl DeferredServices for RejectingServices {
        fn signal_event(&self, _: HandleValue, _: u64, _: u64) -> Result<(), ObjectServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessError::Allocation.into())
        }

        fn wait_one(
            &self,
            _: HandleValue,
            _: u64,
            _: u64,
        ) -> Result<SignalWaitOutcome, ObjectServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ProcessError::Allocation.into())
        }

        fn write_channel(
            &self,
            _: HandleValue,
            _: Option<UserSlice>,
            _: Option<UserSlice>,
            _: usize,
        ) -> Result<(), ChannelServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ChannelServiceError::Process(ProcessError::Allocation))
        }

        fn read_channel(
            &self,
            _: HandleValue,
            _: ReadBuffers,
        ) -> Result<ChannelReadOutcome, ChannelServiceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(ChannelServiceError::Process(ProcessError::Allocation))
        }
    }

    fn invoke(number: u64, arguments: [u64; 6]) -> NativeInvocation {
        NativeInvocation::new(number, arguments, 0x1000)
    }

    let services = RejectingServices {
        calls: Cell::new(0),
    };
    let query = dispatch_immediate(&services, invoke(HYPER_NATIVE_SYS_ABI_QUERY, [u64::MAX; 6]));
    if query.status() != hyper::abi::native::HYPER_NATIVE_STATUS_OK
        || query.values() != &[HYPER_NATIVE_ABI_REVISION, HYPER_NATIVE_FEATURE_CORE]
    {
        return Err(SelfTestError::AbiQuery);
    }
    let unknown = dispatch_immediate(&services, invoke(u64::MAX, [u64::MAX; 6]));
    if unknown.status() != HYPER_NATIVE_STATUS_NOT_SUPPORTED || unknown.values() != &[0, 0] {
        return Err(SelfTestError::UnknownNumber);
    }
    if dispatch_deferred(
        &services,
        invoke(HYPER_NATIVE_SYS_THREAD_YIELD, [u64::MAX; 6]),
    ) != DeferredAction::Yield(success([0, 0]))
        || dispatch_deferred(
            &services,
            invoke(HYPER_NATIVE_SYS_THREAD_EXIT, [u64::MAX; 6]),
        ) != (DeferredAction::ExitThread { status: -1 })
        || dispatch_deferred(
            &services,
            invoke(HYPER_NATIVE_SYS_PROCESS_EXIT, [42, 0, 0, 0, 0, 0]),
        ) != (DeferredAction::ExitProcess { status: 42 })
        || dispatch_deferred(&services, invoke(u64::MAX, [u64::MAX; 6]))
            != DeferredAction::Return(failure(HYPER_NATIVE_STATUS_NOT_SUPPORTED))
    {
        return Err(SelfTestError::DeferredDispatch);
    }
    let timer_allocation = ObjectWaitError::Timer(TimedWaitError::Time(
        crate::kernel::time::Error::TimerQueue(hyper::time::TimerQueueError::Allocation),
    ));
    if status_from_object_wait_error(timer_allocation) != HYPER_NATIVE_STATUS_NO_MEMORY {
        return Err(SelfTestError::ObjectErrorMapping);
    }
    if status_from_channel_error(ChannelError::WouldBlock) != HYPER_NATIVE_STATUS_WOULD_BLOCK
        || status_from_channel_error(ChannelError::PeerClosed) != HYPER_NATIVE_STATUS_PEER_CLOSED
        || status_from_channel_error(ChannelError::MessageTooLarge)
            != HYPER_NATIVE_STATUS_INVALID_ARGUMENT
    {
        return Err(SelfTestError::ChannelErrorMapping);
    }
    let bad_handle = dispatch_immediate(
        &services,
        invoke(HYPER_NATIVE_SYS_HANDLE_CLOSE, [0, 0, 0, 0, 0, 0]),
    );
    if bad_handle.status() != HYPER_NATIVE_STATUS_BAD_HANDLE {
        return Err(SelfTestError::InvalidHandle);
    }
    let bad_rights = dispatch_immediate(
        &services,
        invoke(
            HYPER_NATIVE_SYS_HANDLE_DUPLICATE,
            [1_u64 << 24 | 1, u64::MAX, 0, 0, 0, 0],
        ),
    );
    if bad_rights.status() != HYPER_NATIVE_STATUS_INVALID_ARGUMENT {
        return Err(SelfTestError::InvalidRights);
    }
    let bad_size = dispatch_immediate(
        &services,
        invoke(
            HYPER_NATIVE_SYS_HANDLE_GET_INFO,
            [
                1_u64 << 24 | 1,
                0x2000,
                HANDLE_INFO_SIZE as u64 - 1,
                0,
                0,
                0,
            ],
        ),
    );
    if bad_size.status() != HYPER_NATIVE_STATUS_INVALID_ARGUMENT {
        return Err(SelfTestError::InvalidRecordSize);
    }
    let bad_channel_create = dispatch_immediate(
        &services,
        invoke(HYPER_NATIVE_SYS_CHANNEL_CREATE, [1, 0, 0, 0, 0, 0]),
    );
    let bad_channel_write = dispatch_deferred(
        &services,
        invoke(
            HYPER_NATIVE_SYS_CHANNEL_WRITE,
            [1_u64 << 24 | 1, 1, 0, 0, 0, 0],
        ),
    );
    let bad_channel_read = dispatch_deferred(
        &services,
        invoke(
            HYPER_NATIVE_SYS_CHANNEL_READ,
            [
                1_u64 << 24 | 1,
                0,
                0,
                0,
                0,
                hyper::abi::native::HYPER_NATIVE_CHANNEL_MAX_MESSAGE_HANDLES + 1,
            ],
        ),
    );
    if bad_channel_create.status() != HYPER_NATIVE_STATUS_INVALID_ARGUMENT
        || bad_channel_write
            != DeferredAction::Return(failure(HYPER_NATIVE_STATUS_INVALID_ARGUMENT))
        || bad_channel_read != DeferredAction::Return(failure(HYPER_NATIVE_STATUS_INVALID_ARGUMENT))
    {
        return Err(SelfTestError::ChannelValidation);
    }
    let handle_record = encode_handle_info_fields(0x1122_3344, 0x5566_7788, 0x99aa_bbcc_ddee_ff00);
    if handle_record
        != [
            0x44, 0x33, 0x22, 0x11, 0x88, 0x77, 0x66, 0x55, 0x00, 0xff, 0xee, 0xdd, 0xcc, 0xbb,
            0xaa, 0x99,
        ]
    {
        return Err(SelfTestError::RecordEncoding);
    }
    let object_record = encode_object_basic_info_fields(0x1122_3344_5566_7788, 0x99aa_bbcc);
    if object_record
        != [
            0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11, 0xcc, 0xbb, 0xaa, 0x99, 0, 0, 0, 0,
        ]
    {
        return Err(SelfTestError::RecordEncoding);
    }
    services.reached()
}
