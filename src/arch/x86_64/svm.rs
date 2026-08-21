// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! AMD Secure Virtual Machine (SVM) backend with nested paging.

use core::arch::asm;
use core::cell::UnsafeCell;
use core::ptr::{read_volatile, write_volatile};

use hyper::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use hyper::vm::x86::device::legacy_pc::InterruptSource;
use hyper::vm::x86::svm::{IoDirection, IoExit, NptAccess, NptViolation, SvmFeatures};
use hyper::vm::x86::{CpuidResult, GuestMsr, hypervisor_cpuid, merge_port_input, sanitize_cpuid};

use super::context::VcpuContext;
use super::svm_registers::*;

const MAX_CPUS: usize = hyper::config::MAX_CPUS as usize;
const PAGE_SIZE: usize = 4096;

const GUEST_BOOT_CR3: u64 = 0x70000;
const GUEST_GDT: u64 = 0x50000;
const GUEST_TSS: u64 = 0x51000;
const RESET_PAT: u64 = 0x0007_0406_0007_0406;

#[repr(C, align(4096))]
struct Page([u8; PAGE_SIZE]);

struct Pages(UnsafeCell<[Page; MAX_CPUS]>);
// SAFETY: Each array element is accessed only by its matching CPU with local
// interrupts masked. SVM hardware accesses that same element only during that
// CPU's serialized VMRUN transition; no Rust reference is retained across a
// transition that could conflict with hardware access.
unsafe impl Sync for Pages {}

#[repr(C, align(4096))]
struct IoPermissionMap([u8; PAGE_SIZE * 3]);

#[repr(C, align(4096))]
struct MsrPermissionMap([u8; PAGE_SIZE * 2]);

static VMCBS: Pages = Pages(UnsafeCell::new([const { Page([0; PAGE_SIZE]) }; MAX_CPUS]));
static HOST_SAVE_AREAS: Pages = Pages(UnsafeCell::new([const { Page([0; PAGE_SIZE]) }; MAX_CPUS]));
static IOPM: IoPermissionMap = IoPermissionMap([0xff; PAGE_SIZE * 3]);
static MSRPM: MsrPermissionMap = MsrPermissionMap([0xff; PAGE_SIZE * 2]);
static SVM_ACTIVE: [AtomicBool; MAX_CPUS] = [const { AtomicBool::new(false) }; MAX_CPUS];
static ACTIVE_NPT_ROOT: [AtomicU64; MAX_CPUS] = [const { AtomicU64::new(0) }; MAX_CPUS];
static TLB_PENDING: [AtomicBool; MAX_CPUS] = [const { AtomicBool::new(false) }; MAX_CPUS];
static TIMER_PENDING: [AtomicBool; MAX_CPUS] = [const { AtomicBool::new(false) }; MAX_CPUS];
static ACCEPTING_HOST_INTERRUPT: [AtomicBool; MAX_CPUS] =
    [const { AtomicBool::new(false) }; MAX_CPUS];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Error {
    InterruptsEnabled,
    InvalidAddress,
    InvalidCpu,
}

unsafe extern "C" {
    fn x86_64_svm_run(context: *mut VcpuContext, vmcb: u64, host_save: u64) -> !;
}

pub(super) fn validate() -> Result<(), super::guest::ValidationError> {
    if read_msr(MSR_VM_CR) & VM_CR_SVM_DISABLE != 0 {
        return Err(super::guest::ValidationError::HardwareUnavailable);
    }
    let leaf = core::arch::x86_64::__cpuid(0x8000_000a);
    let features = SvmFeatures::decode(leaf.eax, leaf.ebx, leaf.edx);
    if !features.nested_paging || features.asids <= 1 {
        return Err(super::guest::ValidationError::SecondLevelPagingUnavailable);
    }
    if !features.next_rip {
        return Err(super::guest::ValidationError::MissingNextRip);
    }
    Ok(())
}

