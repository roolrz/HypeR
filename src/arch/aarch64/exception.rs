use core::arch::asm;
use core::mem::{offset_of, size_of};
use core::ptr::addr_of;

use hyper::sync::atomic::{AtomicU64, Ordering};

use super::registers;

static VECTOR_TEST_EXPECTED: AtomicU64 = AtomicU64::new(registers::EXCEPTION_VECTOR_TEST_NONE);

const MAX_CPUS: usize = hyper::config::MAX_CPUS as usize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExceptionOrigin {
    CurrentSp0,
    CurrentSpx,
    LowerAarch64,
    LowerAarch32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExceptionKind {
    Synchronous,
    Irq,
    Fiq,
    SystemError,
}

#[repr(C)]
struct StackBounds {
    // Assembly loads `top` with acquire semantics before reading `bottom`.
    // The layout assertions below keep that publication protocol explicit.
    bottom: AtomicU64,
    top: AtomicU64,
}

impl StackBounds {
    const fn empty() -> Self {
        Self {
            bottom: AtomicU64::new(0),
            top: AtomicU64::new(0),
        }
    }
}

#[repr(C, align(64))]
struct StackTable {
    entries: [StackBounds; MAX_CPUS],
}

impl StackTable {
    const fn new() -> Self {
        Self {
            entries: [const { StackBounds::empty() }; MAX_CPUS],
        }
    }
}

#[unsafe(no_mangle)]
static AARCH64_IRQ_STACK_BOUNDS: StackTable = StackTable::new();

#[unsafe(no_mangle)]
static AARCH64_EMERGENCY_STACK_BOUNDS: StackTable = StackTable::new();

#[repr(C, align(16))]
struct ExceptionFrame {
    general: [u64; 31],
    elr: u64,
    spsr: u64,
    esr: u64,
    far: u64,
    vector: u64,
    sp_el0: u64,
    sp_el1: u64,
    simd: [[u64; 2]; 32],
    fpcr: u64,
    fpsr: u64,
}

const _: () = {
    assert!(size_of::<StackBounds>() == 16);
    assert!(core::mem::align_of::<StackBounds>() == 8);
    assert!(offset_of!(StackBounds, bottom) == 0);
    assert!(offset_of!(StackBounds, top) == 8);
    assert!(offset_of!(StackTable, entries) == 0);
    assert!(offset_of!(ExceptionFrame, general) == registers::EXCEPTION_FRAME_X0_OFFSET as usize);
    assert!(offset_of!(ExceptionFrame, elr) == registers::EXCEPTION_FRAME_ELR_OFFSET as usize);
    assert!(offset_of!(ExceptionFrame, spsr) == registers::EXCEPTION_FRAME_SPSR_OFFSET as usize);
    assert!(offset_of!(ExceptionFrame, esr) == registers::EXCEPTION_FRAME_ESR_OFFSET as usize);
    assert!(offset_of!(ExceptionFrame, far) == registers::EXCEPTION_FRAME_FAR_OFFSET as usize);
    assert!(
        offset_of!(ExceptionFrame, vector) == registers::EXCEPTION_FRAME_VECTOR_OFFSET as usize
    );
    assert!(
        offset_of!(ExceptionFrame, sp_el0) == registers::EXCEPTION_FRAME_SP_EL0_OFFSET as usize
    );
    assert!(
        offset_of!(ExceptionFrame, sp_el1) == registers::EXCEPTION_FRAME_SP_EL1_OFFSET as usize
    );
    assert!(offset_of!(ExceptionFrame, simd) == registers::EXCEPTION_FRAME_SIMD_OFFSET as usize);
    assert!(offset_of!(ExceptionFrame, fpcr) == registers::EXCEPTION_FRAME_FPCR_OFFSET as usize);
    assert!(offset_of!(ExceptionFrame, fpsr) == registers::EXCEPTION_FRAME_FPSR_OFFSET as usize);
    assert!(size_of::<ExceptionFrame>() == registers::EXCEPTION_FRAME_SIZE as usize);
};

unsafe extern "C" {
    static aarch64_runtime_vectors: u8;
    fn aarch64_capture_crash_context(context: *mut CrashContext);
}

pub const NO_EXCEPTION_VECTOR: u64 = u64::MAX;

/// Register and control-state snapshot consumed by architecture-neutral crash
/// policy. Exception entry provides every general register; a software panic
/// cannot preserve x0 because the capture ABI uses it for the output pointer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C, align(16))]
pub struct CrashContext {
    pub general: [u64; 31],
    general_valid: u64,
    pub stack_pointer: u64,
    pub program_counter: u64,
    pub processor_state: u64,
    pub syndrome: u64,
    pub fault_address: u64,
    pub exception_vector: u64,
    pub hardware_id: u64,
    pub current_el: u64,
    pub interrupt_mask: u64,
    pub sctlr_el2: u64,
    pub tcr_el2: u64,
    pub ttbr0_el2: u64,
    pub vbar_el2: u64,
    pub hcr_el2: u64,
}

impl CrashContext {
    pub const FRAME_POINTER_REGISTER: usize = 29;
    pub const GENERAL_REGISTER_COUNT: usize = 31;

    const fn empty() -> Self {
        Self {
            general: [0; 31],
            general_valid: 0,
            stack_pointer: 0,
            program_counter: 0,
            processor_state: 0,
            syndrome: 0,
            fault_address: 0,
            exception_vector: NO_EXCEPTION_VECTOR,
            hardware_id: 0,
            current_el: 0,
            interrupt_mask: 0,
            sctlr_el2: 0,
            tcr_el2: 0,
            ttbr0_el2: 0,
            vbar_el2: 0,
            hcr_el2: 0,
        }
    }

    pub const fn general_is_valid(&self, register: usize) -> bool {
        register < 31 && self.general_valid & (1 << register) != 0
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
            "CPU {cpu}: {role}, MPIDR {:#x}, CurrentEL {:#x}, DAIF {:#x}",
            self.hardware_id, self.current_el, self.interrupt_mask
        ));
    }

    pub fn describe_architecture_registers(&self, mut emit: impl FnMut(core::fmt::Arguments<'_>)) {
        if self.has_exception_frame() {
            emit(format_args!(
                "ESR: {:#018x}  FAR: {:#018x}  vector: {:#x}",
                self.syndrome, self.fault_address, self.exception_vector
            ));
        } else {
            emit(format_args!(
                "origin: software panic (no architectural exception frame)"
            ));
        }
        emit(format_args!(
            "SCTLR_EL2: {:#018x}  TCR_EL2: {:#018x}  TTBR0_EL2: {:#018x}",
            self.sctlr_el2, self.tcr_el2, self.ttbr0_el2
        ));
        emit(format_args!(
            "VBAR_EL2: {:#018x}  HCR_EL2: {:#018x}",
            self.vbar_el2, self.hcr_el2
        ));
    }

    /// Reads one frame record from a pinned kernel stack.
    ///
    /// # Safety
    ///
    /// `bottom..top` must remain a readable, initialized kernel-stack mapping
    /// for the duration of this call. `frame`, when accepted by the numerical
    /// checks below, must carry valid provenance for two initialized `usize`
    /// values in that mapping.
    pub unsafe fn previous_stack_frame(
        frame: usize,
        bottom: usize,
        top: usize,
    ) -> Result<Option<(usize, usize)>, ()> {
        if frame & 0xf != 0 || frame < bottom || frame.checked_add(16).is_none_or(|end| end > top) {
            return Err(());
        }
        // SAFETY: The caller establishes the mapping, lifetime, provenance,
        // and initialization that cannot be inferred from integer bounds.
        let pointer = core::ptr::with_exposed_provenance::<usize>(frame);
        // SAFETY: The checked address covers the first initialized frame word
        // in the caller-provided pinned stack mapping.
        let previous = unsafe { core::ptr::read_volatile(pointer) };
        // SAFETY: The checked 16-byte record covers this second initialized
        // word in the same pinned mapping.
        let link = unsafe { core::ptr::read_volatile(pointer.add(1)) };
        Ok((link != 0).then_some((previous, link)))
    }
}

