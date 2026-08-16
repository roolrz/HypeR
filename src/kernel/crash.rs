//! Architecture-neutral fatal-crash coordination and diagnostics.

use core::cell::UnsafeCell;
use core::fmt;
use core::hint::spin_loop;
use core::mem::MaybeUninit;
use core::panic::PanicInfo;
use core::ptr::read_volatile;

#[cfg(target_arch = "aarch64")]
use hyper::hal::interrupt::InterruptId;
use hyper::hal::interrupt::InterruptTrigger;
use hyper::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::arch::CrashContext;

const MAX_CPUS: usize = hyper::config::MAX_CPUS as usize;
const NO_CRASH_OWNER: usize = usize::MAX;
const STOP_WAIT_LIMIT: usize = 10_000_000;
const MAX_BACKTRACE_DEPTH: usize = 32;

static CRASH_OWNER: AtomicUsize = AtomicUsize::new(NO_CRASH_OWNER);
static CRASH_IPI_READY: AtomicBool = AtomicBool::new(false);
static STOPPED_CPUS: AtomicUsize = AtomicUsize::new(0);
static CPU_CONTEXTS: [CrashSlot; MAX_CPUS] = [const { CrashSlot::new() }; MAX_CPUS];

struct CrashSlot {
    published: AtomicBool,
    context: UnsafeCell<MaybeUninit<CrashContext>>,
}

