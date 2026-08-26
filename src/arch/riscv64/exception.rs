// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use core::arch::asm;

use hyper::hal::interrupt::InterruptId;
use hyper::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

const MAX_CPUS: usize = hyper::config::MAX_CPUS as usize;
pub const NO_EXCEPTION_VECTOR: u64 = u64::MAX;

#[derive(Clone, Copy)]
struct StackBounds {
    bottom: usize,
    top: usize,
}
struct AtomicStackBounds {
    bottom: AtomicUsize,
    top: AtomicUsize,
}

impl AtomicStackBounds {
    const fn new() -> Self {
        Self {
            bottom: AtomicUsize::new(0),
            top: AtomicUsize::new(0),
        }
    }

    fn install(&self, bounds: StackBounds) {
        self.bottom.store(bounds.bottom, Ordering::Relaxed);
        self.top.store(bounds.top, Ordering::Release);
    }

    fn load(&self) -> StackBounds {
        let top = self.top.load(Ordering::Acquire);
        let bottom = self.bottom.load(Ordering::Relaxed);
        StackBounds { bottom, top }
    }
}

static IRQ_STACKS: [AtomicStackBounds; MAX_CPUS] = [const { AtomicStackBounds::new() }; MAX_CPUS];
static EMERGENCY_STACKS: [AtomicStackBounds; MAX_CPUS] =
    [const { AtomicStackBounds::new() }; MAX_CPUS];
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
    pub const FRAME_POINTER_REGISTER: usize = 8;
    pub const GENERAL_REGISTER_COUNT: usize = 32;

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

    pub fn describe_cpu_header(
        &self,
        cpu: usize,
        role: &str,
        mut emit: impl FnMut(core::fmt::Arguments<'_>),
    ) {
        emit(format_args!(
            "CPU {cpu}: {role}, hart ID {:#x}, SSTATUS {:#x}",
            self.hardware_id, self.interrupt_mask
        ));
    }

    pub fn describe_architecture_registers(&self, mut emit: impl FnMut(core::fmt::Arguments<'_>)) {
        if self.has_exception_frame() {
            emit(format_args!(
                "SCAUSE: {:#018x}  STVAL: {:#018x}",
                self.syndrome, self.fault_address
            ));
        } else {
            emit(format_args!(
                "origin: software panic (no architectural exception frame)"
            ));
        }
        emit(format_args!(
            "SATP: {:#018x}  STVEC: {:#018x}",
            self.satp, self.stvec
        ));
        emit(format_args!(
            "HSTATUS: {:#018x}  HGATP: {:#018x}",
            self.hstatus, self.hgatp
        ));
    }

    /// Reads the frame record immediately below `frame`.
    ///
    /// # Safety
    ///
    /// The complete `[bottom, top)` range must remain mapped, readable, and
    /// pinned for this call. `frame` must refer to a live frame record in that
    /// range, and no concurrent agent may modify either word being read.
    pub unsafe fn previous_stack_frame(
        frame: usize,
        bottom: usize,
        top: usize,
    ) -> Result<Option<(usize, usize)>, ()> {
        const RECORD_SIZE: usize = 2 * core::mem::size_of::<usize>();
        if frame & 0xf != 0
            || bottom
                .checked_add(RECORD_SIZE)
                .is_none_or(|minimum| frame < minimum)
            || frame > top
        {
            return Err(());
        }
        let record = frame - RECORD_SIZE;
        // SAFETY: Both words lie inside the validated, pinned kernel stack.
        let previous = unsafe {
            core::ptr::read_volatile(core::ptr::with_exposed_provenance::<usize>(record))
        };
        // SAFETY: The second word is also inside the validated frame record.
        let link = unsafe {
            core::ptr::read_volatile(core::ptr::with_exposed_provenance::<usize>(
                record + core::mem::size_of::<usize>(),
            ))
        };
        Ok((link != 0).then_some((previous, link)))
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
    // SAFETY: These are read-only snapshots of CSRs available in HS mode.
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

/// Installs the assembly-visible exception stacks for one hart.
///
/// # Safety
///
/// Both ranges must be pinned, exclusively reserved stack mappings that remain
/// live while the hart is online. This must complete before the target hart can
/// enter runtime vectors, and each CPU slot may be installed only once.
pub unsafe fn install_exception_stacks(
    cpu: usize,
    irq: (usize, usize),
    emergency: (usize, usize),
) -> Result<(), ()> {
    if cpu >= MAX_CPUS || irq.0 >= irq.1 || emergency.0 >= emergency.1 {
        return Err(());
    }
    IRQ_STACKS[cpu].install(StackBounds {
        bottom: irq.0,
        top: irq.1,
    });
    EMERGENCY_STACKS[cpu].install(StackBounds {
        bottom: emergency.0,
        top: emergency.1,
    });
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

/// Installs the runtime trap vector on the current hart.
///
/// # Safety
///
/// The final executable kernel mapping and a valid exception stack must be
/// active, with local interrupts masked throughout installation.
pub unsafe fn install_runtime_vectors() {
    let address = core::ptr::addr_of!(riscv64_trap_vector) as usize;
    // SAFETY: The method contract guarantees a live executable vector and masked IRQs.
    unsafe { asm!("csrw stvec, {address}", address = in(reg) address, options(nostack)) };
    VECTOR_INSTALLED.store(address as u64, Ordering::Release);
}

/// Installs the already-published runtime trap vector on the current hart.
///
/// # Safety
///
/// The current hart must own an installed exception stack, execute from the
/// permanent kernel mapping, and keep local interrupts masked until the vector
/// has been validated.
pub unsafe fn install_local_runtime_vectors() {
    // SAFETY: STVEC is hart-local; the caller supplies this hart's lifetime and
    // interrupt-mask prerequisites.
    unsafe { install_runtime_vectors() }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationError {
    NotInstalled,
}

pub fn validate_runtime_vectors() -> Result<(), ValidationError> {
    let expected = VECTOR_INSTALLED.load(Ordering::Acquire);
    let installed: usize;
    // SAFETY: STVEC is a readable hart-local supervisor CSR.
    unsafe {
        asm!("csrr {installed}, stvec", installed = out(reg) installed, options(nomem, nostack))
    };
    (expected != 0 && installed == expected as usize)
        .then_some(())
        .ok_or(ValidationError::NotInstalled)
}

/// Validates the trap-vector state local to the calling hart.
pub fn validate_local_runtime_vectors() -> Result<(), ValidationError> {
    validate_runtime_vectors()
}

/// Transfers control to `callback` on the current hart's emergency stack.
///
/// # Safety
///
/// The installed emergency stack must not already be active, and callback must
/// not return or retain references into the abandoned stack.
pub unsafe fn run_on_emergency_stack(callback: extern "C" fn(usize) -> !, argument: usize) -> ! {
    let cpu = super::current_cpu_index();
    let bounds = EMERGENCY_STACKS.get(cpu).map(AtomicStackBounds::load);
    match bounds {
        // SAFETY: The function contract and installed bounds guarantee an unused live stack.
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
        let bounds = IRQ_STACKS.get(cpu).map(AtomicStackBounds::load);
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
            SUPERVISOR_SOFTWARE => {
                super::interrupts::clear_software_interrupt();
                crate::arch::irq::service_kernel_rpc();
            }
            SUPERVISOR_TIMER => {
                dispatch_irq_action(
                    frame,
                    crate::kernel::entry::irq::dispatch(InterruptId::new(0)),
                );
            }
            SUPERVISOR_EXTERNAL => {
                if let Some(action) = crate::kernel::entry::irq::claim_and_dispatch_external() {
                    dispatch_irq_action(frame, action);
                }
            }
            _ => fatal_trap(frame),
        }
        return;
    }
    if frame.guest_origin != 0 {
        let mut guest_frame = super::guest::GuestSyncFrame::new(frame);
        if crate::kernel::entry::vmexit::dispatch_legacy(&mut guest_frame) {
            return;
        }
    }
    fatal_trap(frame)
}

fn fatal_trap(frame: &TrapFrame) -> ! {
    let context = trap_crash_context(frame);
    crate::kernel::entry::exception::fatal(
        context,
        format_args!(
            "fatal RISC-V trap: scause {:#x}, sepc {:#x}, stval {:#x}, sstatus {:#x}",
            frame.scause, frame.sepc, frame.stval, frame.sstatus
        ),
    )
}

fn dispatch_irq_action(frame: &TrapFrame, action: crate::kernel::entry::irq::Action) {
    match action {
        crate::kernel::entry::irq::Action::Resume { postlude } => {
            // This architecture retains the request for a cooperative point
            // until it provides a qualified IRQ-tail continuation.
            let _ = postlude;
        }
        crate::kernel::entry::irq::Action::Stop => {
            crate::kernel::entry::irq::stop(trap_crash_context(frame))
        }
    }
}

fn trap_crash_context(frame: &TrapFrame) -> CrashContext {
    let mut context = capture_crash_context();
    context.general = frame.general;
    context.general_valid = u64::MAX;
    context.stack_pointer = frame.general[2];
    context.program_counter = frame.sepc;
    context.processor_state = frame.sstatus;
    context.syndrome = frame.scause;
    context.fault_address = frame.stval;
    context.exception_vector = frame.scause;
    context
}