const _: () = {
    assert!(offset_of!(CrashContext, general) == registers::CRASH_CONTEXT_X0_OFFSET as usize);
    assert!(
        offset_of!(CrashContext, general_valid) == registers::CRASH_CONTEXT_VALID_OFFSET as usize
    );
    assert!(offset_of!(CrashContext, stack_pointer) == registers::CRASH_CONTEXT_SP_OFFSET as usize);
    assert!(
        offset_of!(CrashContext, program_counter) == registers::CRASH_CONTEXT_PC_OFFSET as usize
    );
    assert!(
        offset_of!(CrashContext, processor_state)
            == registers::CRASH_CONTEXT_PSTATE_OFFSET as usize
    );
    assert!(offset_of!(CrashContext, syndrome) == registers::CRASH_CONTEXT_ESR_OFFSET as usize);
    assert!(
        offset_of!(CrashContext, fault_address) == registers::CRASH_CONTEXT_FAR_OFFSET as usize
    );
    assert!(
        offset_of!(CrashContext, exception_vector)
            == registers::CRASH_CONTEXT_VECTOR_OFFSET as usize
    );
    assert!(
        offset_of!(CrashContext, hardware_id) == registers::CRASH_CONTEXT_MPIDR_OFFSET as usize
    );
    assert!(
        offset_of!(CrashContext, current_el) == registers::CRASH_CONTEXT_CURRENT_EL_OFFSET as usize
    );
    assert!(
        offset_of!(CrashContext, interrupt_mask) == registers::CRASH_CONTEXT_DAIF_OFFSET as usize
    );
    assert!(
        offset_of!(CrashContext, sctlr_el2) == registers::CRASH_CONTEXT_SCTLR_EL2_OFFSET as usize
    );
    assert!(offset_of!(CrashContext, tcr_el2) == registers::CRASH_CONTEXT_TCR_EL2_OFFSET as usize);
    assert!(
        offset_of!(CrashContext, ttbr0_el2) == registers::CRASH_CONTEXT_TTBR0_EL2_OFFSET as usize
    );
    assert!(
        offset_of!(CrashContext, vbar_el2) == registers::CRASH_CONTEXT_VBAR_EL2_OFFSET as usize
    );
    assert!(offset_of!(CrashContext, hcr_el2) == registers::CRASH_CONTEXT_HCR_EL2_OFFSET as usize);
    assert!(size_of::<CrashContext>() == registers::CRASH_CONTEXT_SIZE as usize);
};