pub(super) unsafe fn enter(context: *mut VcpuContext) -> ! {
    if super::local_irq_enabled() {
        crate::kernel::boot::fail(
            "SVM guest entry with local IRQs enabled",
            Error::InterruptsEnabled,
        );
    }
    let cpu = super::current_cpu_index();
    // SAFETY: The caller guarantees a valid exclusive context. The temporary
    // borrow ends before VMRUN and exit access through the raw pointer.
    let prepared = unsafe { prepare(cpu, &*context) };
    let (vmcb, host_save) = match prepared {
        Ok(addresses) => addresses,
        Err(error) => crate::kernel::boot::fail("SVM guest entry preparation", error),
    };
    // SAFETY: No Rust reference to the context remains live across VMRUN.
    unsafe { x86_64_svm_run(context, vmcb, host_save) }
}

pub(super) fn activate_npt(root: u64) {
    let cpu = super::current_cpu_index();
    let Some(slot) = ACTIVE_NPT_ROOT.get(cpu) else {
        crate::kernel::boot::fail("SVM NPT CPU lookup", Error::InvalidCpu);
    };
    slot.store(root, Ordering::Release);
}

pub(super) fn invalidate_npt() {
    tlb_pending().store(true, Ordering::Release);
}

fn prepare(cpu: usize, context: &VcpuContext) -> Result<(u64, u64), Error> {
    if cpu >= MAX_CPUS {
        return Err(Error::InvalidCpu);
    }
    enable(cpu)?;
    // SAFETY: Guest entry is serialized per CPU; this CPU exclusively owns its
    // page-aligned VMCB until the non-returning VMRUN transition.
    let vmcb = unsafe { &mut (*VMCBS.0.get())[cpu] };
    vmcb.0.fill(0);
    write_control_area(vmcb)?;
    write_guest_state(vmcb, context);
    let vmcb_pa = kernel_physical(vmcb.0.as_ptr() as usize)?;
    // SAFETY: Host-save pages are immutable after per-CPU initialization and
    // this in-bounds slot remains pinned for the kernel lifetime.
    let host = unsafe { &(*HOST_SAVE_AREAS.0.get())[cpu] };
    let host_pa = kernel_physical(host.0.as_ptr() as usize)?;
    Ok((vmcb_pa, host_pa))
}

fn enable(cpu: usize) -> Result<(), Error> {
    if SVM_ACTIVE[cpu].load(Ordering::Acquire) {
        return Ok(());
    }
    // SAFETY: Initialization is serialized by the per-CPU SVM_ACTIVE state;
    // this CPU exclusively initializes its pinned host-save page.
    let host = unsafe { &mut (*HOST_SAVE_AREAS.0.get())[cpu] };
    host.0.fill(0);
    let physical = kernel_physical(host.0.as_ptr() as usize)?;
    write_msr(
        GuestMsr::Efer.index(),
        read_msr(GuestMsr::Efer.index()) | EFER_SVME,
    );
    write_msr(MSR_VM_HSAVE_PA, physical);
    SVM_ACTIVE[cpu].store(true, Ordering::Release);
    Ok(())
}

fn write_control_area(vmcb: &mut Page) -> Result<(), Error> {
    let word3 = INTERCEPT_INTR
        | INTERCEPT_CPUID
        | INTERCEPT_HLT
        | INTERCEPT_IO
        | INTERCEPT_MSR
        | INTERCEPT_SHUTDOWN;
    vmcb.write_u32(VMCB_INTERCEPT_WORD3, word3);
    vmcb.write_u32(VMCB_INTERCEPT_WORD4, INTERCEPT_SVM_INSTRUCTIONS);
    let iopm = &IOPM;
    let msrpm = &MSRPM;
    vmcb.write_u64(VMCB_IOPM_BASE, kernel_physical(iopm.0.as_ptr() as usize)?);
    vmcb.write_u64(VMCB_MSRPM_BASE, kernel_physical(msrpm.0.as_ptr() as usize)?);
    vmcb.write_u32(VMCB_ASID, 1);
    vmcb.write_u8(VMCB_TLB_CONTROL, 1);
    vmcb.write_u32(VMCB_INT_CONTROL, V_INTR_MASKING);
    vmcb.write_u64(VMCB_NESTED_CONTROL, 1);
    let root = active_npt_root().ok_or(Error::InvalidAddress)?;
    vmcb.write_u64(VMCB_NESTED_CR3, root);
    Ok(())
}

