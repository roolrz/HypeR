// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native memory service calls under borrowed Process authority.

use super::DeferredProcessServices;
use crate::kernel::abi::native::MemoryServices;
use crate::kernel::capability::HandleValue;
use crate::kernel::mm::user_space::UserSlice;

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
        exact: bool,
    ) -> Result<(HandleValue, u64), crate::kernel::mm::user_space::MemoryServiceError> {
        crate::kernel::mm::user_space::allocate_vmar(self.process, parent, address, size, exact)
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