/// Captures the calling kernel context for a software panic.
#[unsafe(export_name = "aarch64_capture_crash_context_rust")]
pub fn capture_crash_context() -> CrashContext {
    let mut context = CrashContext::empty();
    // SAFETY: The assembly routine writes exactly one aligned CrashContext and
    // preserves all AAPCS64 callee-saved registers.
    unsafe { aarch64_capture_crash_context(&mut context) };
    context.general_valid = ((1u64 << 31) - 1) & !1;
    context
}

fn exception_crash_context(frame: &ExceptionFrame, stack_pointer: u64) -> CrashContext {
    let mut context = capture_crash_context();
    context.general = frame.general;
    context.general_valid = (1u64 << 31) - 1;
    context.stack_pointer = stack_pointer;
    context.program_counter = frame.elr;
    context.processor_state = frame.spsr;
    context.syndrome = frame.esr;
    context.fault_address = frame.far;
    context.exception_vector = frame.vector;
    context
}

pub fn runtime_vector_address() -> u64 {
    addr_of!(aarch64_runtime_vectors) as u64
}

/// Publishes the exception-stack bounds consumed directly by vector assembly.
///
/// # Safety
///
/// The target CPU must not be able to enter the runtime vectors or emergency
/// path until this call completes. Each entry may be installed only once, and
/// both mappings must remain pinned and writable for that CPU's lifetime.
pub unsafe fn install_exception_stacks(
    cpu: usize,
    irq: (usize, usize),
    emergency: (usize, usize),
) -> Result<(), ()> {
    if cpu >= MAX_CPUS
        || irq.0 >= irq.1
        || emergency.0 >= emergency.1
        || irq.0 & registers::STACK_ALIGNMENT_MASK as usize != 0
        || irq.1 & registers::STACK_ALIGNMENT_MASK as usize != 0
        || emergency.0 & registers::STACK_ALIGNMENT_MASK as usize != 0
        || emergency.1 & registers::STACK_ALIGNMENT_MASK as usize != 0
    {
        return Err(());
    }
    let irq_entry = &AARCH64_IRQ_STACK_BOUNDS.entries[cpu];
    irq_entry.bottom.store(irq.0 as u64, Ordering::Relaxed);
    irq_entry.top.store(irq.1 as u64, Ordering::Release);
    let emergency_entry = &AARCH64_EMERGENCY_STACK_BOUNDS.entries[cpu];
    emergency_entry
        .bottom
        .store(emergency.0 as u64, Ordering::Relaxed);
    emergency_entry
        .top
        .store(emergency.1 as u64, Ordering::Release);
    Ok(())
}