fn write_guest_state(vmcb: &mut Page, context: &VcpuContext) {
    vmcb.write_segment(SAVE_ES, 0x10, 0xc93, u32::MAX, 0);
    vmcb.write_segment(SAVE_CS, 0x08, 0xa9b, u32::MAX, 0);
    vmcb.write_segment(SAVE_SS, 0x10, 0xc93, u32::MAX, 0);
    vmcb.write_segment(SAVE_DS, 0x10, 0xc93, u32::MAX, 0);
    vmcb.write_segment(SAVE_FS, 0, 0xc93, u32::MAX, 0);
    vmcb.write_segment(SAVE_GS, 0, 0xc93, u32::MAX, 0);
    vmcb.write_segment(SAVE_GDTR, 0, 0, 39, GUEST_GDT);
    vmcb.write_segment(SAVE_LDTR, 0, 0, 0, 0);
    vmcb.write_segment(SAVE_IDTR, 0, 0, 0xffff, 0);
    vmcb.write_segment(SAVE_TR, 0x18, 0x08b, 0x67, GUEST_TSS);
    vmcb.write_u8(SAVE_CPL, 0);
    // VMRUN requires EFER.SVME in the guest save area even though SVM is not
    // advertised to the guest and all SVM instructions are intercepted.
    vmcb.write_u64(SAVE_EFER, EFER_SVME | (1 << 8) | (1 << 10) | (1 << 11));
    vmcb.write_u64(SAVE_CR4, 0x620);
    vmcb.write_u64(SAVE_CR3, GUEST_BOOT_CR3);
    vmcb.write_u64(SAVE_CR0, 0x8001_0033);
    vmcb.write_u64(SAVE_DR7, 0x400);
    vmcb.write_u64(SAVE_DR6, 0xffff_0ff0);
    vmcb.write_u64(SAVE_RFLAGS, context.flags | 2);
    vmcb.write_u64(SAVE_RIP, context.instruction_pointer);
    vmcb.write_u64(SAVE_RSP, context.general[4]);
    vmcb.write_u64(SAVE_RAX, context.general[0]);
    write_guest_msrs(vmcb, context);
}

fn write_guest_msrs(vmcb: &mut Page, context: &VcpuContext) {
    vmcb.write_u64(SAVE_STAR, context.msrs.star);
    vmcb.write_u64(SAVE_LSTAR, context.msrs.lstar);
    vmcb.write_u64(SAVE_CSTAR, context.msrs.cstar);
    vmcb.write_u64(SAVE_SFMASK, context.msrs.sfmask);
    vmcb.write_u64(SAVE_KERNEL_GS_BASE, context.msrs.kernel_gs_base);
    vmcb.write_u64(SAVE_SYSENTER_CS, 0);
    vmcb.write_u64(SAVE_SYSENTER_ESP, 0);
    vmcb.write_u64(SAVE_SYSENTER_EIP, 0);
    vmcb.write_u64(SAVE_PAT, RESET_PAT);
}

#[unsafe(no_mangle)]
extern "C" fn x86_64_svm_exit_dispatch(context: &mut VcpuContext) {
    // SAFETY: A VMRUN exit is serialized on this CPU, local interrupts remain
    // masked, and this is the sole Rust reference to the active per-CPU VMCB
    // for the duration of dispatch.
    let vmcb = unsafe { &mut *active_vmcb_pointer() };
    synchronize_context(vmcb, context);
    let exit_code = vmcb.read_u64(VMCB_EXIT_CODE);
    match exit_code {
        EXIT_INTR => handle_external_interrupt(),
        EXIT_CPUID => {
            emulate_cpuid(context);
            vmcb.write_u64(SAVE_RAX, context.general[0]);
            advance_rip(vmcb);
        }
        EXIT_HLT => advance_rip(vmcb),
        EXIT_IO => {
            emulate_io(vmcb, context);
            advance_rip(vmcb);
        }
        EXIT_MSR => emulate_msr(vmcb, context),
        EXIT_NPF => handle_npf(vmcb),
        EXIT_SHUTDOWN => crate::kernel::boot::fail("SVM guest shutdown", exit_code),
        u64::MAX => crate::kernel::boot::fail("invalid SVM guest state", dump_exit(vmcb)),
        _ => crate::kernel::boot::fail("unhandled SVM exit", dump_exit(vmcb)),
    }
    prepare_guest_interrupt(vmcb);
    if tlb_pending().swap(false, Ordering::AcqRel) {
        vmcb.write_u8(VMCB_TLB_CONTROL, 3);
    }
    vmcb.write_u32(VMCB_CLEAN_BITS, 0);
}

