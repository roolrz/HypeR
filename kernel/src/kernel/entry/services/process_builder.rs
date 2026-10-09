// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native process builder service calls under borrowed Process authority.

use super::DeferredProcessServices;
use super::affinity::AffinityInputError;
use crate::kernel::abi::native::{
    HierarchyServices, ProcessBuilderServiceError, ProcessBuilderServices,
};
use crate::kernel::accounting::{CommittedCharge, ResourceAmount, ResourceKind};
use crate::kernel::capability::{HandleValue, Rights};
use crate::kernel::mm::user_space::UserSlice;
use crate::kernel::object::ObjectKind;
use crate::kernel::process::{ProcessBuilder, ProcessError, StartupCapability};
use alloc::vec::Vec;

struct ChargedBuilderInput {
    bytes: Vec<u8>,
    _charge: Option<CommittedCharge>,
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
        if length == 0 {
            return Ok(ChargedBuilderInput {
                bytes: Vec::new(),
                _charge: None,
            });
        }
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
            _charge: Some(charge),
        })
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

    fn set_process_builder_data(
        &self,
        builder: HandleValue,
        data: Option<UserSlice>,
    ) -> Result<(), ProcessBuilderServiceError> {
        let builder = self
            .process
            .resolve_handle::<ProcessBuilder>(builder, Rights::WRITE)?;
        let bytes = self.copy_builder_input(data)?;
        builder.object().set_data(bytes.as_bytes())?;
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
        let affinity = self
            .copy_affinity(words, word_count)
            .map_err(|error| match error {
                AffinityInputError::Invalid => ProcessBuilderServiceError::InvalidInput,
                AffinityInputError::Memory(error) => ProcessBuilderServiceError::Process(error),
            })?;
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
    ) -> Result<[HandleValue; 2], ProcessBuilderServiceError> {
        let started = crate::kernel::process::start_process_builder(self.process, builder)
            .map_err(ProcessBuilderServiceError::Start)?;
        Ok([started.supervisor_handle(), started.startup_channel()])
    }

    fn abort_process_builder(
        &self,
        builder: HandleValue,
    ) -> Result<(), ProcessBuilderServiceError> {
        crate::kernel::process::abort_process_builder(self.process, builder)?;
        Ok(())
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
