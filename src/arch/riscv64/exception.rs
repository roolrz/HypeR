use core::arch::asm;
use core::cell::UnsafeCell;

use hyper::hal::interrupt::InterruptId;
use hyper::sync::atomic::{AtomicU64, Ordering};

const MAX_CPUS: usize = hyper::config::MAX_CPUS as usize;
pub const NO_EXCEPTION_VECTOR: u64 = u64::MAX;

#[derive(Clone, Copy)]
struct StackBounds {
    bottom: usize,
    top: usize,
}
impl StackBounds {
    const EMPTY: Self = Self { bottom: 0, top: 0 };
}

struct StackTable(UnsafeCell<[StackBounds; MAX_CPUS]>);
unsafe impl Sync for StackTable {}

static IRQ_STACKS: StackTable = StackTable(UnsafeCell::new([StackBounds::EMPTY; MAX_CPUS]));
static EMERGENCY_STACKS: StackTable = StackTable(UnsafeCell::new([StackBounds::EMPTY; MAX_CPUS]));
static VECTOR_INSTALLED: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C, align(16))]
pub struct CrashContext {
    pub general: [u64; 32],
    general_valid: u64,
    pub stack_pointer: u64,
    pub program_counter: u64,
    pub processor_state: u64,
    pub syndrome: u64,
    pub fault_address: u64,
    pub exception_vector: u64,
    pub hardware_id: u64,
    pub interrupt_mask: u64,
    pub satp: u64,
    pub stvec: u64,
    pub hstatus: u64,
    pub hgatp: u64,
}

impl CrashContext {
    fn empty() -> Self {
        Self {
            general: [0; 32],
            general_valid: 0,
            stack_pointer: 0,
            program_counter: 0,
            processor_state: 0,
            syndrome: 0,
            fault_address: 0,
            exception_vector: NO_EXCEPTION_VECTOR,
            hardware_id: super::current_hardware_id(),
            interrupt_mask: 0,
            satp: 0,
            stvec: 0,
            hstatus: 0,
            hgatp: 0,
        }
    }

    pub const fn general_is_valid(&self, register: usize) -> bool {
        register < 32 && self.general_valid & (1 << register) != 0
    }
    pub const fn has_exception_frame(&self) -> bool {
        self.exception_vector != NO_EXCEPTION_VECTOR
    }
}

pub fn capture_crash_context() -> CrashContext {
    let mut context = CrashContext::empty();
    let sp: u64;
    let sepc: u64;
    let sstatus: u64;
    let scause: u64;
    let stval: u64;
    let satp: u64;
    let stvec: u64;
    let hstatus: u64;
    let hgatp: u64;
    unsafe {
        asm!(
            "mv {sp}, sp", "csrr {sepc}, sepc", "csrr {sstatus}, sstatus",
            "csrr {scause}, scause", "csrr {stval}, stval", "csrr {satp}, satp",
            "csrr {stvec}, stvec", "csrr {hstatus}, hstatus", "csrr {hgatp}, hgatp",
            sp = out(reg) sp, sepc = out(reg) sepc, sstatus = out(reg) sstatus,
            scause = out(reg) scause, stval = out(reg) stval, satp = out(reg) satp,
            stvec = out(reg) stvec, hstatus = out(reg) hstatus, hgatp = out(reg) hgatp,
            options(nomem, nostack)
        );
    }
    context.stack_pointer = sp;
    context.program_counter = sepc;
    context.processor_state = sstatus;
    context.syndrome = scause;
    context.fault_address = stval;
    context.interrupt_mask = sstatus;
    context.satp = satp;
    context.stvec = stvec;
    context.hstatus = hstatus;
    context.hgatp = hgatp;
    context
}

pub fn install_exception_stacks(
    cpu: usize,
    irq: (usize, usize),
    emergency: (usize, usize),
) -> Result<(), ()> {
    if cpu >= MAX_CPUS || irq.0 >= irq.1 || emergency.0 >= emergency.1 {
        return Err(());
    }
    unsafe {
        (*IRQ_STACKS.0.get())[cpu] = StackBounds {
            bottom: irq.0,
            top: irq.1,
        };
        (*EMERGENCY_STACKS.0.get())[cpu] = StackBounds {
            bottom: emergency.0,
            top: emergency.1,
        };
    }
    Ok(())
}

