// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native executable loading into unpublished Process address spaces.

use alloc::vec::Vec;
use core::mem::size_of;

use hyper::exec::{
    elf::{Image, ImageKind, Machine, Relocation, SegmentPermissions},
    startup::Layout as StartupStackLayout,
};
use hyper::mm::{PAGE_SIZE, UniqueFallibleArc};

use super::{ImageError, MachineAbi, ProcessImage};
use crate::kernel::accounting::{ResourceAmount, ResourceDomain, ResourceError, ResourceKind};
use crate::kernel::mm::user_space::{
    MachineError, NativeAddressSpace, NativeImageSegment, Permissions, UserAddress, UserSlice,
};

const USER_ROOT_BASE: u64 = 0x10_0000;
const USER_ROOT_END: u64 = 0x1_0000_0000;
const PIE_MAPPING_BASE: u64 = 0x20_0000;
const INTERPRETER_MAPPING_BASE: u64 = 0x1000_0000;
const DYNAMIC_LIBRARY_MAPPING_BASE: u64 = 0x2000_0000;
const INITIAL_STACK_SIZE: u64 = 256 * 1024;
pub(crate) const INITIAL_STACK_TOP: u64 = 0xffff_0000;
const MAXIMUM_IMAGE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug)]
pub(crate) enum Error {
    Address,
    Allocation,
    Elf(hyper::exec::elf::Error),
    Image(ImageError),
    Machine(MachineError),
    Resource(ResourceError),
    Scheduler(crate::kernel::task::scheduler::Error),
    UnsupportedMachine,
}

impl From<MachineError> for Error {
    fn from(error: MachineError) -> Self {
        Self::Machine(error)
    }
}

impl From<crate::kernel::task::scheduler::Error> for Error {
    fn from(error: crate::kernel::task::scheduler::Error) -> Self {
        Self::Scheduler(error)
    }
}

impl From<ResourceError> for Error {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}

pub(crate) struct LoadedProcessImage {
    pub(crate) image: ProcessImage,
    pub(crate) address_space: UniqueFallibleArc<NativeAddressSpace>,
}

pub(crate) fn load_native(
    snapshot: &crate::kernel::vfs::ExecutableSnapshot,
    domain: ResourceDomain,
    initial_stack: StartupStackLayout,
) -> Result<LoadedProcessImage, Error> {
    let bytes = snapshot.bytes();
    if bytes.len() > MAXIMUM_IMAGE_BYTES as usize {
        return Err(Error::Address);
    }
    let allocation = Image::process_allocation_plan(bytes).map_err(Error::Elf)?;
    let scratch_bytes = allocation
        .parser_bytes()
        .and_then(|bytes| {
            allocation
                .segment_capacity()
                .checked_mul(core::mem::size_of::<NativeImageSegment>())
                .and_then(|segments| bytes.checked_add(segments))
        })
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or(Error::Allocation)?;
    let _scratch_charge = domain
        .reserve(ResourceAmount::ZERO.with(ResourceKind::KernelMemoryBytes, scratch_bytes))?
        .commit();
    let executable = Image::parse_process_with_plan(bytes, allocation).map_err(Error::Elf)?;
    validate_host_machine(executable.machine())?;
    let load_bias = select_load_bias(&executable)?;
    validate_layout(&executable, load_bias)?;

    let root = UserSlice::new(
        UserAddress::new(USER_ROOT_BASE),
        USER_ROOT_END - USER_ROOT_BASE,
    )
    .map_err(|_| Error::Address)?;
    let address_space = NativeAddressSpace::try_new(domain.clone(), root)?;
    match prepare_address_space(
        &executable,
        snapshot,
        load_bias,
        initial_stack,
        &address_space,
        &domain,
    ) {
        Ok(image) => Ok(LoadedProcessImage {
            image,
            address_space,
        }),
        Err(error) => {
            retire_failed_address_space(address_space);
            Err(error)
        }
    }
}