/// Installs the complete runtime EL2 exception vector table.
///
/// # Safety
///
/// The high kernel mapping and stack must be active, and kernel exception
/// services must be ready before local exceptions are unmasked.
pub unsafe fn install_runtime_vectors() {
    let address = addr_of!(aarch64_runtime_vectors) as usize;
    // SAFETY: The assembly symbol is 2 KiB aligned and permanently mapped RX.
    unsafe {
        asm!(
            "msr VBAR_EL2, {address}",
            "isb",
            address = in(reg) address,
            options(nostack, preserves_flags)
        );
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationError {
    ExceptionDidNotReturn,
}

pub fn validate_runtime_vectors() -> Result<(), ValidationError> {
    VECTOR_TEST_EXPECTED.store(
        registers::EXCEPTION_VECTOR_CURRENT_SPX_SYNC,
        Ordering::Release,
    );
    // SAFETY: The dispatcher recognizes this private BRK immediate, advances
    // ELR past the instruction, and restores the complete interrupted state.
    unsafe {
        asm!(
            "brk #{test}",
            test = const registers::EXCEPTION_VECTOR_TEST_IMMEDIATE,
            options(nostack)
        )
    };
    if VECTOR_TEST_EXPECTED.load(Ordering::Acquire) != registers::EXCEPTION_VECTOR_TEST_NONE {
        return Err(ValidationError::ExceptionDidNotReturn);
    }

    VECTOR_TEST_EXPECTED.store(
        registers::EXCEPTION_VECTOR_CURRENT_SP0_SYNC,
        Ordering::Release,
    );
    let _saved_sp_el0: u64;
    // SAFETY: No stack access occurs while SPSel selects the deliberately
    // unusable SP_EL0 value. Slot zero must select SP_EL2 before constructing
    // its frame. ERET restores EL2t, after which SPSel is immediately reset.
    unsafe {
        asm!(
            "mrs {saved}, sp_el0",
            "mov x9, #{invalid_sp}",
            "msr sp_el0, x9",
            "msr spsel, #0",
            "brk #{test}",
            "msr spsel, #1",
            "msr sp_el0, {saved}",
            saved = lateout(reg) _saved_sp_el0,
            invalid_sp = const registers::EXCEPTION_VECTOR_TEST_INVALID_SP,
            test = const registers::EXCEPTION_VECTOR_TEST_IMMEDIATE,
            out("x9") _,
            options(nostack)
        );
    }
    if VECTOR_TEST_EXPECTED.load(Ordering::Acquire) != registers::EXCEPTION_VECTOR_TEST_NONE {
        return Err(ValidationError::ExceptionDidNotReturn);
    }
    Ok(())
}

#[unsafe(no_mangle)]
extern "C" fn aarch64_exception_dispatch(frame: &mut ExceptionFrame) {
    let exception_class = (frame.esr >> registers::ESR_EC_SHIFT) & registers::ESR_EC_MASK;
    if matches!(
        frame.vector,
        registers::EXCEPTION_VECTOR_CURRENT_SP0_SYNC | registers::EXCEPTION_VECTOR_CURRENT_SPX_SYNC
    ) && exception_class == registers::ESR_EC_BRK64
        && frame.esr & registers::ESR_BRK_COMMENT_MASK == registers::EXCEPTION_VECTOR_TEST_IMMEDIATE
        && VECTOR_TEST_EXPECTED
            .compare_exchange(
                frame.vector,
                registers::EXCEPTION_VECTOR_TEST_NONE,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    {
        frame.elr = frame.elr.wrapping_add(4);
        return;
    }

    let Some((kind, origin)) = decode_vector(frame.vector) else {
        let context = exception_crash_context(frame, frame.sp_el1);
        crate::kernel::entry::exception::fatal(
            context,
            format_args!(
                "invalid AArch64 vector {}, ESR {:#x}, ELR {:#x}, FAR {:#x}, SPSR {:#x}",
                frame.vector, frame.esr, frame.elr, frame.far, frame.spsr
            ),
        );
    };
    if kind == ExceptionKind::Irq {
        let Some(interrupt) = super::acknowledge_interrupt() else {
            return;
        };
        match crate::kernel::entry::irq::dispatch(interrupt) {
            crate::kernel::entry::irq::Action::Resume => {}
            crate::kernel::entry::irq::Action::Stop => {
                super::end_interrupt(interrupt);
                let stack_pointer = interrupted_stack_pointer(frame, origin);
                let context = exception_crash_context(frame, stack_pointer);
                crate::kernel::entry::irq::stop(context)
            }
        }
        return;
    }
    if kind == ExceptionKind::Synchronous && origin == ExceptionOrigin::LowerAarch64 {
        let physical_address = guest_physical_address(frame.far);
        let memory_fault = super::decode_guest_memory_fault(frame.esr, physical_address);
        if let Some(fault) = memory_fault {
            match crate::kernel::entry::vmexit::dispatch_memory_fault(fault) {
                crate::kernel::entry::vmexit::MemoryFaultAction::Retry => return,
                crate::kernel::entry::vmexit::MemoryFaultAction::Forward => {}
                crate::kernel::entry::vmexit::MemoryFaultAction::Stop => {
                    let stack_pointer = interrupted_stack_pointer(frame, origin);
                    let context = exception_crash_context(frame, stack_pointer);
                    crate::kernel::entry::exception::fatal(
                        context,
                        format_args!(
                            "failed to dispatch AArch64 guest memory fault at IPA {physical_address:#x}"
                        ),
                    )
                }
            }
        }
        let mut guest_frame = super::GuestSyncFrame::new(
            &mut frame.general,
            &mut frame.elr,
            &mut frame.spsr,
            frame.esr,
            physical_address,
        );
        let handled = if memory_fault.is_some() {
            crate::kernel::entry::vmexit::dispatch_legacy_after_memory_fault(&mut guest_frame)
        } else {
            crate::kernel::entry::vmexit::dispatch_legacy(&mut guest_frame)
        };
        if handled {
            return;
        }
    }

    let stack_pointer = interrupted_stack_pointer(frame, origin);
    let (architecture_class, description) = if kind == ExceptionKind::Fiq {
        (0, "FIQ exception")
    } else {
        (exception_class as u8, syndrome_description(exception_class))
    };
    let context = exception_crash_context(frame, stack_pointer);
    crate::kernel::entry::exception::fatal(
        context,
        format_args!(
            "fatal {kind:?} from {origin:?}: {description} (EC {architecture_class:#x}, ESR {:#x}, FAR {:#x})",
            frame.esr, frame.far
        ),
    )
}

fn guest_physical_address(fault_address: u64) -> u64 {
    let hpfar: u64;
    // SAFETY: HPFAR_EL2 is readable at EL2. Its FIPA field supplies IPA bits
    // above the page offset for a stage-2 abort.
    unsafe {
        asm!(
            "mrs {hpfar}, HPFAR_EL2",
            hpfar = out(reg) hpfar,
            options(nomem, nostack, preserves_flags)
        );
    }
    ((hpfar & registers::HPFAR_EL2_FIPA_MASK) << registers::HPFAR_EL2_FIPA_TO_IPA_SHIFT)
        | (fault_address & registers::PAGE_OFFSET_MASK_4K)
}

fn interrupted_stack_pointer(frame: &ExceptionFrame, origin: ExceptionOrigin) -> u64 {
    match origin {
        ExceptionOrigin::CurrentSpx => {
            (frame as *const ExceptionFrame as u64).wrapping_add(registers::EXCEPTION_FRAME_SIZE)
        }
        ExceptionOrigin::CurrentSp0 => frame.sp_el0,
        // AArch64 EL0t and EL1t use SP_EL0; EL1h uses SP_EL1. Other mode
        // values are invalid for an AArch64 lower-EL vector and are reported
        // with SP_EL1 as the conservative privileged-context choice.
        ExceptionOrigin::LowerAarch64
            if frame.spsr & registers::SPSR_M_MASK != registers::SPSR_EL1H =>
        {
            frame.sp_el0
        }
        ExceptionOrigin::LowerAarch32
            if matches!(
                frame.spsr & registers::SPSR_AARCH32_M_MASK,
                registers::SPSR_AARCH32_USR | registers::SPSR_AARCH32_SYS
            ) =>
        {
            frame.sp_el0
        }
        ExceptionOrigin::LowerAarch64 | ExceptionOrigin::LowerAarch32 => frame.sp_el1,
    }
}

fn decode_vector(vector: u64) -> Option<(ExceptionKind, ExceptionOrigin)> {
    let origin = match vector / 4 {
        0 => ExceptionOrigin::CurrentSp0,
        1 => ExceptionOrigin::CurrentSpx,
        2 => ExceptionOrigin::LowerAarch64,
        3 => ExceptionOrigin::LowerAarch32,
        _ => return None,
    };
    let kind = match vector % 4 {
        0 => ExceptionKind::Synchronous,
        1 => ExceptionKind::Irq,
        2 => ExceptionKind::Fiq,
        3 => ExceptionKind::SystemError,
        _ => return None,
    };
    Some((kind, origin))
}

fn syndrome_description(exception_class: u64) -> &'static str {
    match exception_class {
        registers::ESR_EC_UNKNOWN => "unknown reason",
        registers::ESR_EC_WFX => "trapped WFI or WFE",
        registers::ESR_EC_CP15_RT => "trapped MCR or MRC",
        registers::ESR_EC_CP15_RRT => "trapped MCRR or MRRC",
        registers::ESR_EC_CP14_RT => "trapped MCR or MRC access",
        registers::ESR_EC_CP14_DT => "trapped LDC or STC",
        registers::ESR_EC_FP_ASIMD => "trapped SIMD or floating-point access",
        registers::ESR_EC_CP14_RRT => "trapped MRRC access",
        registers::ESR_EC_ILLEGAL_STATE => "illegal execution state",
        registers::ESR_EC_SVC32 => "supervisor call from AArch32",
        registers::ESR_EC_HVC32 => "hypervisor call from AArch32",
        registers::ESR_EC_SMC32 => "monitor call from AArch32",
        registers::ESR_EC_SVC64 => "supervisor call from AArch64",
        registers::ESR_EC_HVC64 => "hypervisor call from AArch64",
        registers::ESR_EC_SMC64 => "monitor call from AArch64",
        registers::ESR_EC_SYSTEM_REGISTER => "trapped system register access",
        registers::ESR_EC_SVE => "trapped SVE access",
        registers::ESR_EC_PAC_FAILURE => "pointer authentication failure",
        registers::ESR_EC_INSTRUCTION_ABORT_LOWER => "instruction abort from lower EL",
        registers::ESR_EC_INSTRUCTION_ABORT_CURRENT => "instruction abort at current EL",
        registers::ESR_EC_PC_ALIGNMENT => "PC alignment fault",
        registers::ESR_EC_DATA_ABORT_LOWER => "data abort from lower EL",
        registers::ESR_EC_DATA_ABORT_CURRENT => "data abort at current EL",
        registers::ESR_EC_SP_ALIGNMENT => "SP alignment fault",
        registers::ESR_EC_FP32 => "floating-point exception from AArch32",
        registers::ESR_EC_FP64 => "floating-point exception from AArch64",
        registers::ESR_EC_SERROR => "SError interrupt",
        registers::ESR_EC_BREAKPOINT_LOWER | registers::ESR_EC_BREAKPOINT_CURRENT => {
            "hardware breakpoint"
        }
        registers::ESR_EC_SOFTWARE_STEP_LOWER | registers::ESR_EC_SOFTWARE_STEP_CURRENT => {
            "software step"
        }
        registers::ESR_EC_WATCHPOINT_LOWER | registers::ESR_EC_WATCHPOINT_CURRENT => "watchpoint",
        registers::ESR_EC_BKPT32 => "BKPT instruction",
        registers::ESR_EC_VECTOR_CATCH => "vector catch",
        registers::ESR_EC_BRK64 => "BRK instruction",
        _ => "reserved or implementation-defined exception class",
    }
}
