// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native inspection service calls under borrowed Process authority.

use super::DeferredProcessServices;
use crate::kernel::abi::native::{InspectServices, SystemInspectServices};
use crate::kernel::accounting::ResourceDomainObject;
use crate::kernel::capability::{HandleValue, Rights};
use crate::kernel::inspect::{CpuInspector, MemoryInspector, ObjectInspector, TaskInspector};
use crate::kernel::object::KernelObject;
use crate::kernel::process::{ProcessObject, ProcessSnapshot, TaskGroupObject};

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

    fn read_object_details(
        &self,
        inspector: HandleValue,
        process: u64,
        target: u64,
        cursor: u64,
    ) -> Result<crate::kernel::object::diagnostics::ObjectDetails, crate::kernel::inspect::Error>
    {
        let inspector = self
            .process
            .resolve_handle::<ObjectInspector>(
                inspector,
                Rights::INSPECT.union(Rights::INSPECT_DETAILS),
            )
            .map_err(crate::kernel::inspect::Error::Process)?;
        inspector.object().read_details(process, target, cursor)
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
            .resolve_handle::<ObjectInspector>(inspector, ObjectInspector::DERIVATION_RIGHTS)
            .map_err(crate::kernel::inspect::Error::Process)?;
        let target = self
            .process
            .resolve_handle::<ProcessObject>(process, Rights::INSPECT)
            .map_err(crate::kernel::inspect::Error::Process)?;
        let derived = inspector
            .object()
            .try_derive_process(target.object(), &self.process.resource_domain())?;
        self.process
            .create_object(derived, ObjectInspector::DERIVATION_RIGHTS)
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
            .resolve_handle::<ObjectInspector>(inspector, ObjectInspector::DERIVATION_RIGHTS)
            .map_err(crate::kernel::inspect::Error::Process)?;
        let target = self
            .process
            .resolve_handle::<TaskGroupObject>(group, Rights::INSPECT)
            .map_err(crate::kernel::inspect::Error::Process)?;
        let derived = inspector
            .object()
            .try_derive_task_group(target.object(), &self.process.resource_domain())?;
        self.process
            .create_object(derived, ObjectInspector::DERIVATION_RIGHTS)
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
            .resolve_handle::<ObjectInspector>(inspector, ObjectInspector::DERIVATION_RIGHTS)
            .map_err(crate::kernel::inspect::Error::Process)?;
        let target = self
            .process
            .resolve_handle::<ResourceDomainObject>(domain, Rights::INSPECT)
            .map_err(crate::kernel::inspect::Error::Process)?;
        let derived = inspector
            .object()
            .try_derive_resource_domain(target.object(), &self.process.resource_domain())?;
        self.process
            .create_object(derived, ObjectInspector::DERIVATION_RIGHTS)
            .map_err(crate::kernel::inspect::Error::Process)
    }
}
