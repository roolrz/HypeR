// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Native inspection service calls under borrowed Process authority.

use crate::kernel::capability::InvocationReceiver;

use super::ProcessServices;
use crate::kernel::abi::native::{InspectServices, SystemInspectServices};
use crate::kernel::accounting::ResourceDomainObject;
use crate::kernel::capability::{HandleValue, Rights};
use crate::kernel::inspect::{CpuInspector, MemoryInspector, ObjectInspector, TaskInspector};
use crate::kernel::object::KernelObject;
use crate::kernel::process::{ProcessObject, ProcessSnapshot, TaskGroupObject};

impl SystemInspectServices for ProcessServices<'_> {
    fn profile_start(
        &self,
        receiver: &InvocationReceiver,
        period: u64,
        seconds: u64,
    ) -> crate::kernel::profile::Result<u64> {
        receiver
            .resolve::<CpuInspector>(Rights::SET_ATTRIBUTES)
            .map_err(|_| hyper::abi::native::HYPER_NATIVE_STATUS_ACCESS_DENIED)?;
        crate::kernel::profile::start(period, seconds)
    }
    fn profile_stop(
        &self,
        receiver: &InvocationReceiver,
        generation: u64,
    ) -> crate::kernel::profile::Result<()> {
        receiver
            .resolve::<CpuInspector>(Rights::SET_ATTRIBUTES)
            .map_err(|_| hyper::abi::native::HYPER_NATIVE_STATUS_ACCESS_DENIED)?;
        crate::kernel::profile::stop(generation)
    }
    fn profile_read(
        &self,
        receiver: &InvocationReceiver,
        generation: u64,
        cpu: u64,
        cursor: u64,
        output: &mut [u8],
    ) -> crate::kernel::profile::Result<usize> {
        receiver
            .resolve::<CpuInspector>(Rights::INSPECT_DETAILS)
            .map_err(|_| hyper::abi::native::HYPER_NATIVE_STATUS_ACCESS_DENIED)?;
        crate::kernel::profile::query(generation, cpu, cursor, output)
    }

    fn trace_start(
        &self,
        receiver: &InvocationReceiver,
        config: &[u8; 56],
    ) -> crate::kernel::trace::Result<u64> {
        receiver
            .resolve::<CpuInspector>(Rights::SET_ATTRIBUTES)
            .map_err(|_| hyper::abi::native::HYPER_NATIVE_STATUS_ACCESS_DENIED)?;
        crate::kernel::trace::capture_start(config)
    }
    fn trace_stop(
        &self,
        receiver: &InvocationReceiver,
        generation: u64,
    ) -> crate::kernel::trace::Result<()> {
        receiver
            .resolve::<CpuInspector>(Rights::SET_ATTRIBUTES)
            .map_err(|_| hyper::abi::native::HYPER_NATIVE_STATUS_ACCESS_DENIED)?;
        crate::kernel::trace::capture_stop(generation)
    }
    fn trace_read(
        &self,
        receiver: &InvocationReceiver,
        generation: u64,
        cpu: u64,
        cursor: u64,
        output: &mut [u8],
    ) -> crate::kernel::trace::Result<usize> {
        receiver
            .resolve::<CpuInspector>(Rights::INSPECT_DETAILS)
            .map_err(|_| hyper::abi::native::HYPER_NATIVE_STATUS_ACCESS_DENIED)?;
        crate::kernel::trace::query(generation, cpu, cursor, output)
    }

    fn memory_statistics(
        &self,
        inspector: &InvocationReceiver,
        query: crate::kernel::inspect::memory::Query,
    ) -> Result<crate::kernel::inspect::memory::Statistics, crate::kernel::inspect::Error> {
        let inspector = inspector
            .resolve::<MemoryInspector>(Rights::INSPECT)
            .map_err(crate::kernel::process::ProcessError::from)
            .map_err(crate::kernel::inspect::Error::Process)?;
        inspector.object().statistics(query)
    }
    fn memory_observation(
        &self,
        inspector: &InvocationReceiver,
    ) -> Result<crate::kernel::inspect::MemoryObservation, crate::kernel::inspect::Error> {
        let inspector = inspector
            .resolve::<MemoryInspector>(Rights::INSPECT)
            .map_err(crate::kernel::process::ProcessError::from)
            .map_err(crate::kernel::inspect::Error::Process)?;
        inspector.object().snapshot()
    }

    fn cpu_observation(
        &self,
        inspector: &InvocationReceiver,
    ) -> Result<crate::kernel::task::scheduler::CpuTimeSnapshot, crate::kernel::inspect::Error>
    {
        let inspector = inspector
            .resolve::<CpuInspector>(Rights::INSPECT)
            .map_err(crate::kernel::process::ProcessError::from)
            .map_err(crate::kernel::inspect::Error::Process)?;
        Ok(inspector.object().snapshot())
    }
}

