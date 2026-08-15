//! Architecture-neutral thread objects and execution payloads.

use alloc::alloc::{alloc_zeroed, dealloc};
use alloc::boxed::Box;
use core::alloc::Layout;
use core::ptr::NonNull;

const DEFAULT_KERNEL_STACK_SIZE: usize = 64 * 1024;
const KERNEL_STACK_ALIGNMENT: usize = 16;
const THREAD_NAME_CAPACITY: usize = 32;

pub type KernelThreadEntry = extern "C" fn(usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ThreadId(u64);

impl ThreadId {
    pub const BOOTSTRAP: Self = Self(0);

    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

pub const THREAD_PRIORITY_LEVELS: usize = 32;

/// Fixed scheduler priority. Lower numeric values run first.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ThreadPriority(u8);

impl ThreadPriority {
    pub const HIGHEST: Self = Self(0);
    pub const NORMAL: Self = Self(16);
    pub const LOWEST: Self = Self((THREAD_PRIORITY_LEVELS - 1) as u8);

    pub const fn new(value: u8) -> Option<Self> {
        if (value as usize) < THREAD_PRIORITY_LEVELS {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThreadState {
    Dormant,
    Ready,
    Running,
    Idle,
    Blocked,
    Terminated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum QueueMembership {
    None,
    Ready { cpu: usize, priority: u8 },
    Waiting { queue: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct QueueLinks {
    pub previous: Option<ThreadId>,
    pub next: Option<ThreadId>,
    pub membership: QueueMembership,
}

impl QueueLinks {
    pub(crate) const EMPTY: Self = Self {
        previous: None,
        next: None,
        membership: QueueMembership::None,
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AddressSpaceId(pub u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtualMachineId(pub u64);

pub struct UserExecution {
    pub address_space: AddressSpaceId,
    pub context: crate::arch::UserContext,
}

pub struct VcpuExecution {
    pub virtual_machine: VirtualMachineId,
    pub vcpu_id: u32,
    pub context: crate::arch::VcpuContext,
}

pub enum ThreadExecution {
    Kernel,
    User(Box<UserExecution>),
    Vcpu(Box<VcpuExecution>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionKind {
    Kernel,
    User,
    Vcpu,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Allocation,
    InvalidStackLayout,
    NameTooLong,
    Vgic(crate::arch::VgicError),
}

impl From<crate::arch::VgicError> for Error {
    fn from(error: crate::arch::VgicError) -> Self {
        Self::Vgic(error)
    }
}

/// A schedulable execution entity.
///
/// Every thread owns a kernel scheduling context and, except for the bootstrap
/// thread, a private kernel stack. User and vCPU architectural state is an
/// attached execution payload; it is deliberately separate from the context
/// used while the scheduler and exception handlers execute at EL2.
pub struct Thread {
    id: ThreadId,
    cpu_index: usize,
    name: ThreadName,
    priority: ThreadPriority,
    state: ThreadState,
    queue_links: QueueLinks,
    context: crate::arch::ThreadContext,
    kernel_stack: Option<KernelStack>,
    execution: ThreadExecution,
}

impl Thread {
    pub fn bootstrap(cpu_index: usize) -> Self {
        let name = match ThreadName::new("bootstrap") {
            Ok(name) => name,
            Err(_) => ThreadName::empty(),
        };
        Self {
            id: ThreadId::BOOTSTRAP,
            cpu_index,
            name,
            priority: ThreadPriority::NORMAL,
            state: ThreadState::Running,
            queue_links: QueueLinks::EMPTY,
            context: crate::arch::ThreadContext::empty(),
            kernel_stack: None,
            execution: ThreadExecution::Kernel,
        }
    }

    pub fn kernel(
        id: ThreadId,
        cpu_index: usize,
        name: &str,
        entry: KernelThreadEntry,
        argument: usize,
    ) -> Result<Self, Error> {
        let stack = KernelStack::allocate(DEFAULT_KERNEL_STACK_SIZE)?;
        let mut context = crate::arch::ThreadContext::empty();
        context.prepare(stack.top(), entry, argument);
        Ok(Self {
            id,
            cpu_index,
            name: ThreadName::new(name)?,
            priority: ThreadPriority::NORMAL,
            state: ThreadState::Dormant,
            queue_links: QueueLinks::EMPTY,
            context,
            kernel_stack: Some(stack),
            execution: ThreadExecution::Kernel,
        })
    }

    /// Creates the already-running bootstrap context for a secondary CPU.
    pub fn secondary_bootstrap(id: ThreadId, cpu_index: usize, name: &str) -> Result<Self, Error> {
        Ok(Self {
            id,
            cpu_index,
            name: ThreadName::new(name)?,
            priority: ThreadPriority::NORMAL,
            state: ThreadState::Running,
            queue_links: QueueLinks::EMPTY,
            context: crate::arch::ThreadContext::empty(),
            kernel_stack: Some(KernelStack::allocate(DEFAULT_KERNEL_STACK_SIZE)?),
            execution: ThreadExecution::Kernel,
        })
    }

    pub fn user(
        id: ThreadId,
        cpu_index: usize,
        name: &str,
        address_space: AddressSpaceId,
        context: crate::arch::UserContext,
    ) -> Result<Self, Error> {
        Self::with_payload(
            id,
            cpu_index,
            name,
            ThreadExecution::User(try_box(UserExecution {
                address_space,
                context,
            })?),
        )
    }

    pub fn vcpu(
        id: ThreadId,
        cpu_index: usize,
        name: &str,
        virtual_machine: VirtualMachineId,
        vcpu_id: u32,
        mut context: crate::arch::VcpuContext,
    ) -> Result<Self, Error> {
        let _ = context.initialize_vgic()?;
        Self::with_payload(
            id,
            cpu_index,
            name,
            ThreadExecution::Vcpu(try_box(VcpuExecution {
                virtual_machine,
                vcpu_id,
                context,
            })?),
        )
    }

    fn with_payload(
        id: ThreadId,
        cpu_index: usize,
        name: &str,
        execution: ThreadExecution,
    ) -> Result<Self, Error> {
        Ok(Self {
            id,
            cpu_index,
            name: ThreadName::new(name)?,
            priority: ThreadPriority::NORMAL,
            state: ThreadState::Dormant,
            queue_links: QueueLinks::EMPTY,
            context: crate::arch::ThreadContext::empty(),
            kernel_stack: Some(KernelStack::allocate(DEFAULT_KERNEL_STACK_SIZE)?),
            execution,
        })
    }

    pub const fn id(&self) -> ThreadId {
        self.id
    }

    /// Returns the CPU that owns this thread's scheduling context.
    ///
    /// Context migration requires a stopped-thread hand-off protocol and is
    /// deliberately not implicit in the shared scheduler run queue.
    pub const fn cpu_index(&self) -> usize {
        self.cpu_index
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub const fn state(&self) -> ThreadState {
        self.state
    }

    pub const fn priority(&self) -> ThreadPriority {
        self.priority
    }

    pub(crate) fn set_priority(&mut self, priority: ThreadPriority) {
        self.priority = priority;
    }

    pub fn set_state(&mut self, state: ThreadState) {
        self.state = state;
    }

    pub(crate) const fn queue_links(&self) -> QueueLinks {
        self.queue_links
    }

    pub(crate) fn set_queue_links(&mut self, links: QueueLinks) {
        self.queue_links = links;
    }

    pub fn context(&self) -> &crate::arch::ThreadContext {
        &self.context
    }

    pub fn context_mut(&mut self) -> &mut crate::arch::ThreadContext {
        &mut self.context
    }

    pub const fn execution_kind(&self) -> ExecutionKind {
        match self.execution {
            ThreadExecution::Kernel => ExecutionKind::Kernel,
            ThreadExecution::User(_) => ExecutionKind::User,
            ThreadExecution::Vcpu(_) => ExecutionKind::Vcpu,
        }
    }

    pub fn user_execution(&self) -> Option<&UserExecution> {
        match &self.execution {
            ThreadExecution::User(execution) => Some(execution.as_ref()),
            _ => None,
        }
    }

    pub fn vcpu_execution(&self) -> Option<&VcpuExecution> {
        match &self.execution {
            ThreadExecution::Vcpu(execution) => Some(execution.as_ref()),
            _ => None,
        }
    }

    pub fn vcpu_execution_mut(&mut self) -> Option<&mut VcpuExecution> {
        match &mut self.execution {
            ThreadExecution::Vcpu(execution) => Some(execution.as_mut()),
            _ => None,
        }
    }

    pub const fn owns_kernel_stack(&self) -> bool {
        self.kernel_stack.is_some()
    }

    pub(crate) fn kernel_stack_top(&self) -> Option<usize> {
        self.kernel_stack.as_ref().map(KernelStack::top)
    }

    pub(crate) fn kernel_stack_bounds(&self) -> Option<(usize, usize)> {
        self.kernel_stack.as_ref().map(KernelStack::bounds)
    }
}

struct ThreadName {
    bytes: [u8; THREAD_NAME_CAPACITY],
    len: u8,
}

impl ThreadName {
    fn new(name: &str) -> Result<Self, Error> {
        if name.len() > THREAD_NAME_CAPACITY {
            return Err(Error::NameTooLong);
        }
        let mut bytes = [0; THREAD_NAME_CAPACITY];
        bytes[..name.len()].copy_from_slice(name.as_bytes());
        Ok(Self {
            bytes,
            len: name.len() as u8,
        })
    }

    const fn empty() -> Self {
        Self {
            bytes: [0; THREAD_NAME_CAPACITY],
            len: 0,
        }
    }

    fn as_str(&self) -> &str {
        let bytes = &self.bytes[..usize::from(self.len)];
        // SAFETY: ThreadName is constructed exclusively from validated UTF-8
        // string slices and never exposes mutable access to its byte storage.
        unsafe { core::str::from_utf8_unchecked(bytes) }
    }
}

struct KernelStack {
    base: NonNull<u8>,
    layout: Layout,
}

impl KernelStack {
    fn allocate(size: usize) -> Result<Self, Error> {
        let layout = Layout::from_size_align(size, KERNEL_STACK_ALIGNMENT)
            .map_err(|_| Error::InvalidStackLayout)?;
        // SAFETY: The global allocator is initialized before scheduler setup;
        // the returned allocation is owned by this KernelStack on success.
        let base = NonNull::new(unsafe { alloc_zeroed(layout) }).ok_or(Error::Allocation)?;
        Ok(Self { base, layout })
    }

    fn top(&self) -> usize {
        self.base.as_ptr() as usize + self.layout.size()
    }

    fn bounds(&self) -> (usize, usize) {
        (self.base.as_ptr() as usize, self.top())
    }
}

impl Drop for KernelStack {
    fn drop(&mut self) {
        // SAFETY: base was allocated with exactly this layout and ownership is
        // unique to KernelStack until this destructor runs.
        unsafe { dealloc(self.base.as_ptr(), self.layout) };
    }
}

// SAFETY: KernelStack owns an allocation and contains no thread-affine state.
unsafe impl Send for KernelStack {}

fn try_box<T>(value: T) -> Result<Box<T>, Error> {
    let layout = Layout::new::<T>();
    // SAFETY: A successful allocation has the exact layout required by T. The
    // value is initialized before ownership transfers to Box.
    let pointer =
        NonNull::new(unsafe { alloc::alloc::alloc(layout) } as *mut T).ok_or(Error::Allocation)?;
    // SAFETY: pointer is valid, aligned, and uniquely owned for one T.
    unsafe {
        pointer.as_ptr().write(value);
        Ok(Box::from_raw(pointer.as_ptr()))
    }
}
