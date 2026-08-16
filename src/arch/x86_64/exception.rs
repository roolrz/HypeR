use core::arch::asm;
use core::cell::UnsafeCell;

use hyper::hal::interrupt::InterruptId;
use hyper::sync::atomic::{AtomicBool, Ordering};

const MAX_CPUS: usize = hyper::config::MAX_CPUS as usize;
const IDT_ENTRIES: usize = 256;
pub const NO_EXCEPTION_VECTOR: u64 = u64::MAX;

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct IdtEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    attributes: u8,
    offset_middle: u16,
    offset_high: u32,
    reserved: u32,
}

impl IdtEntry {
    const EMPTY: Self = Self {
        offset_low: 0,
        selector: 0,
        ist: 0,
        attributes: 0,
        offset_middle: 0,
        offset_high: 0,
        reserved: 0,
    };

    fn interrupt(handler: u64, ist: u8) -> Self {
        Self {
            offset_low: handler as u16,
            selector: 0x20,
            ist,
            attributes: 0x8e,
            offset_middle: (handler >> 16) as u16,
            offset_high: (handler >> 32) as u32,
            reserved: 0,
        }
    }
}

#[repr(C, packed)]
struct DescriptorTablePointer {
    limit: u16,
    base: u64,
}

#[repr(C, packed)]
struct TaskStateSegment {
    reserved_0: u32,
    privilege_stack: [u64; 3],
    reserved_1: u64,
    interrupt_stack: [u64; 7],
    reserved_2: u64,
    reserved_3: u16,
    io_bitmap_offset: u16,
}

impl TaskStateSegment {
    const EMPTY: Self = Self {
        reserved_0: 0,
        privilege_stack: [0; 3],
        reserved_1: 0,
        interrupt_stack: [0; 7],
        reserved_2: 0,
        reserved_3: 0,
        io_bitmap_offset: core::mem::size_of::<Self>() as u16,
    };
}

struct DescriptorStorage(UnsafeCell<[[u64; 7]; MAX_CPUS]>);
unsafe impl Sync for DescriptorStorage {}
struct TaskStateStorage(UnsafeCell<[TaskStateSegment; MAX_CPUS]>);
unsafe impl Sync for TaskStateStorage {}

static GDTS: DescriptorStorage = DescriptorStorage(UnsafeCell::new([[0; 7]; MAX_CPUS]));
static TASK_STATES: TaskStateStorage = TaskStateStorage(UnsafeCell::new(
    [const { TaskStateSegment::EMPTY }; MAX_CPUS],
));

struct IdtStorage(UnsafeCell<[IdtEntry; IDT_ENTRIES]>);
unsafe impl Sync for IdtStorage {}

static IDT: IdtStorage = IdtStorage(UnsafeCell::new([IdtEntry::EMPTY; IDT_ENTRIES]));
static INSTALLED: AtomicBool = AtomicBool::new(false);

#[repr(C)]
pub(crate) struct ExceptionFrame {
    // Register order follows the push sequence as observed from the final RSP.
    general: [u64; 15],
    vector: u64,
    error: u64,
    rip: u64,
    cs: u64,
    rflags: u64,
    rsp: u64,
    ss: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C, align(16))]
pub struct CrashContext {
    pub general: [u64; 16],
    general_valid: u64,
    pub stack_pointer: u64,
    pub program_counter: u64,
    pub processor_state: u64,
    pub syndrome: u64,
    pub fault_address: u64,
    pub exception_vector: u64,
    pub hardware_id: u64,
    pub interrupt_mask: u64,
    pub cr3: u64,
}

impl CrashContext {
    pub const fn general_is_valid(&self, register: usize) -> bool {
        register < 16 && self.general_valid & (1 << register) != 0
    }

    pub const fn has_exception_frame(&self) -> bool {
        self.exception_vector != NO_EXCEPTION_VECTOR
    }
}

pub fn capture_crash_context() -> CrashContext {
    let frame_pointer: u64;
    let program_counter: u64;
    let rsp: u64;
    let rflags: u64;
    let cr2: u64;
    let cr3: u64;
    unsafe {
        asm!(
            "mov rax, rbp",
            "lea rcx, [rip + 2f]",
            "2:",
            "mov rdx, rsp",
            "pushfq",
            "pop rsi",
            "mov rdi, cr2",
            "mov r8, cr3",
            lateout("rax") frame_pointer,
            lateout("rcx") program_counter,
            lateout("rdx") rsp,
            lateout("rsi") rflags,
            lateout("rdi") cr2,
            lateout("r8") cr3,
        )
    };
    let mut general = [0; 16];
    general[4] = rsp;
    general[5] = frame_pointer;
    CrashContext {
        general,
        general_valid: (1 << 4) | (1 << 5),
        stack_pointer: rsp,
        program_counter,
        processor_state: rflags,
        syndrome: 0,
        fault_address: cr2,
        exception_vector: NO_EXCEPTION_VECTOR,
        hardware_id: super::current_hardware_id(),
        interrupt_mask: rflags,
        cr3,
    }
}

