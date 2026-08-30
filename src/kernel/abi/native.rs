// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! `HypeR` Native syscall validation and dispatch.

use hyper::abi::native::{
    HYPER_NATIVE_ABI_REVISION, HYPER_NATIVE_FEATURE_CORE, HYPER_NATIVE_STATUS_ACCESS_DENIED,
    HYPER_NATIVE_STATUS_BAD_HANDLE, HYPER_NATIVE_STATUS_BAD_STATE, HYPER_NATIVE_STATUS_BUSY,
    HYPER_NATIVE_STATUS_FAULT, HYPER_NATIVE_STATUS_INTERNAL, HYPER_NATIVE_STATUS_INVALID_ARGUMENT,
    HYPER_NATIVE_STATUS_NO_MEMORY, HYPER_NATIVE_STATUS_NOT_SUPPORTED,
    HYPER_NATIVE_STATUS_RESOURCE_LIMIT, HYPER_NATIVE_SYS_ABI_QUERY, HYPER_NATIVE_SYS_HANDLE_CLOSE,
    HYPER_NATIVE_SYS_HANDLE_DUPLICATE, HYPER_NATIVE_SYS_HANDLE_GET_INFO,
    HYPER_NATIVE_SYS_HANDLE_REPLACE, HYPER_NATIVE_SYS_OBJECT_GET_BASIC_INFO, HyperNativeHandleInfo,
    HyperNativeObjectBasicInfo, HyperNativeStatus, NativeInvocation, NativeResult,
};

use crate::kernel::accounting::ResourceError;
use crate::kernel::capability::{HandleError, HandleInfo, HandleValue, Rights};
use crate::kernel::mm::user_space::{
    AddressError, AddressSpaceError, MachineError, UserAddress, UserSlice,
};
use crate::kernel::process::ProcessError;

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
        _ => sys_not_supported(),
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
fn sys_not_supported() -> NativeResult {
    failure(HYPER_NATIVE_STATUS_NOT_SUPPORTED)
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
        ProcessError::Lifecycle(_) | ProcessError::AddressSpaceReferenced => {
            HYPER_NATIVE_STATUS_BAD_STATE
        }
        ProcessError::Resource(error) => status_from_resource_error(error),
        ProcessError::Scheduler(_) | ProcessError::TaskGroup(_) => HYPER_NATIVE_STATUS_INTERNAL,
        ProcessError::UserEntry(_) => HYPER_NATIVE_STATUS_NOT_SUPPORTED,
        ProcessError::UserMemory(error) => status_from_machine_error(error),
    }
}

const fn status_from_handle_error(error: HandleError) -> HyperNativeStatus {
    match error {
        HandleError::Allocation => HYPER_NATIVE_STATUS_NO_MEMORY,
        HandleError::InvalidHandle | HandleError::WrongObjectType => HYPER_NATIVE_STATUS_BAD_HANDLE,
        HandleError::AccessDenied => HYPER_NATIVE_STATUS_ACCESS_DENIED,
        HandleError::UnsupportedRights | HandleError::UnsupportedFlags => {
            HYPER_NATIVE_STATUS_INVALID_ARGUMENT
        }
        HandleError::ObjectRetired | HandleError::TableRetired => HYPER_NATIVE_STATUS_BAD_STATE,
        HandleError::ActiveHandleLimit
        | HandleError::ReservationIdExhausted
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
    RecordEncoding,
    ValidationReachedService,
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