fn synchronize_context(vmcb: &Page, context: &mut VcpuContext) {
    context.general[0] = vmcb.read_u64(SAVE_RAX);
    context.general[4] = vmcb.read_u64(SAVE_RSP);
    context.instruction_pointer = vmcb.read_u64(SAVE_RIP);
    context.flags = vmcb.read_u64(SAVE_RFLAGS);
    context.msrs.star = vmcb.read_u64(SAVE_STAR);
    context.msrs.lstar = vmcb.read_u64(SAVE_LSTAR);
    context.msrs.cstar = vmcb.read_u64(SAVE_CSTAR);
    context.msrs.sfmask = vmcb.read_u64(SAVE_SFMASK);
    context.msrs.kernel_gs_base = vmcb.read_u64(SAVE_KERNEL_GS_BASE);
}

fn emulate_cpuid(context: &mut VcpuContext) {
    let leaf = context.general[0] as u32;
    let subleaf = context.general[2] as u32;
    let value = if leaf == 0x4000_0000 {
        hypervisor_cpuid()
    } else if (0x4000_0001..0x5000_0000).contains(&leaf) {
        CpuidResult::ZERO
    } else {
        let host = core::arch::x86_64::__cpuid_count(leaf, subleaf);
        sanitize_cpuid(
            leaf,
            subleaf,
            CpuidResult {
                eax: host.eax,
                ebx: host.ebx,
                ecx: host.ecx,
                edx: host.edx,
            },
        )
    };
    context.general[0] = u64::from(value.eax);
    context.general[1] = u64::from(value.ebx);
    context.general[2] = u64::from(value.ecx);
    context.general[3] = u64::from(value.edx);
}

fn emulate_io(vmcb: &mut Page, context: &mut VcpuContext) {
    let info = vmcb.read_u64(VMCB_EXIT_INFO1);
    let Some(exit) = IoExit::decode(info) else {
        crate::kernel::boot::fail("invalid SVM I/O access", info);
    };
    if exit.string || exit.repeat {
        crate::kernel::boot::fail("unsupported SVM string I/O", info);
    }
    let input = exit.direction == IoDirection::Input;
    let result = match crate::kernel::entry::vmexit::dispatch_port_io(
        exit.port,
        exit.size,
        !input,
        context.general[0] as u32,
    ) {
        Ok(value) => value,
        Err(error) => crate::kernel::boot::fail("legacy PC port I/O", error),
    };
    if let Some(value) = result {
        context.general[0] = merge_port_input(context.general[0], value, exit.size)
            .unwrap_or_else(|| crate::kernel::boot::fail("invalid SVM I/O access size", exit.size));
        vmcb.write_u64(SAVE_RAX, context.general[0]);
    }
}

fn emulate_msr(vmcb: &mut Page, context: &mut VcpuContext) {
    let msr = context.general[2] as u32;
    if vmcb.read_u64(VMCB_EXIT_INFO1) == 0 {
        let Some(value) = read_guest_msr(vmcb, context, msr) else {
            inject_general_protection(vmcb);
            return;
        };
        context.general[0] = value as u32 as u64;
        context.general[3] = (value >> 32) as u32 as u64;
    } else {
        let value = (context.general[0] as u32 as u64) | ((context.general[3] as u32 as u64) << 32);
        if !write_guest_msr(vmcb, context, msr, value) {
            inject_general_protection(vmcb);
            return;
        }
    }
    vmcb.write_u64(SAVE_RAX, context.general[0]);
    advance_rip(vmcb);
}

