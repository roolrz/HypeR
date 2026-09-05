// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Bounded System-V-shaped initial-stack construction for Native processes.
//!
//! Architecture entry receives only a stack pointer. This module owns the
//! machine-visible word order while the C runtime derives convenient counts
//! and views without introducing a second, HypeR-specific entry structure.

use alloc::vec::Vec;

use crate::mm::PAGE_SIZE;

const WORD_BYTES: usize = core::mem::size_of::<u64>();
type AbiStartupHandle = crate::abi::native::HyperNativeStartupHandle;
const STARTUP_HANDLE_BYTES: usize = core::mem::size_of::<AbiStartupHandle>();
const STARTUP_HANDLE_ALIGNMENT: usize = core::mem::align_of::<AbiStartupHandle>();
const STARTUP_HANDLE_PURPOSE_OFFSET: usize = core::mem::offset_of!(AbiStartupHandle, purpose);
const STARTUP_HANDLE_FLAGS_OFFSET: usize = core::mem::offset_of!(AbiStartupHandle, flags);
const STARTUP_HANDLE_VALUE_OFFSET: usize = core::mem::offset_of!(AbiStartupHandle, handle);
const STACK_ALIGNMENT: usize = 16;
const MAXIMUM_INITIAL_BYTES: usize = 64 * 1024;
const AT_NULL: u64 = 0;
const AT_PAGESZ: u64 = 6;
const AT_ENTRY: u64 = 9;
const STANDARD_AUXILIARY_ENTRIES: usize = 2;
const HYPER_AUXILIARY_ENTRIES: usize = 2;
const AUXILIARY_TERMINATORS: usize = 1;
const AUXILIARY_ENTRY_COUNT: usize =
    STANDARD_AUXILIARY_ENTRIES + HYPER_AUXILIARY_ENTRIES + AUXILIARY_TERMINATORS;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    AddressOverflow,
    Allocation,
    EmbeddedNul,
    LayoutMismatch,
    TooLarge,
}

/// One semantic capability transferred to a process at first entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StartupHandle {
    pub purpose: u32,
    pub handle: u64,
}

/// Immutable sizing decision shared by the loader and final encoder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Layout {
    stack_top: u64,
    stack_pointer: u64,
    records_offset: usize,
    strings_offset: usize,
    total_bytes: usize,
    argument_count: usize,
    environment_count: usize,
    handle_count: usize,
}

/// Complete byte image copied to the exact initial user-stack range.
pub struct EncodedStack {
    base: u64,
    bytes: Vec<u8>,
}

impl Layout {
    pub fn try_new(
        stack_top: u64,
        arguments: &[&str],
        environment: &[&str],
        handle_count: usize,
    ) -> Result<Self, Error> {
        validate_strings(arguments)?;
        validate_strings(environment)?;
        let pointer_words = 1usize
            .checked_add(arguments.len())
            .and_then(|words| words.checked_add(1))
            .and_then(|words| words.checked_add(environment.len()))
            .and_then(|words| words.checked_add(1))
            .and_then(|words| words.checked_add(AUXILIARY_ENTRY_COUNT * 2))
            .ok_or(Error::TooLarge)?;
        let records_offset = align_up(
            pointer_words
                .checked_mul(WORD_BYTES)
                .ok_or(Error::TooLarge)?,
            STARTUP_HANDLE_ALIGNMENT,
        )?;
        let strings_offset = records_offset
            .checked_add(
                handle_count
                    .checked_mul(STARTUP_HANDLE_BYTES)
                    .ok_or(Error::TooLarge)?,
            )
            .ok_or(Error::TooLarge)?;
        let string_bytes = encoded_string_bytes(arguments)?
            .checked_add(encoded_string_bytes(environment)?)
            .ok_or(Error::TooLarge)?;
        let total_bytes = align_up(
            strings_offset
                .checked_add(string_bytes)
                .ok_or(Error::TooLarge)?,
            STACK_ALIGNMENT,
        )?;
        if total_bytes > MAXIMUM_INITIAL_BYTES {
            return Err(Error::TooLarge);
        }
        let stack_pointer = stack_top
            .checked_sub(u64::try_from(total_bytes).map_err(|_| Error::TooLarge)?)
            .ok_or(Error::AddressOverflow)?;
        if !stack_pointer.is_multiple_of(STACK_ALIGNMENT as u64) {
            return Err(Error::AddressOverflow);
        }
        Ok(Self {
            stack_top,
            stack_pointer,
            records_offset,
            strings_offset,
            total_bytes,
            argument_count: arguments.len(),
            environment_count: environment.len(),
            handle_count,
        })
    }

    pub const fn stack_pointer(self) -> u64 {
        self.stack_pointer
    }

    /// Exclusive top of the encoded initial-stack range.
    pub const fn stack_top(self) -> u64 {
        self.stack_top
    }