unsafe extern "C" {
    static riscv64_trap_vector: u8;
    fn riscv64_call_trap_on_stack(
        frame: *mut TrapFrame,
        stack_top: usize,
        callback: extern "C" fn(&mut TrapFrame),
    );
}

pub unsafe fn install_runtime_vectors() {
    let address = core::ptr::addr_of!(riscv64_trap_vector) as usize;
    unsafe { asm!("csrw stvec, {address}", address = in(reg) address, options(nostack)) };
    VECTOR_INSTALLED.store(address as u64, Ordering::Release);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationError {
    NotInstalled,
}

pub fn validate_runtime_vectors() -> Result<(), ValidationError> {
    (VECTOR_INSTALLED.load(Ordering::Acquire) != 0)
        .then_some(())
        .ok_or(ValidationError::NotInstalled)
}

pub unsafe fn run_on_emergency_stack(callback: extern "C" fn(usize) -> !, argument: usize) -> ! {
    let cpu = super::current_cpu_index();
    let bounds = unsafe { (*EMERGENCY_STACKS.0.get()).get(cpu).copied() };
    match bounds {
        Some(bounds) if bounds.bottom < bounds.top => unsafe {
            super::context::run_on_stack(bounds.top, callback, argument)
        },
        _ => callback(argument),
    }
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

#[repr(C, align(16))]
pub(crate) struct TrapFrame {
    pub(crate) general: [u64; 32],
    pub(crate) sepc: u64,
    pub(crate) sstatus: u64,
    pub(crate) scause: u64,
    pub(crate) stval: u64,
    pub(crate) htval: u64,
    pub(crate) htinst: u64,
    guest_origin: u64,
    host_cpu_index: u64,
}

#[unsafe(no_mangle)]
extern "C" fn riscv64_trap_dispatch(frame: &mut TrapFrame) {
    const INTERRUPT: u64 = 1 << 63;
    if frame.scause & INTERRUPT != 0 {
        let cpu = super::current_cpu_index();
        let bounds = unsafe { (*IRQ_STACKS.0.get()).get(cpu).copied() };
        let Some(bounds) = bounds.filter(|bounds| bounds.bottom < bounds.top) else {
            fatal_trap(frame);
        };
        let current_stack = core::ptr::from_ref(frame).addr();
        if current_stack < bounds.bottom || current_stack >= bounds.top {
            // SAFETY: Per-CPU IRQ-stack installation validates these bounds.
            // Hardware masks S-mode interrupts on trap entry, so this CPU is
            // the exclusive owner until the callback returns.
            unsafe {
                riscv64_call_trap_on_stack(core::ptr::from_mut(frame), bounds.top, dispatch_trap)
            };
            return;
        }
    }
    dispatch_trap(frame);
}

extern "C" fn dispatch_trap(frame: &mut TrapFrame) {
    const INTERRUPT: u64 = 1 << 63;
    const SUPERVISOR_TIMER: u64 = 5;
    const SUPERVISOR_EXTERNAL: u64 = 9;
    const SUPERVISOR_SOFTWARE: u64 = 1;
    if frame.scause & INTERRUPT != 0 {
        match frame.scause & !INTERRUPT {
            SUPERVISOR_SOFTWARE => super::interrupts::clear_software_interrupt(),
            SUPERVISOR_TIMER => crate::kernel::irq::interrupt::dispatch(InterruptId::new(0)),
            SUPERVISOR_EXTERNAL => {
                if let Some(interrupt) = crate::kernel::irq::interrupt::acknowledge_external() {
                    crate::kernel::irq::interrupt::dispatch(interrupt);
                }
            }
            _ => fatal_trap(frame),
        }
        return;
    }
    if frame.guest_origin != 0 {
        let mut guest_frame = super::guest::GuestSyncFrame::new(frame);
        if crate::kernel::vm::handle_guest_sync(&mut guest_frame) {
            return;
        }
    }
    fatal_trap(frame)
}

fn fatal_trap(frame: &TrapFrame) -> ! {
    let mut context = capture_crash_context();
    context.general = frame.general;
    context.general_valid = u64::MAX;
    context.stack_pointer = frame.general[2];
    context.program_counter = frame.sepc;
    context.processor_state = frame.sstatus;
    context.syndrome = frame.scause;
    context.fault_address = frame.stval;
    context.exception_vector = frame.scause;
    crate::kernel::irq::exception::fatal_invalid_vector(
        frame.scause,
        frame.scause,
        frame.sepc,
        frame.stval,
        frame.sstatus,
        context,
    )
}