fn read_guest_msr(vmcb: &Page, context: &VcpuContext, msr: u32) -> Option<u64> {
    Some(match GuestMsr::decode(msr)? {
        GuestMsr::SysenterCs => vmcb.read_u64(SAVE_SYSENTER_CS),
        GuestMsr::SysenterEsp => vmcb.read_u64(SAVE_SYSENTER_ESP),
        GuestMsr::SysenterEip => vmcb.read_u64(SAVE_SYSENTER_EIP),
        GuestMsr::Efer => vmcb.read_u64(SAVE_EFER) & !EFER_SVME,
        GuestMsr::FsBase => vmcb.read_u64(SAVE_FS + 8),
        GuestMsr::GsBase => vmcb.read_u64(SAVE_GS + 8),
        GuestMsr::ApicBase => 0,
        GuestMsr::TimestampCounter => read_msr(GuestMsr::TimestampCounter.index()),
        GuestMsr::Pat => vmcb.read_u64(SAVE_PAT),
        GuestMsr::Star => context.msrs.star,
        GuestMsr::Lstar => context.msrs.lstar,
        GuestMsr::Cstar => context.msrs.cstar,
        GuestMsr::Sfmask => context.msrs.sfmask,
        GuestMsr::KernelGsBase => context.msrs.kernel_gs_base,
        GuestMsr::TscAux => context.msrs.tsc_aux,
    })
}

fn write_guest_msr(vmcb: &mut Page, context: &mut VcpuContext, msr: u32, value: u64) -> bool {
    let Some(msr) = GuestMsr::decode(msr) else {
        return false;
    };
    match msr {
        GuestMsr::SysenterCs => vmcb.write_u64(SAVE_SYSENTER_CS, value),
        GuestMsr::SysenterEsp => vmcb.write_u64(SAVE_SYSENTER_ESP, value),
        GuestMsr::SysenterEip => vmcb.write_u64(SAVE_SYSENTER_EIP, value),
        // SVME is a VMRUN implementation requirement, not part of the
        // guest-visible CPU contract (CPUID does not advertise SVM).
        GuestMsr::Efer => vmcb.write_u64(SAVE_EFER, value | EFER_SVME),
        GuestMsr::FsBase => vmcb.write_u64(SAVE_FS + 8, value),
        GuestMsr::GsBase => vmcb.write_u64(SAVE_GS + 8, value),
        GuestMsr::Pat => vmcb.write_u64(SAVE_PAT, value),
        GuestMsr::Star => {
            context.msrs.star = value;
            vmcb.write_u64(SAVE_STAR, value);
        }
        GuestMsr::Lstar => {
            context.msrs.lstar = value;
            vmcb.write_u64(SAVE_LSTAR, value);
        }
        GuestMsr::Cstar => {
            context.msrs.cstar = value;
            vmcb.write_u64(SAVE_CSTAR, value);
        }
        GuestMsr::Sfmask => {
            context.msrs.sfmask = value;
            vmcb.write_u64(SAVE_SFMASK, value);
        }
        GuestMsr::KernelGsBase => {
            context.msrs.kernel_gs_base = value;
            vmcb.write_u64(SAVE_KERNEL_GS_BASE, value);
        }
        GuestMsr::TscAux => context.msrs.tsc_aux = value,
        GuestMsr::TimestampCounter | GuestMsr::ApicBase => return false,
    }
    true
}