impl CrashSlot {
    const fn new() -> Self {
        Self {
            published: AtomicBool::new(false),
            context: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    fn publish(&self, context: CrashContext) {
        // SAFETY: Each CPU publishes at most once after local exceptions are
        // masked. Release publication makes the complete Copy value visible.
        unsafe { (*self.context.get()).write(context) };
        self.published.store(true, Ordering::Release);
    }

    fn read(&self) -> Option<CrashContext> {
        if !self.published.load(Ordering::Acquire) {
            return None;
        }
        // SAFETY: Acquire observed the release after full initialization, and
        // crash contexts are immutable after publication.
        Some(unsafe { *(*self.context.get()).assume_init_ref() })
    }
}

// SAFETY: Per-CPU single-writer publication is synchronized by `published`.
unsafe impl Sync for CrashSlot {}

/// Reserves and installs the all-but-self crash-stop interrupt.
pub(crate) fn initialize(boot: &super::boot::Initialization) {
    let Some(hardware_interrupt) = crate::arch::crash_stop_interrupt() else {
        crate::println!("HypeR: crash-stop cross-call is unavailable on this platform");
        return;
    };
    let interrupt = match super::irq::interrupt::map(
        boot.interrupts().root_domain,
        hardware_interrupt,
        0,
        InterruptTrigger::Edge,
    ) {
        Ok(interrupt) => interrupt,
        Err(error) => super::boot::fail("crash-stop interrupt mapping", error),
    };
    if let Err(error) = super::irq::interrupt::register_shared(interrupt, 0, crash_stop_interrupt) {
        super::boot::fail("crash-stop interrupt registration", error);
    }
    CRASH_IPI_READY.store(true, Ordering::Release);
    crate::println!("HypeR: crash-stop IPI and CPU state capture initialized");
}

fn crash_stop_interrupt(
    _interrupt: super::irq::interrupt::VirtualInterrupt,
    _context: usize,
) -> super::irq::interrupt::HandlerResult {
    super::irq::interrupt::HandlerResult::Handled
}

#[cfg(target_arch = "aarch64")]
pub(crate) fn is_stop_interrupt(interrupt: InterruptId) -> bool {
    CRASH_IPI_READY.load(Ordering::Acquire) && crate::arch::is_crash_stop_interrupt(interrupt)
}

/// Handles a Rust panic without delegating crash policy to the log subsystem.
#[unsafe(export_name = "kernel_crash_panic")]
pub fn panic(info: &PanicInfo<'_>) -> ! {
    enter(
        crate::arch::capture_crash_context(),
        format_args!("Kernel panic - not syncing: {info}"),
    )
}

/// Handles a fatal architectural exception with its exact interrupted frame.
#[cfg(target_arch = "aarch64")]
pub(crate) fn fatal_exception(
    report: hyper::hal::exception::ExceptionReport,
    context: CrashContext,
) -> ! {
    enter(
        context,
        format_args!(
            "fatal {:?} from {:?}: {} (EC {:#x}, ESR {:#x}, FAR {:#x})",
            report.kind,
            report.origin,
            report.description,
            report.architecture_class,
            report.syndrome,
            report.fault_address_register
        ),
    )
}

/// Handles fatal kernel failures that do not already carry an exception frame.
pub(crate) fn fatal(arguments: fmt::Arguments<'_>) -> ! {
    enter(crate::arch::capture_crash_context(), arguments)
}

/// Handles a fatal failure that already has an architecture snapshot.
pub(crate) fn fatal_context(context: CrashContext, arguments: fmt::Arguments<'_>) -> ! {
    enter(context, arguments)
}

/// Publishes a remote CPU's exact IRQ frame and permanently stops that CPU.
#[cfg(target_arch = "aarch64")]
pub(crate) fn stop_this_cpu(context: CrashContext) -> ! {
    let mut payload = StopPayload { context };
    // SAFETY: payload remains live on the abandoned stack and the callback
    // never returns after switching to the per-CPU emergency stack.
    unsafe {
        crate::arch::run_on_emergency_stack(
            stop_this_cpu_on_emergency_stack,
            (&mut payload as *mut StopPayload) as usize,
        )
    }
}

#[cfg(target_arch = "aarch64")]
struct StopPayload {
    context: CrashContext,
}

#[cfg(target_arch = "aarch64")]
extern "C" fn stop_this_cpu_on_emergency_stack(argument: usize) -> ! {
    // SAFETY: stop_this_cpu passes one live payload and the callback never
    // returns to outlive it.
    let payload = unsafe { &*(argument as *const StopPayload) };
    crate::arch::disable_local_interrupts();
    publish_current_cpu(payload.context);
    STOPPED_CPUS.fetch_add(1, Ordering::AcqRel);
    crate::arch::halt()
}

fn enter(context: CrashContext, reason: fmt::Arguments<'_>) -> ! {
    let mut payload = CrashPayload { context, reason };
    // SAFETY: payload remains live on the abandoned stack and fatal handling
    // permanently owns control after switching to the emergency stack.
    unsafe {
        crate::arch::run_on_emergency_stack(
            enter_on_emergency_stack,
            (&mut payload as *mut CrashPayload<'_>) as usize,
        )
    }
}

struct CrashPayload<'reason> {
    context: CrashContext,
    reason: fmt::Arguments<'reason>,
}

extern "C" fn enter_on_emergency_stack(argument: usize) -> ! {
    // SAFETY: enter passes one live payload and this callback never returns.
    let payload = unsafe { &*(argument as *const CrashPayload<'_>) };
    crate::arch::disable_local_interrupts();
    let cpu = crate::arch::current_cpu_index();
    match CRASH_OWNER.compare_exchange(NO_CRASH_OWNER, cpu, Ordering::AcqRel, Ordering::Acquire) {
        Ok(_) => {}
        Err(owner) if owner != cpu => {
            publish_current_cpu(payload.context);
            STOPPED_CPUS.fetch_add(1, Ordering::AcqRel);
            crate::arch::halt()
        }
        Err(_) => {
            super::log::emergency(format_args!(
                "RECURSIVE KERNEL PANIC on CPU {cpu}; diagnostics aborted"
            ));
            crate::arch::halt()
        }
    }

    publish_current_cpu(payload.context);
    let stop = stop_other_cpus();
    emit_banner(cpu, payload.reason, stop);
    dump_cpu_states(cpu);
    super::log::emergency(format_args!(
        "---[ end HypeR kernel panic - system halted ]---"
    ));
    crate::arch::halt()
}

fn publish_current_cpu(context: CrashContext) {
    if let Some(slot) = CPU_CONTEXTS.get(crate::arch::current_cpu_index()) {
        slot.publish(context);
    }
}

#[derive(Clone, Copy)]
struct StopResult {
    expected: usize,
    stopped: usize,
    sent: bool,
}

fn stop_other_cpus() -> StopResult {
    let expected = super::cpu::online_cpu_count().saturating_sub(1);
    let sent = expected != 0
        && CRASH_IPI_READY.load(Ordering::Acquire)
        && crate::arch::broadcast_crash_stop();
    if sent {
        for _ in 0..STOP_WAIT_LIMIT {
            if STOPPED_CPUS.load(Ordering::Acquire) >= expected {
                break;
            }
            spin_loop();
        }
    }
    StopResult {
        expected,
        stopped: STOPPED_CPUS.load(Ordering::Acquire).min(expected),
        sent,
    }
}

fn emit_banner(cpu: usize, reason: fmt::Arguments<'_>, stop: StopResult) {
    super::log::emergency(format_args!(
        "============================================================"
    ));
    super::log::emergency(format_args!("HypeR KERNEL PANIC - NOT SYNCING"));
    super::log::emergency(format_args!(
        "============================================================"
    ));
    super::log::emergency(format_args!("BUG: fatal kernel failure on CPU {cpu}"));
    super::log::emergency(reason);
    if stop.expected == 0 {
        super::log::emergency(format_args!("SMP: no other online CPUs"));
    } else if !stop.sent {
        super::log::emergency(format_args!(
            "SMP: crash-stop IPI unavailable; {} CPU(s) may still be running",
            stop.expected
        ));
    } else {
        super::log::emergency(format_args!(
            "SMP: stopped {}/{} other online CPU(s)",
            stop.stopped, stop.expected
        ));
    }
}

fn dump_cpu_states(owner: usize) {
    let online = super::cpu::online_cpu_count().max(owner.saturating_add(1));
    for (cpu, slot) in CPU_CONTEXTS.iter().enumerate().take(online.min(MAX_CPUS)) {
        let Some(context) = slot.read() else {
            super::log::emergency(format_args!(
                "CPU {cpu}: ONLINE, failed to stop or state unavailable"
            ));
            continue;
        };
        let role = if cpu == owner { "crashing" } else { "stopped" };
        let task = super::task::scheduler::crash_snapshot(cpu);
        dump_cpu_header(cpu, role, context, task);
        dump_registers(&context);
        dump_backtrace(cpu, &context, task);
    }
}

fn dump_cpu_header(
    cpu: usize,
    role: &str,
    context: CrashContext,
    task: Option<super::task::scheduler::CrashTaskSnapshot>,
) {
    #[cfg(target_arch = "aarch64")]
    super::log::emergency(format_args!(
        "CPU {cpu}: {role}, MPIDR {:#x}, CurrentEL {:#x}, DAIF {:#x}",
        context.hardware_id, context.current_el, context.interrupt_mask
    ));
    #[cfg(target_arch = "riscv64")]
    super::log::emergency(format_args!(
        "CPU {cpu}: {role}, hart ID {:#x}, SSTATUS {:#x}",
        context.hardware_id, context.interrupt_mask
    ));
    match task {
        Some(task) => {
            super::log::emergency(format_args!(
                "CPU {cpu}: task {} {:?} {:?}",
                task.id.get(),
                task.state,
                task.execution
            ));
            if let Some(stack) = task.stack_statistics {
                super::log::emergency(format_args!(
                    "CPU {cpu}: task stack {}/{} bytes used, guard {:#x}, canary {}",
                    stack.used,
                    stack.size,
                    stack.guard_page,
                    if stack.canary_intact {
                        "intact"
                    } else {
                        "CORRUPTED"
                    }
                ));
            }
        }
        None => super::log::emergency(format_args!(
            "CPU {cpu}: current task unavailable (scheduler lock busy or not initialized)"
        )),
    }
}

fn dump_registers(context: &CrashContext) {
    #[cfg(target_arch = "aarch64")]
    dump_general_registers(context, 31);
    #[cfg(target_arch = "riscv64")]
    dump_general_registers(context, 32);
    emit_symbolized("PC", context.program_counter, context.program_counter);
    super::log::emergency(format_args!(
        "SP: {:#018x}  STATUS: {:#018x}",
        context.stack_pointer, context.processor_state
    ));
    #[cfg(target_arch = "aarch64")]
    dump_architecture_registers(context);
    #[cfg(target_arch = "riscv64")]
    dump_architecture_registers(context);
}

fn dump_general_registers(context: &CrashContext, register_count: usize) {
    for base in (0..register_count).step_by(4) {
        let remaining = register_count - base;
        if remaining < 4 {
            match remaining {
                1 => super::log::emergency(format_args!(
                    "x{base:02}: {}",
                    RegisterValue::new(context, base)
                )),
                2 => super::log::emergency(format_args!(
                    "x{base:02}: {}  x{:02}: {}",
                    RegisterValue::new(context, base),
                    base + 1,
                    RegisterValue::new(context, base + 1)
                )),
                3 => super::log::emergency(format_args!(
                    "x{base:02}: {}  x{:02}: {}  x{:02}: {}",
                    RegisterValue::new(context, base),
                    base + 1,
                    RegisterValue::new(context, base + 1),
                    base + 2,
                    RegisterValue::new(context, base + 2)
                )),
                _ => {}
            }
            break;
        }
        super::log::emergency(format_args!(
            "x{base:02}: {}  x{:02}: {}  x{:02}: {}  x{:02}: {}",
            RegisterValue::new(context, base),
            base + 1,
            RegisterValue::new(context, base + 1),
            base + 2,
            RegisterValue::new(context, base + 2),
            base + 3,
            RegisterValue::new(context, base + 3)
        ));
    }
}

#[cfg(target_arch = "aarch64")]
fn dump_architecture_registers(context: &CrashContext) {
    if context.has_exception_frame() {
        super::log::emergency(format_args!(
            "ESR: {:#018x}  FAR: {:#018x}  vector: {:#x}",
            context.syndrome, context.fault_address, context.exception_vector
        ));
    } else {
        super::log::emergency(format_args!(
            "origin: software panic (no architectural exception frame)"
        ));
    }
    super::log::emergency(format_args!(
        "SCTLR_EL2: {:#018x}  TCR_EL2: {:#018x}  TTBR0_EL2: {:#018x}",
        context.sctlr_el2, context.tcr_el2, context.ttbr0_el2
    ));
    super::log::emergency(format_args!(
        "VBAR_EL2: {:#018x}  HCR_EL2: {:#018x}",
        context.vbar_el2, context.hcr_el2
    ));
}

#[cfg(target_arch = "riscv64")]
fn dump_architecture_registers(context: &CrashContext) {
    if context.has_exception_frame() {
        super::log::emergency(format_args!(
            "SCAUSE: {:#018x}  STVAL: {:#018x}",
            context.syndrome, context.fault_address
        ));
    } else {
        super::log::emergency(format_args!(
            "origin: software panic (no architectural exception frame)"
        ));
    }
    super::log::emergency(format_args!(
        "SATP: {:#018x}  STVEC: {:#018x}",
        context.satp, context.stvec
    ));
    super::log::emergency(format_args!(
        "HSTATUS: {:#018x}  HGATP: {:#018x}",
        context.hstatus, context.hgatp
    ));
}

struct RegisterValue {
    valid: bool,
    value: u64,
}

impl RegisterValue {
    fn new(context: &CrashContext, register: usize) -> Self {
        Self {
            valid: context.general_is_valid(register),
            value: context.general.get(register).copied().unwrap_or(0),
        }
    }
}

impl fmt::Display for RegisterValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.valid {
            write!(formatter, "{:#018x}", self.value)
        } else {
            formatter.write_str("??????????????????")
        }
    }
}

fn dump_backtrace(
    cpu: usize,
    context: &CrashContext,
    task: Option<super::task::scheduler::CrashTaskSnapshot>,
) {
    super::log::emergency(format_args!("CPU {cpu} Call trace:"));
    emit_trace_entry(0, context.program_counter, context.program_counter);
    #[cfg(target_arch = "aarch64")]
    const FRAME_POINTER_REGISTER: usize = 29;
    #[cfg(target_arch = "riscv64")]
    const FRAME_POINTER_REGISTER: usize = 8;
    if !context.general_is_valid(FRAME_POINTER_REGISTER) {
        super::log::emergency(format_args!("  frame pointer unavailable"));
        return;
    }
    let Some((bottom, top)) = stack_bounds(cpu, context.stack_pointer, task) else {
        super::log::emergency(format_args!("  stack bounds unavailable; unwind stopped"));
        return;
    };
    walk_frame_chain(
        context.general[FRAME_POINTER_REGISTER] as usize,
        bottom,
        top,
    );
}

fn stack_bounds(
    cpu: usize,
    stack_pointer: u64,
    task: Option<super::task::scheduler::CrashTaskSnapshot>,
) -> Option<(usize, usize)> {
    task.and_then(|task| task.stack)
        .filter(|(bottom, top)| *bottom <= stack_pointer as usize && stack_pointer as usize <= *top)
        .or_else(|| crate::arch::bootstrap_stack_bounds(stack_pointer))
        .or_else(|| super::mm::stack::exception_stack_bounds(cpu, stack_pointer as usize))
}

#[cfg(target_arch = "aarch64")]
fn walk_frame_chain(mut frame: usize, bottom: usize, top: usize) {
    for depth in 1..MAX_BACKTRACE_DEPTH {
        if frame & 0xf != 0 || frame < bottom || frame.checked_add(16).is_none_or(|end| end > top) {
            super::log::emergency(format_args!(
                "  invalid frame pointer {frame:#x}; unwind stopped"
            ));
            return;
        }
        // SAFETY: The frame is aligned and both words lie inside the current
        // kernel stack allocation, which remains pinned during crash handling.
        let (previous, link) = unsafe {
            (
                read_volatile(frame as *const usize),
                read_volatile((frame + core::mem::size_of::<usize>()) as *const usize),
            )
        };
        if link == 0 {
            return;
        }
        emit_trace_entry(depth, link as u64, (link as u64).saturating_sub(4));
        if previous <= frame {
            return;
        }
        frame = previous;
    }
    super::log::emergency(format_args!(
        "  backtrace truncated at {MAX_BACKTRACE_DEPTH} entries"
    ));
}

#[cfg(target_arch = "riscv64")]
fn walk_frame_chain(mut frame: usize, bottom: usize, top: usize) {
    const RECORD_SIZE: usize = 2 * core::mem::size_of::<usize>();
    for depth in 1..MAX_BACKTRACE_DEPTH {
        if frame & 0xf != 0 || frame < bottom + RECORD_SIZE || frame > top {
            super::log::emergency(format_args!(
                "  invalid frame pointer {frame:#x}; unwind stopped"
            ));
            return;
        }
        let record = frame - RECORD_SIZE;
        // SAFETY: The frame is aligned and the standard RISC-V frame record
        // lies entirely inside the pinned current kernel stack.
        let (previous, link) = unsafe {
            (
                read_volatile(record as *const usize),
                read_volatile((record + core::mem::size_of::<usize>()) as *const usize),
            )
        };
        if link == 0 {
            return;
        }
        emit_trace_entry(depth, link as u64, (link as u64).saturating_sub(4));
        if previous <= frame {
            return;
        }
        frame = previous;
    }
    super::log::emergency(format_args!(
        "  backtrace truncated at {MAX_BACKTRACE_DEPTH} entries"
    ));
}

fn emit_trace_entry(depth: usize, address: u64, lookup_address: u64) {
    match usize::try_from(lookup_address)
        .ok()
        .and_then(|address| super::debug::kallsyms::lookup(address).ok().flatten())
    {
        Some(symbol) => {
            super::log::emergency(format_args!("  #{depth:02} [<{address:#018x}>] {symbol}"))
        }
        None => super::log::emergency(format_args!("  #{depth:02} [<{address:#018x}>]")),
    }
}

fn emit_symbolized(label: &str, address: u64, lookup_address: u64) {
    match usize::try_from(lookup_address)
        .ok()
        .and_then(|address| super::debug::kallsyms::lookup(address).ok().flatten())
    {
        Some(symbol) => super::log::emergency(format_args!("{label}: {address:#018x} <{symbol}>")),
        None => super::log::emergency(format_args!("{label}: {address:#018x}")),
    }
}