fn prepare_address_space(
    executable: &Image<'_>,
    snapshot: &crate::kernel::vfs::ExecutableSnapshot,
    load_bias: u64,
    initial_stack: StartupStackLayout,
    address_space: &NativeAddressSpace,
    domain: &ResourceDomain,
) -> Result<ProcessImage, Error> {
    let mut segments = prepare_segments(executable, snapshot, load_bias, address_space)?;
    let interpreter = match executable.interpreter() {
        Some(path) => Some(load_interpreter(
            path,
            executable.machine(),
            address_space,
            domain,
        )?),
        None => None,
    };
    if let Some(loaded) = &interpreter {
        validate_dynamic_layout(executable, load_bias, loaded)?;
    }
    if interpreter.is_none() {
        apply_relocations(executable, load_bias, &mut segments)?;
    }

    let stack_base = INITIAL_STACK_TOP
        .checked_sub(INITIAL_STACK_SIZE)
        .ok_or(Error::Address)?;
    let stack_range = UserSlice::new(UserAddress::new(stack_base), INITIAL_STACK_SIZE)
        .map_err(|_| Error::Address)?;
    if initial_stack.stack_top() != INITIAL_STACK_TOP
        || initial_stack.stack_pointer() < stack_range.base().get()
        || initial_stack.stack_pointer() >= stack_range.end().get()
        || !initial_stack.stack_pointer().is_multiple_of(16)
    {
        return Err(Error::Address);
    }
    let stack = NativeImageSegment::try_new(address_space, stack_range, Permissions::read_write())?;

    let pin = crate::kernel::task::scheduler::preempt_disable()?;
    let (entry, interpreter_base) = interpreter.as_ref().map_or(
        (relocated_address(load_bias, executable.entry())?, 0),
        |loaded| (loaded.entry, loaded.load_bias),
    );
    let install_result = install_segments(
        address_space,
        segments,
        interpreter.map(|loaded| loaded.segments),
        stack,
        &pin,
    );
    // Mapping installation needs CPU affinity, but it does not define a
    // scheduling point. In particular, the bootstrap loader runs before the
    // first transition to an IRQ-enabled Thread context. Restore only the
    // pin's accounting here and leave deferred scheduling to the caller's
    // ordinary return or blocking boundary.
    crate::kernel::task::scheduler::preempt_enable_without_reschedule(pin)?;
    install_result?;

    let program_entry = relocated_address(load_bias, executable.entry())?;
    ProcessImage::try_native_with_auxiliary(
        machine_abi(executable.machine()),
        UserAddress::new(entry),
        UserAddress::new(initial_stack.stack_pointer()),
        UserAddress::new(0),
        hyper::exec::startup::AuxiliaryValues {
            program_header: relocated_address(load_bias, executable.program_header_address())?,
            program_header_entry_size: 56,
            program_header_count: u64::from(executable.program_header_count()),
            interpreter_base,
            program_entry,
        },
    )
    .map_err(Error::Image)
}

struct LoadedInterpreter {
    segments: Vec<NativeImageSegment>,
    entry: u64,
    load_bias: u64,
    mapping_start: u64,
    mapping_end: u64,
}

fn validate_dynamic_layout(
    executable: &Image<'_>,
    load_bias: u64,
    interpreter: &LoadedInterpreter,
) -> Result<(), Error> {
    let executable_end = relocated_address(load_bias, executable.maximum_mapping_address())?;
    if executable_end > interpreter.mapping_start
        || interpreter.mapping_end > DYNAMIC_LIBRARY_MAPPING_BASE
    {
        return Err(Error::Address);
    }
    Ok(())
}

fn load_interpreter(
    path: &str,
    executable_machine: Machine,
    address_space: &NativeAddressSpace,
    domain: &ResourceDomain,
) -> Result<LoadedInterpreter, Error> {
    let file = crate::kernel::vfs::lookup(path, domain)
        .map_err(|_| Error::Address)?
        .ok_or(Error::Address)?;
    let snapshot = file.into_executable().ok_or(Error::Address)?;
    let bytes = snapshot.bytes();
    if bytes.len() > MAXIMUM_IMAGE_BYTES as usize {
        return Err(Error::Address);
    }
    let plan = Image::allocation_plan(bytes).map_err(Error::Elf)?;
    let image = Image::parse_with_plan(bytes, plan).map_err(Error::Elf)?;
    if image.machine() != executable_machine || image.kind() != ImageKind::PositionIndependent {
        return Err(Error::UnsupportedMachine);
    }
    let load_bias = INTERPRETER_MAPPING_BASE
        .checked_sub(image.minimum_mapping_address())
        .filter(|bias| bias.is_multiple_of(PAGE_SIZE))
        .ok_or(Error::Address)?;
    validate_layout(&image, load_bias)?;
    let mut segments = prepare_segments(&image, &snapshot, load_bias, address_space)?;
    apply_relocations(&image, load_bias, &mut segments)?;
    Ok(LoadedInterpreter {
        segments,
        entry: relocated_address(load_bias, image.entry())?,
        load_bias,
        mapping_start: relocated_address(load_bias, image.minimum_mapping_address())?,
        mapping_end: relocated_address(load_bias, image.maximum_mapping_address())?,
    })
}

fn machine_abi(machine: Machine) -> MachineAbi {
    match machine {
        Machine::Aarch64 => MachineAbi::Aarch64,
        Machine::Riscv64 => MachineAbi::Riscv64,
    }
}

fn validate_host_machine(machine: Machine) -> Result<(), Error> {
    use crate::hal::user::HostMachine;
    if matches!(
        (machine, crate::hal::user::host_machine()),
        (Machine::Aarch64, HostMachine::Aarch64) | (Machine::Riscv64, HostMachine::Riscv64)
    ) {
        Ok(())
    } else {
        Err(Error::UnsupportedMachine)
    }
}