pub fn install_exception_stacks(
    cpu: usize,
    irq: (usize, usize),
    emergency: (usize, usize),
) -> Result<(), ()> {
    if cpu >= MAX_CPUS || irq.0 >= irq.1 || emergency.0 >= emergency.1 {
        return Err(());
    }
    let task_states = unsafe { &mut *TASK_STATES.0.get() };
    task_states[cpu].interrupt_stack[0] = irq.1 as u64;
    task_states[cpu].interrupt_stack[1] = emergency.1 as u64;
    Ok(())
}

unsafe extern "C" {
    static x86_64_vector_stubs: [i32; IDT_ENTRIES];
}

pub unsafe fn install_runtime_vectors() {
    let table = unsafe { &mut *IDT.0.get() };
    let base = core::ptr::addr_of!(x86_64_vector_stubs) as usize;
    for (vector, (entry, offset)) in table
        .iter_mut()
        .zip(unsafe { x86_64_vector_stubs }.iter())
        .enumerate()
    {
        let handler = base.wrapping_add_signed(*offset as isize);
        let ist = if vector < 32 { 2 } else { 1 };
        *entry = IdtEntry::interrupt(handler as u64, ist);
    }
    install_local_vectors();
    INSTALLED.store(true, Ordering::Release);
}

pub fn install_local_vectors() {
    install_local_descriptors();
    let table = unsafe { &*IDT.0.get() };
    let pointer = DescriptorTablePointer {
        limit: (core::mem::size_of_val(table) - 1) as u16,
        base: table.as_ptr() as u64,
    };
    unsafe { asm!("lidt [{}]", in(reg) &pointer, options(readonly, nostack)) };
}

fn install_local_descriptors() {
    let cpu = super::current_cpu_index();
    let Some(table) = (unsafe { &mut *GDTS.0.get() }).get_mut(cpu) else {
        super::halt();
    };
    let Some(task_state) = (unsafe { &mut *TASK_STATES.0.get() }).get_mut(cpu) else {
        super::halt();
    };
    let base = core::ptr::from_mut(task_state).addr() as u64;
    let limit = (core::mem::size_of::<TaskStateSegment>() - 1) as u64;
    table[2] = 0x00cf_9a00_0000_ffff;
    table[3] = 0x00cf_9200_0000_ffff;
    table[4] = 0x00af_9a00_0000_ffff;
    table[5] = limit | ((base & 0x00ff_ffff) << 16) | (0x89 << 40) | (((base >> 24) & 0xff) << 56);
    table[6] = base >> 32;
    let pointer = DescriptorTablePointer {
        limit: (core::mem::size_of_val(table) - 1) as u16,
        base: table.as_ptr() as u64,
    };
    unsafe {
        asm!(
            "lgdt [{}]",
            "mov ax, 0x28",
            "ltr ax",
            in(reg) &pointer,
            out("ax") _,
            options(readonly, nostack),
        )
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationError {
    NotInstalled,
}

pub fn validate_runtime_vectors() -> Result<(), ValidationError> {
    INSTALLED
        .load(Ordering::Acquire)
        .then_some(())
        .ok_or(ValidationError::NotInstalled)
}

unsafe extern "C" {
    fn x86_64_run_on_stack(top: usize, callback: extern "C" fn(usize) -> !, argument: usize) -> !;
}

pub unsafe fn run_on_emergency_stack(callback: extern "C" fn(usize) -> !, argument: usize) -> ! {
    let cpu = super::current_cpu_index();
    let top = unsafe { (*TASK_STATES.0.get()).get(cpu) }
        .map(|state| state.interrupt_stack[1] as usize)
        .unwrap_or(0);
    if top == 0 {
        callback(argument)
    }
    unsafe { x86_64_run_on_stack(top, callback, argument) }
}

pub fn bootstrap_stack_bounds(stack_pointer: u64) -> Option<(usize, usize)> {
    unsafe extern "C" {
        static __boot_stack_bottom: u8;
        static __boot_stack_top: u8;
    }
    let bottom = core::ptr::addr_of!(__boot_stack_bottom) as usize;
    let top = core::ptr::addr_of!(__boot_stack_top) as usize;
    (stack_pointer >= bottom as u64 && stack_pointer < top as u64).then_some((bottom, top))
}

#[unsafe(no_mangle)]
extern "C" fn x86_64_vector_dispatch(frame: &mut ExceptionFrame) {
    let vector = frame.vector as u32;
    if vector >= 32 {
        crate::kernel::irq::interrupt::dispatch(InterruptId::new(vector));
        return;
    }
    let mut context = capture_crash_context();
    context.program_counter = frame.rip;
    context.stack_pointer = frame.rsp;
    context.processor_state = frame.rflags;
    context.syndrome = frame.error;
    context.exception_vector = frame.vector;
    context.general = [
        frame.general[14],
        frame.general[13],
        frame.general[12],
        frame.general[11],
        frame.rsp,
        frame.general[10],
        frame.general[9],
        frame.general[8],
        frame.general[7],
        frame.general[6],
        frame.general[5],
        frame.general[4],
        frame.general[3],
        frame.general[2],
        frame.general[1],
        frame.general[0],
    ];
    context.general_valid = u16::MAX as u64;
    crate::kernel::irq::exception::fatal_architecture(
        context,
        format_args!(
            "fatal x86 exception: vector {}, error {:#x}, RIP {:#x}, CR2 {:#x}, RFLAGS {:#x}",
            frame.vector, frame.error, frame.rip, context.fault_address, frame.rflags
        ),
    )
}
