// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Capability-scoped task and object observability.
//!
//! Inspector objects carry immutable scope and visibility policy. Diagnostic
//! identifiers and cursors are deliberately observation-only: neither can be
//! converted back into operational authority.

use crate::kernel::accounting::{
    CommittedCharge, ResourceAmount, ResourceDomain, ResourceError, ResourceKind,
};
use crate::kernel::authority::Rights;
use crate::kernel::capability::{HandleScanCursor, HandleSnapshot};
use crate::kernel::object::{
    KernelObject, ObjectCreationError, ObjectKind, ObjectSnapshot, TransferClass,
    object_allocation_size, private,
};
use crate::kernel::process::{
    ProcessId, ProcessObject, ProcessScanCursor, ProcessSnapshot, TaskGroupId,
};
use crate::kernel::task::{ThreadObjectRegistryPhase, ThreadObjectScanCursor, ThreadRole};

// Keep capture pages deliberately small: scans execute on 16-KiB kernel
// stacks, and source directory snapshots coexist with ABI encoding frames.
pub(crate) const PROCESS_PAGE_CAPACITY: usize = 8;
pub(crate) const THREAD_PAGE_CAPACITY: usize = 8;
pub(crate) const OBJECT_PAGE_CAPACITY: usize = 8;
pub(crate) const HANDLE_PAGE_CAPACITY: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Scope {
    System,
    ResourceDomain(crate::kernel::accounting::ResourceDomainId),
    TaskGroup(TaskGroupId),
    Process(ProcessId),
}

impl Scope {
    fn permits(self, process: ProcessSnapshot) -> bool {
        match self {
            Self::System => true,
            Self::ResourceDomain(domain) => process.domain_id == domain,
            Self::TaskGroup(group) => process.group_id == group,
            Self::Process(id) => process.id == id,
        }
    }

    fn permits_task_group(
        self,
        group: TaskGroupId,
        domain: crate::kernel::accounting::ResourceDomainId,
    ) -> bool {
        match self {
            Self::System => true,
            Self::ResourceDomain(permitted) => permitted == domain,
            Self::TaskGroup(permitted) => permitted == group,
            Self::Process(_) => false,
        }
    }