fn retire_failed_address_space(address_space: UniqueFallibleArc<NativeAddressSpace>) {
    if let Err(failure) = NativeAddressSpace::retire_unpublished(address_space) {
        let (error, retained) = failure.into_parts();
        crate::pr_err!(
            "HypeR: retaining a failed Native image address space after cleanup error: {error:?}"
        );
        drop(retained);
    }
}

fn select_load_bias(image: &Image<'_>) -> Result<u64, Error> {
    match image.kind() {
        ImageKind::Executable => Ok(0),
        ImageKind::PositionIndependent => PIE_MAPPING_BASE
            .checked_sub(image.minimum_mapping_address())
            .filter(|bias| bias.is_multiple_of(PAGE_SIZE))
            .ok_or(Error::Address),
    }
}

fn validate_layout(image: &Image<'_>, load_bias: u64) -> Result<(), Error> {
    let start = relocated_address(load_bias, image.minimum_mapping_address())?;
    let end = relocated_address(load_bias, image.maximum_mapping_address())?;
    let stack_guard = INITIAL_STACK_TOP
        .checked_sub(INITIAL_STACK_SIZE)
        .and_then(|base| base.checked_sub(PAGE_SIZE))
        .ok_or(Error::Address)?;
    if start < USER_ROOT_BASE || start >= end || end > stack_guard {
        return Err(Error::Address);
    }
    let mut total = 0u64;
    for segment in image.segments() {
        total = total
            .checked_add(segment.mapping_size())
            .ok_or(Error::Address)?;
        if total > MAXIMUM_IMAGE_BYTES {
            return Err(Error::Address);
        }
    }
    Ok(())
}

fn prepare_segments(
    image: &Image<'_>,
    snapshot: &crate::kernel::vfs::ExecutableSnapshot,
    load_bias: u64,
    address_space: &NativeAddressSpace,
) -> Result<Vec<NativeImageSegment>, Error> {
    let mut prepared = Vec::new();
    prepared
        .try_reserve_exact(image.segments().len())
        .map_err(|_| Error::Allocation)?;
    for segment in image.segments() {
        let address = relocated_address(load_bias, segment.mapping_address())?;
        let range = UserSlice::new(UserAddress::new(address), segment.mapping_size())
            .map_err(|_| Error::Address)?;
        let loaded = NativeImageSegment::try_from_snapshot(
            address_space,
            range,
            map_permissions(segment.permissions()),
            snapshot.storage().clone(),
            segment.file_offset() - segment.data_offset(),
            segment.data().len() as u64,
            segment.data_offset(),
        )?;
        prepared.push(loaded);
    }
    Ok(prepared)
}

fn apply_relocations(
    image: &Image<'_>,
    load_bias: u64,
    segments: &mut [NativeImageSegment],
) -> Result<(), Error> {
    for relocation in image.relocations() {
        let target = UserAddress::new(relocated_address(load_bias, relocation.target())?);
        let segment = segment_containing(segments, target).ok_or(Error::Address)?;
        let value = match relocation {
            Relocation::Relative { addend, .. } => add_signed(load_bias, addend)?,
            Relocation::RelativeInPlace { .. } => segment
                .read_word(target)?
                .checked_add(load_bias)
                .ok_or(Error::Address)?,
        };
        segment.write_word(target, value)?;
    }
    Ok(())
}

fn install_segments(
    address_space: &NativeAddressSpace,
    segments: Vec<NativeImageSegment>,
    interpreter: Option<Vec<NativeImageSegment>>,
    stack: NativeImageSegment,
    pin: &(impl hyper::cpu::PinnedExecution + 'static),
) -> Result<(), Error> {
    for segment in segments {
        segment.install(address_space, pin)?;
    }
    if let Some(interpreter) = interpreter {
        for segment in interpreter {
            segment.install(address_space, pin)?;
        }
    }
    stack.install(address_space, pin)?;
    Ok(())
}

fn segment_containing(
    segments: &mut [NativeImageSegment],
    address: UserAddress,
) -> Option<&mut NativeImageSegment> {
    let end = address.checked_add(size_of::<u64>() as u64)?;
    segments
        .iter_mut()
        .find(|segment| segment.range().base() <= address && end <= segment.range().end())
}

const fn map_permissions(permissions: SegmentPermissions) -> Permissions {
    if permissions.executable() {
        Permissions::read_execute()
    } else if permissions.writable() {
        Permissions::read_write()
    } else {
        Permissions::read_only()
    }
}

fn relocated_address(load_bias: u64, address: u64) -> Result<u64, Error> {
    load_bias.checked_add(address).ok_or(Error::Address)
}

fn add_signed(base: u64, addend: i64) -> Result<u64, Error> {
    if addend >= 0 {
        base.checked_add(addend as u64).ok_or(Error::Address)
    } else {
        base.checked_sub(addend.unsigned_abs())
            .ok_or(Error::Address)
    }
}