fn handle_npf(vmcb: &Page) {
    let info = vmcb.read_u64(VMCB_EXIT_INFO1);
    let address = vmcb.read_u64(VMCB_EXIT_INFO2);
    let violation = NptViolation::decode(info);
    let access = match violation.access {
        NptAccess::Read => hyper::vm::exit::MemoryAccess::Read,
        NptAccess::Write => hyper::vm::exit::MemoryAccess::Write,
        NptAccess::Execute => hyper::vm::exit::MemoryAccess::Execute,
    };
    let mut frame = super::GuestSyncFrame::memory_fault(hyper::vm::exit::GuestMemoryFault::new(
        hyper::vm::exit::GuestPhysicalAddress::new(address),
        access,
        violation.during_page_walk,
    ));
    if !crate::kernel::entry::vmexit::dispatch_legacy(&mut frame) {
        crate::kernel::boot::fail("unhandled NPT violation", (address, info));
    }
}

fn handle_external_interrupt() {
    // SVM reports that a physical interrupt is pending, not its vector. Let
    // the local APIC deliver it through the normal IDT, then mask IRQs again
    // before the next VMRUN.
    let accepting = accepting_host_interrupt();
    accepting.store(true, Ordering::Release);
    // SAFETY: This executes at CPL0 on an SVM-enabled CPU. STGI/STI admits one
    // host interrupt window; CLI/CLGI closes it before returning to VMRUN.
    unsafe { asm!("stgi", "sti", "nop", "cli", "clgi", options(nostack)) };
    accepting.store(false, Ordering::Release);
}

pub(super) fn observe_host_interrupt(vector: u32) {
    if vector == super::platform::TIMER_VECTOR && accepting_host_interrupt().load(Ordering::Acquire)
    {
        timer_pending().store(true, Ordering::Release);
    }
}

fn prepare_guest_interrupt(vmcb: &mut Page) {
    let timer_pending = timer_pending();
    let pending = match crate::kernel::entry::vmexit::pending_legacy_interrupt(
        timer_pending.load(Ordering::Acquire),
    ) {
        Ok(Some(pending)) => pending,
        Ok(None) => return,
        Err(error) => crate::kernel::boot::fail("legacy PIC interrupt routing", error),
    };
    let mut control = vmcb.read_u32(VMCB_INT_CONTROL);
    control &= !((0xf << V_INTR_PRIORITY_SHIFT) | V_IRQ | V_IGNORE_TPR);
    control |= V_INTR_MASKING
        | V_IRQ
        | V_IGNORE_TPR
        | (u32::from(pending.vector >> 4) << V_INTR_PRIORITY_SHIFT);
    vmcb.write_u32(VMCB_INT_VECTOR, u32::from(pending.vector));
    vmcb.write_u32(VMCB_INT_CONTROL, control);
    if pending.source == InterruptSource::Timer {
        timer_pending.store(false, Ordering::Release);
    }
}

fn inject_general_protection(vmcb: &mut Page) {
    vmcb.write_u32(VMCB_EVENT_ERROR, 0);
    vmcb.write_u32(
        VMCB_EVENT_INJECTION,
        EVENT_VALID | EVENT_ERROR_VALID | EVENT_EXCEPTION | EXCEPTION_GENERAL_PROTECTION,
    );
}

fn advance_rip(vmcb: &mut Page) {
    vmcb.write_u64(SAVE_RIP, vmcb.read_u64(VMCB_NEXT_RIP));
}

fn active_vmcb_pointer() -> *mut Page {
    let cpu = super::current_cpu_index();
    if cpu >= MAX_CPUS {
        crate::kernel::boot::fail("SVM VMCB CPU lookup", Error::InvalidCpu);
    }
    // `Page` elements are contiguous at the start of the backing array. A raw
    // pointer is returned so lookup itself does not forge a `'static` mutable
    // reference; the serialized exit boundary creates the only reference.
    VMCBS.0.get().cast::<Page>().wrapping_add(cpu)
}

fn active_npt_root() -> Option<u64> {
    ACTIVE_NPT_ROOT
        .get(super::current_cpu_index())
        .map(|slot| slot.load(Ordering::Acquire))
        .filter(|root| *root != 0)
}

fn tlb_pending() -> &'static AtomicBool {
    TLB_PENDING
        .get(super::current_cpu_index())
        .unwrap_or_else(|| crate::kernel::boot::fail("SVM TLB CPU lookup", Error::InvalidCpu))
}