    pub fn encode(
        self,
        entry: u64,
        arguments: &[&str],
        environment: &[&str],
        handles: &[StartupHandle],
    ) -> Result<EncodedStack, Error> {
        if Self::try_new(self.stack_top, arguments, environment, handles.len())? != self {
            return Err(Error::LayoutMismatch);
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(self.total_bytes)
            .map_err(|_| Error::Allocation)?;
        bytes.resize(self.total_bytes, 0);

        let mut word_offset = 0usize;
        write_word(
            &mut bytes,
            &mut word_offset,
            u64::try_from(arguments.len()).map_err(|_| Error::TooLarge)?,
        )?;
        let mut string_offset = self.strings_offset;
        for argument in arguments {
            write_word(
                &mut bytes,
                &mut word_offset,
                pointer(self.stack_pointer, string_offset)?,
            )?;
            write_string(&mut bytes, &mut string_offset, argument)?;
        }
        write_word(&mut bytes, &mut word_offset, 0)?;
        for variable in environment {
            write_word(
                &mut bytes,
                &mut word_offset,
                pointer(self.stack_pointer, string_offset)?,
            )?;
            write_string(&mut bytes, &mut string_offset, variable)?;
        }
        write_word(&mut bytes, &mut word_offset, 0)?;
        write_auxiliary(&mut bytes, &mut word_offset, AT_PAGESZ, PAGE_SIZE)?;
        write_auxiliary(&mut bytes, &mut word_offset, AT_ENTRY, entry)?;
        write_auxiliary(
            &mut bytes,
            &mut word_offset,
            crate::abi::native::HYPER_NATIVE_AUXV_STARTUP_HANDLES,
            pointer(self.stack_pointer, self.records_offset)?,
        )?;
        write_auxiliary(
            &mut bytes,
            &mut word_offset,
            crate::abi::native::HYPER_NATIVE_AUXV_STARTUP_HANDLE_COUNT,
            u64::try_from(handles.len()).map_err(|_| Error::TooLarge)?,
        )?;
        write_auxiliary(&mut bytes, &mut word_offset, AT_NULL, 0)?;
        if word_offset > self.records_offset {
            return Err(Error::LayoutMismatch);
        }

        for (index, handle) in handles.iter().enumerate() {
            let offset = self
                .records_offset
                .checked_add(
                    index
                        .checked_mul(STARTUP_HANDLE_BYTES)
                        .ok_or(Error::TooLarge)?,
                )
                .ok_or(Error::TooLarge)?;
            write_u32(
                &mut bytes,
                offset + STARTUP_HANDLE_PURPOSE_OFFSET,
                handle.purpose,
            )?;
            write_u32(&mut bytes, offset + STARTUP_HANDLE_FLAGS_OFFSET, 0)?;
            write_u64(
                &mut bytes,
                offset + STARTUP_HANDLE_VALUE_OFFSET,
                handle.handle,
            )?;
        }
        if string_offset > self.total_bytes {
            return Err(Error::LayoutMismatch);
        }
        Ok(EncodedStack {
            base: self.stack_pointer,
            bytes,
        })
    }
}

impl EncodedStack {
    pub const fn base(&self) -> u64 {
        self.base
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

fn validate_strings(strings: &[&str]) -> Result<(), Error> {
    if strings.iter().any(|value| value.as_bytes().contains(&0)) {
        Err(Error::EmbeddedNul)
    } else {
        Ok(())
    }
}

fn encoded_string_bytes(strings: &[&str]) -> Result<usize, Error> {
    strings.iter().try_fold(0usize, |total, value| {
        total
            .checked_add(value.len())
            .and_then(|bytes| bytes.checked_add(1))
            .ok_or(Error::TooLarge)
    })
}

fn write_auxiliary(
    output: &mut [u8],
    offset: &mut usize,
    kind: u64,
    value: u64,
) -> Result<(), Error> {
    write_word(output, offset, kind)?;
    write_word(output, offset, value)
}

fn write_word(output: &mut [u8], offset: &mut usize, value: u64) -> Result<(), Error> {
    write_u64(output, *offset, value)?;
    *offset = offset.checked_add(WORD_BYTES).ok_or(Error::TooLarge)?;
    Ok(())
}

fn write_string(output: &mut [u8], offset: &mut usize, value: &str) -> Result<(), Error> {
    let end = offset.checked_add(value.len()).ok_or(Error::TooLarge)?;
    let target = output.get_mut(*offset..end).ok_or(Error::LayoutMismatch)?;
    target.copy_from_slice(value.as_bytes());
    *offset = end.checked_add(1).ok_or(Error::TooLarge)?;
    if output.get(end) != Some(&0) {
        return Err(Error::LayoutMismatch);
    }
    Ok(())
}

fn write_u32(output: &mut [u8], offset: usize, value: u32) -> Result<(), Error> {
    let end = offset.checked_add(4).ok_or(Error::TooLarge)?;
    output
        .get_mut(offset..end)
        .ok_or(Error::LayoutMismatch)?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn write_u64(output: &mut [u8], offset: usize, value: u64) -> Result<(), Error> {
    let end = offset.checked_add(8).ok_or(Error::TooLarge)?;
    output
        .get_mut(offset..end)
        .ok_or(Error::LayoutMismatch)?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn pointer(base: u64, offset: usize) -> Result<u64, Error> {
    base.checked_add(u64::try_from(offset).map_err(|_| Error::AddressOverflow)?)
        .ok_or(Error::AddressOverflow)
}

fn align_up(value: usize, alignment: usize) -> Result<usize, Error> {
    value
        .checked_add(alignment - 1)
        .map(|rounded| rounded & !(alignment - 1))
        .ok_or(Error::TooLarge)
}