    fn permits_resource_domain(self, domain: crate::kernel::accounting::ResourceDomainId) -> bool {
        match self {
            Self::System => true,
            Self::ResourceDomain(permitted) => permitted == domain,
            Self::TaskGroup(_) | Self::Process(_) => false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TaskVisibility(u32);

impl TaskVisibility {
    const PROCESS_BASIC: Self = Self(1 << 0);
    const THREAD_BASIC: Self = Self(1 << 1);
    const KERNEL_THREADS: Self = Self(1 << 2);
    const SYSTEM: Self =
        Self(Self::PROCESS_BASIC.0 | Self::THREAD_BASIC.0 | Self::KERNEL_THREADS.0);

    const fn contains(self, required: Self) -> bool {
        self.0 & required.0 == required.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ObjectVisibility(u32);

impl ObjectVisibility {
    const HANDLE_BASIC: Self = Self(1 << 0);
    const OBJECT_BASIC: Self = Self(1 << 1);
    const REFERENCE_COUNTS: Self = Self(1 << 2);
    const KERNEL_OBJECTS: Self = Self(1 << 3);
    const SYSTEM: Self = Self(
        Self::HANDLE_BASIC.0
            | Self::OBJECT_BASIC.0
            | Self::REFERENCE_COUNTS.0
            | Self::KERNEL_OBJECTS.0,
    );

    const fn contains(self, required: Self) -> bool {
        self.0 & required.0 == required.0
    }
}

#[derive(Debug)]
pub(crate) enum Error {
    AccessDenied,
    Allocation,
    NotFound,
    Object(ObjectCreationError),
    Process(crate::kernel::process::ProcessError),
    Resource(ResourceError),
    Scheduler(crate::kernel::task::scheduler::Error),
}

impl From<ObjectCreationError> for Error {
    fn from(error: ObjectCreationError) -> Self {
        Self::Object(error)
    }
}

impl From<ResourceError> for Error {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}

impl From<crate::kernel::process::ProcessError> for Error {
    fn from(error: crate::kernel::process::ProcessError) -> Self {
        Self::Process(error)
    }
}

impl From<crate::kernel::task::scheduler::Error> for Error {
    fn from(error: crate::kernel::task::scheduler::Error) -> Self {
        Self::Scheduler(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TaskThreadSnapshot {
    pub(crate) koid: crate::kernel::object::Koid,
    pub(crate) process_koid: Option<crate::kernel::object::Koid>,
    pub(crate) name: crate::kernel::task::scheduler::ThreadNameSnapshot,
    pub(crate) role: ThreadRole,
    pub(crate) registry_phase: ThreadObjectRegistryPhase,
}

pub(crate) struct Page<T: Copy, const N: usize> {
    entries: [Option<T>; N],
    len: usize,
    next: u64,
}

impl<T: Copy, const N: usize> Page<T, N> {
    fn empty() -> Self {
        Self {
            entries: [None; N],
            len: 0,
            next: 0,
        }
    }

    fn push(&mut self, entry: T) {
        if self.len >= N {
            crate::hal::cpu::halt();
        }
        self.entries[self.len] = Some(entry);
        self.len += 1;
    }

    pub(crate) fn entries(&self) -> impl Iterator<Item = &T> {
        self.entries[..self.len].iter().filter_map(Option::as_ref)
    }

    pub(crate) const fn len(&self) -> usize {
        self.len
    }

    pub(crate) const fn next(&self) -> u64 {
        self.next
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProcessHandleSnapshot {
    pub(crate) process_koid: crate::kernel::object::Koid,
    pub(crate) handle: HandleSnapshot,
}

/// Immutable authority to observe a bounded task hierarchy.
pub(crate) struct TaskInspector {
    scope: Scope,
    visibility: TaskVisibility,
    _object_charge: CommittedCharge,
}

impl TaskInspector {
    pub(crate) fn try_system(domain: &ResourceDomain) -> Result<Self, Error> {
        Ok(Self {
            scope: Scope::System,
            visibility: TaskVisibility::SYSTEM,
            _object_charge: reserve_object_charge::<Self>(domain)?,
        })
    }

    pub(crate) fn try_derive_process(
        &self,
        process: &ProcessObject,
        sponsor: &ResourceDomain,
    ) -> Result<Self, Error> {
        let target = process.snapshot();
        if !self.scope.permits(target) {
            return Err(Error::NotFound);
        }
        Ok(Self {
            scope: Scope::Process(target.id),
            visibility: TaskVisibility(self.visibility.0 & !TaskVisibility::KERNEL_THREADS.0),
            _object_charge: reserve_object_charge::<Self>(sponsor)?,
        })
    }

    pub(crate) fn try_derive_task_group(
        &self,
        group: &crate::kernel::process::TaskGroupObject,
        sponsor: &ResourceDomain,
    ) -> Result<Self, Error> {
        let group = group.group();
        if !self.scope.permits_task_group(group.id(), group.domain_id()) {
            return Err(Error::NotFound);
        }
        Ok(Self {
            scope: Scope::TaskGroup(group.id()),
            visibility: TaskVisibility(self.visibility.0 & !TaskVisibility::KERNEL_THREADS.0),
            _object_charge: reserve_object_charge::<Self>(sponsor)?,
        })
    }

    pub(crate) fn try_derive_resource_domain(
        &self,
        domain: &crate::kernel::accounting::ResourceDomainObject,
        sponsor: &ResourceDomain,
    ) -> Result<Self, Error> {
        if !self.scope.permits_resource_domain(domain.domain().id()) {
            return Err(Error::NotFound);
        }
        Ok(Self {
            scope: Scope::ResourceDomain(domain.domain().id()),
            visibility: TaskVisibility(self.visibility.0 & !TaskVisibility::KERNEL_THREADS.0),
            _object_charge: reserve_object_charge::<Self>(sponsor)?,
        })
    }

    pub(crate) fn scan_processes(
        &self,
        cursor: u64,
    ) -> Result<Page<ProcessSnapshot, PROCESS_PAGE_CAPACITY>, Error> {
        if !self.visibility.contains(TaskVisibility::PROCESS_BASIC) {
            return Err(Error::AccessDenied);
        }
        let source = crate::kernel::process::scan(ProcessScanCursor::from_token(cursor));
        let mut output = Page::empty();
        for process in source.entries() {
            let snapshot = process.snapshot();
            if self.scope.permits(snapshot) {
                output.push(snapshot);
            }
        }
        output.next = source.next().map_or(0, ProcessScanCursor::token);
        Ok(output)
    }

    pub(crate) fn scan_threads(
        &self,
        cursor: u64,
    ) -> Result<Page<TaskThreadSnapshot, THREAD_PAGE_CAPACITY>, Error> {
        if !self.visibility.contains(TaskVisibility::THREAD_BASIC) {
            return Err(Error::AccessDenied);
        }
        let token = usize::try_from(cursor).map_err(|_| Error::NotFound)?;
        let source = crate::kernel::task::scheduler::scan_thread_objects(
            ThreadObjectScanCursor::from_token(token),
        )?;
        let mut output = Page::empty();
        for thread in source.entries() {
            let (process, permitted) = match thread.object.process {
                Some(id) => {
                    let process = find_process(id);
                    let permitted = process.is_some_and(|snapshot| self.scope.permits(snapshot));
                    (process, permitted)
                }
                None => (
                    None,
                    self.scope == Scope::System
                        && self.visibility.contains(TaskVisibility::KERNEL_THREADS),
                ),
            };
            if permitted {
                output.push(TaskThreadSnapshot {
                    koid: thread.object.object.koid,
                    process_koid: process.map(|process| process.koid),
                    name: thread.name,
                    role: thread.object.role,
                    registry_phase: thread.phase,
                });
            }
        }
        output.next = match source.next() {
            Some(next) => u64::try_from(next.token()).map_err(|_| Error::Allocation)?,
            None => 0,
        };
        Ok(output)
    }
}

impl private::Sealed for TaskInspector {}
impl private::UserExportable for TaskInspector {}

impl KernelObject for TaskInspector {
    const KIND: ObjectKind = ObjectKind::TASK_INSPECTOR;
    const TRANSFER_CLASS: TransferClass = TransferClass::Leaf;
    const SUPPORTED_RIGHTS: Rights = Rights::DUPLICATE
        .union(Rights::TRANSFER)
        .union(Rights::INSPECT)
        .union(Rights::DERIVE);
}

/// Immutable authority to observe kernel objects and Process handle edges.
pub(crate) struct ObjectInspector {
    scope: Scope,
    visibility: ObjectVisibility,
    _object_charge: CommittedCharge,
}

impl ObjectInspector {
    pub(crate) fn try_system(domain: &ResourceDomain) -> Result<Self, Error> {
        Ok(Self {
            scope: Scope::System,
            visibility: ObjectVisibility::SYSTEM,
            _object_charge: reserve_object_charge::<Self>(domain)?,
        })
    }

    pub(crate) fn try_derive_process(
        &self,
        process: &ProcessObject,
        sponsor: &ResourceDomain,
    ) -> Result<Self, Error> {
        let target = process.snapshot();
        if !self.scope.permits(target) {
            return Err(Error::NotFound);
        }
        Ok(Self {
            scope: Scope::Process(target.id),
            visibility: ObjectVisibility(
                self.visibility.0
                    & (ObjectVisibility::HANDLE_BASIC.0 | ObjectVisibility::OBJECT_BASIC.0),
            ),
            _object_charge: reserve_object_charge::<Self>(sponsor)?,
        })
    }

    pub(crate) fn try_derive_task_group(
        &self,
        group: &crate::kernel::process::TaskGroupObject,
        sponsor: &ResourceDomain,
    ) -> Result<Self, Error> {
        let group = group.group();
        if !self.scope.permits_task_group(group.id(), group.domain_id()) {
            return Err(Error::NotFound);
        }
        Ok(Self {
            scope: Scope::TaskGroup(group.id()),
            visibility: ObjectVisibility(
                self.visibility.0
                    & (ObjectVisibility::HANDLE_BASIC.0 | ObjectVisibility::OBJECT_BASIC.0),
            ),
            _object_charge: reserve_object_charge::<Self>(sponsor)?,
        })
    }

    pub(crate) fn try_derive_resource_domain(
        &self,
        domain: &crate::kernel::accounting::ResourceDomainObject,
        sponsor: &ResourceDomain,
    ) -> Result<Self, Error> {
        if !self.scope.permits_resource_domain(domain.domain().id()) {
            return Err(Error::NotFound);
        }
        Ok(Self {
            scope: Scope::ResourceDomain(domain.domain().id()),
            visibility: ObjectVisibility(
                self.visibility.0
                    & (ObjectVisibility::HANDLE_BASIC.0 | ObjectVisibility::OBJECT_BASIC.0),
            ),
            _object_charge: reserve_object_charge::<Self>(sponsor)?,
        })
    }

    pub(crate) fn scan_objects(
        &self,
        cursor: u64,
    ) -> Result<Page<ObjectSnapshot, OBJECT_PAGE_CAPACITY>, Error> {
        if self.scope != Scope::System
            || !self.visibility.contains(ObjectVisibility::KERNEL_OBJECTS)
        {
            return Err(Error::AccessDenied);
        }
        let source = crate::kernel::object::scan(
            crate::kernel::object::ObjectScanCursor::from_token(cursor),
        );
        let mut output = Page::empty();
        for object in source.entries() {
            output.push(*object);
        }
        output.next = source
            .next()
            .map_or(0, crate::kernel::object::ObjectScanCursor::token);
        Ok(output)
    }

    pub(crate) fn scan_process_handles(
        &self,
        process_koid: u64,
        cursor: u64,
    ) -> Result<Page<ProcessHandleSnapshot, HANDLE_PAGE_CAPACITY>, Error> {
        if !self.visibility.contains(ObjectVisibility::HANDLE_BASIC)
            || !self.visibility.contains(ObjectVisibility::OBJECT_BASIC)
        {
            return Err(Error::AccessDenied);
        }
        let process = find_process_by_koid(process_koid, self.scope).ok_or(Error::NotFound)?;
        let token = usize::try_from(cursor).map_err(|_| Error::NotFound)?;
        let source = process.scan_handles(HandleScanCursor::from_token(token))?;
        let snapshot = process.snapshot();
        let mut output = Page::empty();
        for handle in source.entries() {
            output.push(ProcessHandleSnapshot {
                process_koid: snapshot.koid,
                handle: *handle,
            });
        }
        output.next = match source.next() {
            Some(next) => u64::try_from(next.token()).map_err(|_| Error::Allocation)?,
            None => 0,
        };
        Ok(output)
    }
}

impl private::Sealed for ObjectInspector {}
impl private::UserExportable for ObjectInspector {}

impl KernelObject for ObjectInspector {
    const KIND: ObjectKind = ObjectKind::OBJECT_INSPECTOR;
    const TRANSFER_CLASS: TransferClass = TransferClass::Leaf;
    const SUPPORTED_RIGHTS: Rights = Rights::DUPLICATE
        .union(Rights::TRANSFER)
        .union(Rights::INSPECT)
        .union(Rights::DERIVE);
}

fn find_process(id: ProcessId) -> Option<ProcessSnapshot> {
    find_process_matching(|snapshot| snapshot.id == id).map(|process| process.snapshot())
}

fn find_process_by_koid(
    koid: u64,
    scope: Scope,
) -> Option<crate::kernel::process::ProcessDiagnosticRef> {
    find_process_matching(|snapshot| snapshot.koid.get() == koid && scope.permits(snapshot))
}

fn find_process_matching(
    mut predicate: impl FnMut(ProcessSnapshot) -> bool,
) -> Option<crate::kernel::process::ProcessDiagnosticRef> {
    let mut cursor = ProcessScanCursor::start();
    loop {
        let page = crate::kernel::process::scan(cursor);
        for process in page.entries() {
            if predicate(process.snapshot()) {
                return Some(process.clone_diagnostic());
            }
        }
        cursor = page.next()?;
    }
}

fn reserve_object_charge<T: KernelObject>(
    domain: &ResourceDomain,
) -> Result<CommittedCharge, Error> {
    let bytes = object_allocation_size::<T>()
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(Error::Allocation)?;
    Ok(domain
        .reserve(
            ResourceAmount::ZERO
                .with(ResourceKind::KernelObjects, 1)
                .with(ResourceKind::KernelMemoryBytes, bytes),
        )?
        .commit())
}