fn timer_pending() -> &'static AtomicBool {
    TIMER_PENDING
        .get(super::current_cpu_index())
        .unwrap_or_else(|| crate::kernel::boot::fail("SVM timer CPU lookup", Error::InvalidCpu))
}

fn accepting_host_interrupt() -> &'static AtomicBool {
    ACCEPTING_HOST_INTERRUPT
        .get(super::current_cpu_index())
        .unwrap_or_else(|| crate::kernel::boot::fail("SVM IRQ CPU lookup", Error::InvalidCpu))
}

fn dump_exit(vmcb: &Page) -> (u64, u64, u64, u64) {
    (
        vmcb.read_u64(VMCB_EXIT_CODE),
        vmcb.read_u64(VMCB_EXIT_INFO1),
        vmcb.read_u64(VMCB_EXIT_INFO2),
        vmcb.read_u64(SAVE_RIP),
    )
}

impl Page {
    fn read_u32(&self, offset: usize) -> u32 {
        // SAFETY: Callers use aligned architectural VMCB offsets contained in
        // this pinned 4-KiB page.
        unsafe { read_volatile(self.0.as_ptr().add(offset).cast::<u32>()) }
    }

    fn read_u64(&self, offset: usize) -> u64 {
        // SAFETY: Callers use aligned architectural VMCB offsets contained in
        // this pinned 4-KiB page.
        unsafe { read_volatile(self.0.as_ptr().add(offset).cast::<u64>()) }
    }

    fn write_u8(&mut self, offset: usize, value: u8) {
        // SAFETY: The exclusive page borrow and architectural offset identify
        // one initialized byte within this pinned VMCB.
        unsafe { write_volatile(self.0.as_mut_ptr().add(offset), value) };
    }

    fn write_u32(&mut self, offset: usize, value: u32) {
        // SAFETY: Callers provide aligned architectural offsets within this
        // exclusively borrowed pinned VMCB page.
        unsafe { write_volatile(self.0.as_mut_ptr().add(offset).cast::<u32>(), value) };
    }

    fn write_u64(&mut self, offset: usize, value: u64) {
        // SAFETY: Callers provide aligned architectural offsets within this
        // exclusively borrowed pinned VMCB page.
        unsafe { write_volatile(self.0.as_mut_ptr().add(offset).cast::<u64>(), value) };
    }

    fn write_segment(&mut self, offset: usize, selector: u16, attr: u16, limit: u32, base: u64) {
        // SAFETY: `offset` is an aligned architectural segment-state offset;
        // all four fields fit inside this exclusively borrowed VMCB page.
        unsafe {
            write_volatile(self.0.as_mut_ptr().add(offset).cast::<u16>(), selector);
            write_volatile(self.0.as_mut_ptr().add(offset + 2).cast::<u16>(), attr);
            write_volatile(self.0.as_mut_ptr().add(offset + 4).cast::<u32>(), limit);
            write_volatile(self.0.as_mut_ptr().add(offset + 8).cast::<u64>(), base);
        }
    }
}

fn kernel_physical(virtual_address: usize) -> Result<u64, Error> {
    crate::kernel::boot::with_boot_state(|state| {
        (virtual_address as u64)
            .checked_sub(state.memory.kernel_base())
            .and_then(|offset| state.image_physical_start.checked_add(offset))
    })
    .ok_or(Error::InvalidAddress)
}

fn read_msr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;
    // SAFETY: This kernel executes at CPL0 and callers request MSRs admitted by
    // the SVM initialization and guest-state policy.
    unsafe { asm!("rdmsr", in("ecx") msr, out("eax") low, out("edx") high, options(nostack)) };
    u64::from(low) | (u64::from(high) << 32)
}

fn write_msr(msr: u32, value: u64) {
    // SAFETY: This kernel executes at CPL0 and callers write only the admitted
    // SVM/host-state MSRs with architecture-valid values.
    unsafe {
        asm!(
            "wrmsr",
            in("ecx") msr,
            in("eax") value as u32,
            in("edx") (value >> 32) as u32,
            options(nostack)
        )
    };
}