impl InspectServices for ProcessServices<'_> {
    fn scan_processes(
        &self,
        inspector: &InvocationReceiver,
        cursor: u64,
        output: &mut crate::kernel::inspect::Page<
            ProcessSnapshot,
            { crate::kernel::inspect::PROCESS_PAGE_CAPACITY },
        >,
    ) -> Result<(), crate::kernel::inspect::Error> {
        let inspector = inspector
            .resolve::<TaskInspector>(Rights::INSPECT)
            .map_err(crate::kernel::process::ProcessError::from)
            .map_err(crate::kernel::inspect::Error::Process)?;
        inspector.object().scan_processes(cursor, output)
    }

    fn scan_threads(
        &self,
        inspector: &InvocationReceiver,
        cursor: u64,
        output: &mut crate::kernel::inspect::Page<
            crate::kernel::inspect::TaskThreadSnapshot,
            { crate::kernel::inspect::THREAD_PAGE_CAPACITY },
        >,
    ) -> Result<(), crate::kernel::inspect::Error> {
        let inspector = inspector
            .resolve::<TaskInspector>(Rights::INSPECT)
            .map_err(crate::kernel::process::ProcessError::from)
            .map_err(crate::kernel::inspect::Error::Process)?;
        inspector.object().scan_threads(cursor, output)
    }

    fn read_object_details(
        &self,
        inspector: &InvocationReceiver,
        process: u64,
        target: u64,
        cursor: u64,
    ) -> Result<crate::kernel::object::diagnostics::ObjectDetails, crate::kernel::inspect::Error>
    {
        let inspector = inspector
            .resolve::<ObjectInspector>(Rights::INSPECT.union(Rights::INSPECT_DETAILS))
            .map_err(crate::kernel::process::ProcessError::from)
            .map_err(crate::kernel::inspect::Error::Process)?;
        inspector.object().read_details(process, target, cursor)
    }

    fn scan_objects(
        &self,
        inspector: &InvocationReceiver,
        cursor: u64,
        output: &mut crate::kernel::inspect::Page<
            crate::kernel::object::ObjectSnapshot,
            { crate::kernel::inspect::OBJECT_PAGE_CAPACITY },
        >,
    ) -> Result<(), crate::kernel::inspect::Error> {
        let inspector = inspector
            .resolve::<ObjectInspector>(Rights::INSPECT)
            .map_err(crate::kernel::process::ProcessError::from)
            .map_err(crate::kernel::inspect::Error::Process)?;
        inspector.object().scan_objects(cursor, output)
    }

    fn scan_process_handles(
        &self,
        inspector: &InvocationReceiver,
        process_koid: u64,
        cursor: u64,
    ) -> Result<
        crate::kernel::inspect::Page<
            crate::kernel::inspect::ProcessHandleSnapshot,
            { crate::kernel::inspect::HANDLE_PAGE_CAPACITY },
        >,
        crate::kernel::inspect::Error,
    > {
        let inspector = inspector
            .resolve::<ObjectInspector>(Rights::INSPECT)
            .map_err(crate::kernel::process::ProcessError::from)
            .map_err(crate::kernel::inspect::Error::Process)?;
        inspector
            .object()
            .scan_process_handles(process_koid, cursor)
    }

    fn derive_task_inspector(
        &self,
        inspector: &InvocationReceiver,
        process: HandleValue,
    ) -> Result<HandleValue, crate::kernel::inspect::Error> {
        let inspector = inspector
            .resolve::<TaskInspector>(<TaskInspector as KernelObject>::SUPPORTED_RIGHTS)
            .map_err(crate::kernel::process::ProcessError::from)
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
        inspector: &InvocationReceiver,
        process: HandleValue,
    ) -> Result<HandleValue, crate::kernel::inspect::Error> {
        let inspector = inspector
            .resolve::<ObjectInspector>(ObjectInspector::DERIVATION_RIGHTS)
            .map_err(crate::kernel::process::ProcessError::from)
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
        inspector: &InvocationReceiver,
        group: HandleValue,
    ) -> Result<HandleValue, crate::kernel::inspect::Error> {
        let inspector = inspector
            .resolve::<TaskInspector>(<TaskInspector as KernelObject>::SUPPORTED_RIGHTS)
            .map_err(crate::kernel::process::ProcessError::from)
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
        inspector: &InvocationReceiver,
        group: HandleValue,
    ) -> Result<HandleValue, crate::kernel::inspect::Error> {
        let inspector = inspector
            .resolve::<ObjectInspector>(ObjectInspector::DERIVATION_RIGHTS)
            .map_err(crate::kernel::process::ProcessError::from)
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
        inspector: &InvocationReceiver,
        domain: HandleValue,
    ) -> Result<HandleValue, crate::kernel::inspect::Error> {
        let inspector = inspector
            .resolve::<TaskInspector>(<TaskInspector as KernelObject>::SUPPORTED_RIGHTS)
            .map_err(crate::kernel::process::ProcessError::from)
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
        inspector: &InvocationReceiver,
        domain: HandleValue,
    ) -> Result<HandleValue, crate::kernel::inspect::Error> {
        let inspector = inspector
            .resolve::<ObjectInspector>(ObjectInspector::DERIVATION_RIGHTS)
            .map_err(crate::kernel::process::ProcessError::from)
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
