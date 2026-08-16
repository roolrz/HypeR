use core::arch::asm;
use core::cell::UnsafeCell;
use core::ptr::{read_unaligned, write_volatile};

use hyper::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use super::context::{VcpuContext, VcpuMsrState};

const MAX_CPUS: usize = hyper::config::MAX_CPUS as usize;
const PAGE_SIZE: usize = 4096;
const HOST_STACK_SIZE: usize = 16 * 1024;

const IA32_FEATURE_CONTROL: u32 = 0x3a;
const IA32_VMX_BASIC: u32 = 0x480;
const IA32_VMX_PINBASED_CTLS: u32 = 0x481;
const IA32_VMX_PROCBASED_CTLS: u32 = 0x482;
const IA32_VMX_EXIT_CTLS: u32 = 0x483;
const IA32_VMX_ENTRY_CTLS: u32 = 0x484;
const IA32_VMX_CR0_FIXED0: u32 = 0x486;
const IA32_VMX_CR0_FIXED1: u32 = 0x487;
const IA32_VMX_CR4_FIXED0: u32 = 0x488;
const IA32_VMX_CR4_FIXED1: u32 = 0x489;
const IA32_VMX_PROCBASED_CTLS2: u32 = 0x48b;
const IA32_VMX_EPT_VPID_CAP: u32 = 0x48c;
const IA32_VMX_TRUE_PINBASED_CTLS: u32 = 0x48d;
const IA32_VMX_TRUE_PROCBASED_CTLS: u32 = 0x48e;
const IA32_VMX_TRUE_EXIT_CTLS: u32 = 0x48f;
const IA32_VMX_TRUE_ENTRY_CTLS: u32 = 0x490;
const IA32_EFER: u32 = 0xc000_0080;
const IA32_FS_BASE: u32 = 0xc000_0100;
const IA32_GS_BASE: u32 = 0xc000_0101;

const VMCS_CTRL_PIN: u64 = 0x4000;
const VMCS_CTRL_PRIMARY: u64 = 0x4002;
const VMCS_CTRL_EXCEPTION_BITMAP: u64 = 0x4004;
const VMCS_CTRL_EXIT: u64 = 0x400c;
const VMCS_CTRL_EXIT_MSR_STORE_COUNT: u64 = 0x400e;
const VMCS_CTRL_EXIT_MSR_LOAD_COUNT: u64 = 0x4010;
const VMCS_CTRL_ENTRY: u64 = 0x4012;
const VMCS_CTRL_ENTRY_MSR_LOAD_COUNT: u64 = 0x4014;
const VMCS_CTRL_ENTRY_INTERRUPT: u64 = 0x4016;
const VMCS_CTRL_SECONDARY: u64 = 0x401e;
const VMCS_EPT_POINTER: u64 = 0x201a;
const VMCS_EXIT_MSR_STORE_ADDRESS: u64 = 0x2006;
const VMCS_EXIT_MSR_LOAD_ADDRESS: u64 = 0x2008;
const VMCS_ENTRY_MSR_LOAD_ADDRESS: u64 = 0x200a;
const VMCS_LINK_POINTER: u64 = 0x2800;
const VMCS_GUEST_PAT: u64 = 0x2804;
const VMCS_GUEST_EFER: u64 = 0x2806;
const VMCS_HOST_PAT: u64 = 0x2c00;
const VMCS_HOST_EFER: u64 = 0x2c02;

const VMCS_GUEST_ES_SELECTOR: u64 = 0x0800;
const VMCS_GUEST_CS_SELECTOR: u64 = 0x0802;
const VMCS_GUEST_SS_SELECTOR: u64 = 0x0804;
const VMCS_GUEST_DS_SELECTOR: u64 = 0x0806;
const VMCS_GUEST_FS_SELECTOR: u64 = 0x0808;
const VMCS_GUEST_GS_SELECTOR: u64 = 0x080a;
const VMCS_GUEST_LDTR_SELECTOR: u64 = 0x080c;
const VMCS_GUEST_TR_SELECTOR: u64 = 0x080e;
const VMCS_HOST_ES_SELECTOR: u64 = 0x0c00;
const VMCS_HOST_CS_SELECTOR: u64 = 0x0c02;
const VMCS_HOST_SS_SELECTOR: u64 = 0x0c04;
const VMCS_HOST_DS_SELECTOR: u64 = 0x0c06;
const VMCS_HOST_FS_SELECTOR: u64 = 0x0c08;
const VMCS_HOST_GS_SELECTOR: u64 = 0x0c0a;
const VMCS_HOST_TR_SELECTOR: u64 = 0x0c0c;

const VMCS_GUEST_ES_LIMIT: u64 = 0x4800;
const VMCS_GUEST_CS_LIMIT: u64 = 0x4802;
const VMCS_GUEST_SS_LIMIT: u64 = 0x4804;
const VMCS_GUEST_DS_LIMIT: u64 = 0x4806;
const VMCS_GUEST_FS_LIMIT: u64 = 0x4808;
const VMCS_GUEST_GS_LIMIT: u64 = 0x480a;
const VMCS_GUEST_LDTR_LIMIT: u64 = 0x480c;
const VMCS_GUEST_TR_LIMIT: u64 = 0x480e;
const VMCS_GUEST_GDTR_LIMIT: u64 = 0x4810;
const VMCS_GUEST_IDTR_LIMIT: u64 = 0x4812;
const VMCS_GUEST_ES_AR: u64 = 0x4814;
const VMCS_GUEST_CS_AR: u64 = 0x4816;
const VMCS_GUEST_SS_AR: u64 = 0x4818;
const VMCS_GUEST_DS_AR: u64 = 0x481a;
const VMCS_GUEST_FS_AR: u64 = 0x481c;
const VMCS_GUEST_GS_AR: u64 = 0x481e;
const VMCS_GUEST_LDTR_AR: u64 = 0x4820;
const VMCS_GUEST_TR_AR: u64 = 0x4822;
const VMCS_GUEST_INTERRUPTIBILITY: u64 = 0x4824;
const VMCS_GUEST_ACTIVITY: u64 = 0x4826;
const VMCS_GUEST_SYSENTER_CS: u64 = 0x482a;
const VMCS_HOST_SYSENTER_CS: u64 = 0x4c00;

const VMCS_GUEST_CR0: u64 = 0x6800;
const VMCS_GUEST_CR3: u64 = 0x6802;
const VMCS_GUEST_CR4: u64 = 0x6804;
const VMCS_GUEST_ES_BASE: u64 = 0x6806;
const VMCS_GUEST_CS_BASE: u64 = 0x6808;
const VMCS_GUEST_SS_BASE: u64 = 0x680a;
const VMCS_GUEST_DS_BASE: u64 = 0x680c;
const VMCS_GUEST_FS_BASE: u64 = 0x680e;
const VMCS_GUEST_GS_BASE: u64 = 0x6810;
const VMCS_GUEST_LDTR_BASE: u64 = 0x6812;
const VMCS_GUEST_TR_BASE: u64 = 0x6814;
const VMCS_GUEST_GDTR_BASE: u64 = 0x6816;
const VMCS_GUEST_IDTR_BASE: u64 = 0x6818;
const VMCS_GUEST_DR7: u64 = 0x681a;
const VMCS_GUEST_RSP: u64 = 0x681c;
const VMCS_GUEST_RIP: u64 = 0x681e;
const VMCS_GUEST_RFLAGS: u64 = 0x6820;
const VMCS_GUEST_PENDING_DEBUG: u64 = 0x6822;
const VMCS_GUEST_SYSENTER_ESP: u64 = 0x6824;
const VMCS_GUEST_SYSENTER_EIP: u64 = 0x6826;

const VMCS_HOST_CR0: u64 = 0x6c00;
const VMCS_HOST_CR3: u64 = 0x6c02;
const VMCS_HOST_CR4: u64 = 0x6c04;
const VMCS_HOST_FS_BASE: u64 = 0x6c06;
const VMCS_HOST_GS_BASE: u64 = 0x6c08;
const VMCS_HOST_TR_BASE: u64 = 0x6c0a;
const VMCS_HOST_GDTR_BASE: u64 = 0x6c0c;
const VMCS_HOST_IDTR_BASE: u64 = 0x6c0e;
const VMCS_HOST_SYSENTER_ESP: u64 = 0x6c10;
const VMCS_HOST_SYSENTER_EIP: u64 = 0x6c12;
const VMCS_HOST_RSP: u64 = 0x6c14;
const VMCS_HOST_RIP: u64 = 0x6c16;

const VMCS_EXIT_REASON: u64 = 0x4402;
const VMCS_EXIT_INTERRUPT_INFO: u64 = 0x4404;
const VMCS_EXIT_INSTRUCTION_LENGTH: u64 = 0x440c;
const VMCS_EXIT_QUALIFICATION: u64 = 0x6400;
const VMCS_GUEST_PHYSICAL_ADDRESS: u64 = 0x2400;
const VMCS_INSTRUCTION_ERROR: u64 = 0x4400;

const PRIMARY_INTERRUPT_WINDOW: u32 = 1 << 2;
const PRIMARY_HLT_EXITING: u32 = 1 << 7;
const PRIMARY_UNCONDITIONAL_IO: u32 = 1 << 24;
const PRIMARY_SECONDARY_CONTROLS: u32 = 1 << 31;
const PIN_EXTERNAL_INTERRUPT_EXITING: u32 = 1;
const SECONDARY_EPT: u32 = 1 << 1;
const EXIT_HOST_64_BIT: u32 = 1 << 9;
const EXIT_ACK_INTERRUPT: u32 = 1 << 15;
const EXIT_SAVE_PAT: u32 = 1 << 18;
const EXIT_LOAD_PAT: u32 = 1 << 19;
const EXIT_SAVE_EFER: u32 = 1 << 20;
const EXIT_LOAD_EFER: u32 = 1 << 21;
const ENTRY_GUEST_64_BIT: u32 = 1 << 9;
const ENTRY_LOAD_PAT: u32 = 1 << 14;
const ENTRY_LOAD_EFER: u32 = 1 << 15;

const EXIT_EXTERNAL_INTERRUPT: u32 = 1;
const EXIT_INTERRUPT_WINDOW: u32 = 7;
const EXIT_CPUID: u32 = 10;
const EXIT_HLT: u32 = 12;
const EXIT_IO: u32 = 30;
const EXIT_RDMSR: u32 = 31;
const EXIT_WRMSR: u32 = 32;
const EXIT_EPT_VIOLATION: u32 = 48;

const IA32_APIC_BASE: u32 = 0x1b;
const IA32_PAT: u32 = 0x277;
const IA32_SYSENTER_CS: u32 = 0x174;
const IA32_SYSENTER_ESP: u32 = 0x175;
const IA32_SYSENTER_EIP: u32 = 0x176;
const IA32_STAR: u32 = 0xc000_0081;
const IA32_LSTAR: u32 = 0xc000_0082;
const IA32_CSTAR: u32 = 0xc000_0083;
const IA32_SFMASK: u32 = 0xc000_0084;
const IA32_KERNEL_GS_BASE: u32 = 0xc000_0102;
const IA32_TSC_AUX: u32 = 0xc000_0103;

const GUEST_BOOT_CR3: u64 = 0x70000;
const GUEST_GDT: u64 = 0x50000;
const GUEST_TSS: u64 = 0x51000;
const RESET_PAT: u64 = 0x0007_0406_0007_0406;
const GUEST_MSR_COUNT: usize = 5;

#[repr(C, align(4096))]
struct VmxPage([u8; PAGE_SIZE]);
struct VmxPages(UnsafeCell<[VmxPage; MAX_CPUS]>);
unsafe impl Sync for VmxPages {}
#[repr(C, align(16))]
struct HostStacks(UnsafeCell<[[u8; HOST_STACK_SIZE]; MAX_CPUS]>);
unsafe impl Sync for HostStacks {}

#[derive(Clone, Copy)]
#[repr(C)]
struct MsrEntry {
    index: u32,
    reserved: u32,
    value: u64,
}

impl MsrEntry {
    const EMPTY: Self = Self {
        index: 0,
        reserved: 0,
        value: 0,
    };

    const fn new(index: u32, value: u64) -> Self {
        Self {
            index,
            reserved: 0,
            value,
        }
    }
}

#[repr(C, align(16))]
struct MsrList([MsrEntry; GUEST_MSR_COUNT]);

const _: () = {
    assert!(core::mem::size_of::<MsrEntry>() == 16);
    assert!(core::mem::align_of::<MsrList>() >= 16);
};

struct MsrLists(UnsafeCell<[MsrList; MAX_CPUS]>);

// SAFETY: Each list is accessed only by its matching CPU while local IRQs are
// masked, or by VMX hardware as part of that CPU's entry/exit transition.
unsafe impl Sync for MsrLists {}

static VMXON_PAGES: VmxPages = VmxPages(UnsafeCell::new(
    [const { VmxPage([0; PAGE_SIZE]) }; MAX_CPUS],
));
static VMCS_PAGES: VmxPages = VmxPages(UnsafeCell::new(
    [const { VmxPage([0; PAGE_SIZE]) }; MAX_CPUS],
));
static HOST_STACKS: HostStacks = HostStacks(UnsafeCell::new([[0; HOST_STACK_SIZE]; MAX_CPUS]));
static GUEST_MSR_LISTS: MsrLists = MsrLists(UnsafeCell::new(
    [const { MsrList([MsrEntry::EMPTY; GUEST_MSR_COUNT]) }; MAX_CPUS],
));
static HOST_MSR_LISTS: MsrLists = MsrLists(UnsafeCell::new(
    [const { MsrList([MsrEntry::EMPTY; GUEST_MSR_COUNT]) }; MAX_CPUS],
));
static VMX_ACTIVE: [AtomicBool; MAX_CPUS] = [const { AtomicBool::new(false) }; MAX_CPUS];
static ACTIVE_CONTEXT: [AtomicUsize; MAX_CPUS] = [const { AtomicUsize::new(0) }; MAX_CPUS];
static TIMER_PENDING: AtomicBool = AtomicBool::new(false);

#[repr(C)]
pub struct ExitRegisters {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    r11: u64,
    r10: u64,
    r9: u64,
    r8: u64,
    rdi: u64,
    rsi: u64,
    rbp: u64,
    rdx: u64,
    rcx: u64,
    rbx: u64,
    rax: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidAddress,
    InvalidCpu,
    InvalidControl,
    VmxInstruction,
}

unsafe extern "C" {
    fn x86_64_vmlaunch(context: *const VcpuContext) -> !;
    fn x86_64_vmexit_entry();
}

pub unsafe fn enter(context: &mut VcpuContext) -> ! {
    if let Err(error) = prepare_vmcs(context) {
        crate::kernel::boot::fail("VMX guest entry preparation", error);
    }
    let cpu = super::current_cpu_index();
    let Some(slot) = ACTIVE_CONTEXT.get(cpu) else {
        crate::kernel::boot::fail("VMX active-context publication", cpu);
    };
    slot.store(context as *mut VcpuContext as usize, Ordering::Release);
    unsafe { x86_64_vmlaunch(context) }
}

pub(super) fn validate() -> Result<(), super::guest::ValidationError> {
    let basic_features = core::arch::x86_64::__cpuid(1);
    if basic_features.ecx & (1 << 5) == 0 {
        return Err(super::guest::ValidationError::VmxUnavailable);
    }
    let secondary = read_msr(IA32_VMX_PROCBASED_CTLS2) >> 32;
    let ept = read_msr(IA32_VMX_EPT_VPID_CAP);
    let required_ept = (1 << 6) | (1 << 14) | (1 << 20) | (1 << 25);
    if secondary & u64::from(SECONDARY_EPT) == 0 || ept & required_ept != required_ept {
        return Err(super::guest::ValidationError::EptUnavailable);
    }
    Ok(())
}

fn prepare_vmcs(context: &VcpuContext) -> Result<(), Error> {
    let cpu = super::current_cpu_index();
    if cpu >= MAX_CPUS {
        return Err(Error::InvalidCpu);
    }
    enable_vmx(cpu)?;
    let vmcs = unsafe { &mut (*VMCS_PAGES.0.get())[cpu] };
    vmcs.0.fill(0);
    let revision = read_msr(IA32_VMX_BASIC) as u32;
    unsafe { write_volatile(vmcs.0.as_mut_ptr().cast::<u32>(), revision) };
    let vmcs_pa = kernel_physical(vmcs.0.as_ptr() as usize)?;
    vmclear(vmcs_pa)?;
    vmptrld(vmcs_pa)?;

    let eptp = super::stage2::active_eptp().ok_or(Error::InvalidAddress)?;
    write_controls(eptp)?;
    write_guest_state(context)?;
    write_host_state(cpu)?;
    write_msr_lists(cpu, context)?;
    Ok(())
}

fn write_msr_lists(cpu: usize, context: &VcpuContext) -> Result<(), Error> {
    // SAFETY: `cpu` was validated against MAX_CPUS, VMX is stopped, and this
    // CPU exclusively owns both per-CPU lists until the following VM entry.
    let guest = unsafe { &mut (*GUEST_MSR_LISTS.0.get())[cpu] };
    guest.0 = msr_entries(context.msrs);
    // SAFETY: The same per-CPU ownership rule applies to the host restore list.
    let host = unsafe { &mut (*HOST_MSR_LISTS.0.get())[cpu] };
    host.0 = msr_entries(VcpuMsrState {
        star: read_msr(IA32_STAR),
        lstar: read_msr(IA32_LSTAR),
        cstar: read_msr(IA32_CSTAR),
        sfmask: read_msr(IA32_SFMASK),
        kernel_gs_base: read_msr(IA32_KERNEL_GS_BASE),
        tsc_aux: 0,
    });
    let guest_address = kernel_physical(guest.0.as_ptr() as usize)?;
    let host_address = kernel_physical(host.0.as_ptr() as usize)?;
    vmwrite(VMCS_EXIT_MSR_STORE_ADDRESS, guest_address)?;
    vmwrite(VMCS_EXIT_MSR_LOAD_ADDRESS, host_address)?;
    vmwrite(VMCS_ENTRY_MSR_LOAD_ADDRESS, guest_address)?;
    vmwrite(VMCS_CTRL_EXIT_MSR_STORE_COUNT, GUEST_MSR_COUNT as u64)?;
    vmwrite(VMCS_CTRL_EXIT_MSR_LOAD_COUNT, GUEST_MSR_COUNT as u64)?;
    vmwrite(VMCS_CTRL_ENTRY_MSR_LOAD_COUNT, GUEST_MSR_COUNT as u64)
}

const fn msr_entries(state: VcpuMsrState) -> [MsrEntry; GUEST_MSR_COUNT] {
    [
        MsrEntry::new(IA32_STAR, state.star),
        MsrEntry::new(IA32_LSTAR, state.lstar),
        MsrEntry::new(IA32_CSTAR, state.cstar),
        MsrEntry::new(IA32_SFMASK, state.sfmask),
        MsrEntry::new(IA32_KERNEL_GS_BASE, state.kernel_gs_base),
    ]
}

fn enable_vmx(cpu: usize) -> Result<(), Error> {
    if VMX_ACTIVE[cpu].load(Ordering::Acquire) {
        return Ok(());
    }
    let feature = read_msr(IA32_FEATURE_CONTROL);
    if feature & 1 == 0 {
        write_msr(IA32_FEATURE_CONTROL, feature | 1 | (1 << 2));
    } else if feature & (1 << 2) == 0 {
        return Err(Error::InvalidControl);
    }
    let mut cr0 = read_cr0();
    cr0 |= read_msr(IA32_VMX_CR0_FIXED0);
    cr0 &= read_msr(IA32_VMX_CR0_FIXED1);
    let mut cr4 = read_cr4() | (1 << 13);
    cr4 |= read_msr(IA32_VMX_CR4_FIXED0);
    cr4 &= read_msr(IA32_VMX_CR4_FIXED1);
    write_cr0(cr0);
    write_cr4(cr4);

    let page = unsafe { &mut (*VMXON_PAGES.0.get())[cpu] };
    page.0.fill(0);
    unsafe {
        write_volatile(
            page.0.as_mut_ptr().cast::<u32>(),
            read_msr(IA32_VMX_BASIC) as u32,
        )
    };
    vmxon(kernel_physical(page.0.as_ptr() as usize)?)?;
    VMX_ACTIVE[cpu].store(true, Ordering::Release);
    Ok(())
}

fn write_controls(eptp: u64) -> Result<(), Error> {
    vmwrite(
        VMCS_CTRL_PIN,
        u64::from(adjust_control(
            PIN_EXTERNAL_INTERRUPT_EXITING,
            control_msr(IA32_VMX_PINBASED_CTLS, IA32_VMX_TRUE_PINBASED_CTLS),
        )?),
    )?;
    vmwrite(
        VMCS_CTRL_PRIMARY,
        u64::from(adjust_control(
            PRIMARY_HLT_EXITING | PRIMARY_UNCONDITIONAL_IO | PRIMARY_SECONDARY_CONTROLS,
            control_msr(IA32_VMX_PROCBASED_CTLS, IA32_VMX_TRUE_PROCBASED_CTLS),
        )?),
    )?;
    vmwrite(
        VMCS_CTRL_SECONDARY,
        u64::from(adjust_control(SECONDARY_EPT, IA32_VMX_PROCBASED_CTLS2)?),
    )?;
    vmwrite(
        VMCS_CTRL_EXIT,
        u64::from(adjust_control(
            EXIT_HOST_64_BIT
                | EXIT_ACK_INTERRUPT
                | EXIT_SAVE_PAT
                | EXIT_LOAD_PAT
                | EXIT_SAVE_EFER
                | EXIT_LOAD_EFER,
            control_msr(IA32_VMX_EXIT_CTLS, IA32_VMX_TRUE_EXIT_CTLS),
        )?),
    )?;
    vmwrite(
        VMCS_CTRL_ENTRY,
        u64::from(adjust_control(
            ENTRY_GUEST_64_BIT | ENTRY_LOAD_PAT | ENTRY_LOAD_EFER,
            control_msr(IA32_VMX_ENTRY_CTLS, IA32_VMX_TRUE_ENTRY_CTLS),
        )?),
    )?;
    vmwrite(VMCS_CTRL_EXCEPTION_BITMAP, 0)?;
    vmwrite(VMCS_CTRL_ENTRY_INTERRUPT, 0)?;
    vmwrite(VMCS_EPT_POINTER, eptp)?;
    Ok(())
}

fn write_guest_state(context: &VcpuContext) -> Result<(), Error> {
    for (field, value) in [
        (VMCS_GUEST_ES_SELECTOR, 0x10),
        (VMCS_GUEST_CS_SELECTOR, 0x08),
        (VMCS_GUEST_SS_SELECTOR, 0x10),
        (VMCS_GUEST_DS_SELECTOR, 0x10),
        (VMCS_GUEST_FS_SELECTOR, 0),
        (VMCS_GUEST_GS_SELECTOR, 0),
        (VMCS_GUEST_LDTR_SELECTOR, 0),
        (VMCS_GUEST_TR_SELECTOR, 0x18),
    ] {
        vmwrite(field, value)?;
    }
    for field in [
        VMCS_GUEST_ES_LIMIT,
        VMCS_GUEST_CS_LIMIT,
        VMCS_GUEST_SS_LIMIT,
        VMCS_GUEST_DS_LIMIT,
        VMCS_GUEST_FS_LIMIT,
        VMCS_GUEST_GS_LIMIT,
    ] {
        vmwrite(field, 0xffff_ffff)?;
    }
    vmwrite(VMCS_GUEST_LDTR_LIMIT, 0)?;
    vmwrite(VMCS_GUEST_TR_LIMIT, 0x67)?;
    vmwrite(VMCS_GUEST_GDTR_LIMIT, 39)?;
    vmwrite(VMCS_GUEST_IDTR_LIMIT, 0xffff)?;
    for field in [VMCS_GUEST_ES_AR, VMCS_GUEST_SS_AR, VMCS_GUEST_DS_AR] {
        vmwrite(field, 0xc093)?;
    }
    vmwrite(VMCS_GUEST_FS_AR, 1 << 16)?;
    vmwrite(VMCS_GUEST_GS_AR, 1 << 16)?;
    vmwrite(VMCS_GUEST_CS_AR, 0xa09b)?;
    vmwrite(VMCS_GUEST_LDTR_AR, 1 << 16)?;
    vmwrite(VMCS_GUEST_TR_AR, 0x008b)?;
    for field in [
        VMCS_GUEST_ES_BASE,
        VMCS_GUEST_CS_BASE,
        VMCS_GUEST_SS_BASE,
        VMCS_GUEST_DS_BASE,
        VMCS_GUEST_FS_BASE,
        VMCS_GUEST_GS_BASE,
        VMCS_GUEST_LDTR_BASE,
    ] {
        vmwrite(field, 0)?;
    }
    vmwrite(VMCS_GUEST_TR_BASE, GUEST_TSS)?;
    vmwrite(VMCS_GUEST_GDTR_BASE, GUEST_GDT)?;
    vmwrite(VMCS_GUEST_IDTR_BASE, 0)?;
    vmwrite(VMCS_GUEST_CR0, 0x8001_0033)?;
    vmwrite(VMCS_GUEST_CR3, GUEST_BOOT_CR3)?;
    vmwrite(VMCS_GUEST_CR4, 0x620)?;
    vmwrite(VMCS_GUEST_DR7, 0x400)?;
    vmwrite(VMCS_GUEST_RSP, context.general[4])?;
    vmwrite(VMCS_GUEST_RIP, context.instruction_pointer)?;
    vmwrite(VMCS_GUEST_RFLAGS, context.flags | 2)?;
    vmwrite(VMCS_GUEST_PENDING_DEBUG, 0)?;
    vmwrite(VMCS_GUEST_INTERRUPTIBILITY, 0)?;
    vmwrite(VMCS_GUEST_ACTIVITY, 0)?;
    vmwrite(VMCS_GUEST_SYSENTER_CS, 0)?;
    vmwrite(VMCS_GUEST_SYSENTER_ESP, 0)?;
    vmwrite(VMCS_GUEST_SYSENTER_EIP, 0)?;
    vmwrite(VMCS_GUEST_PAT, RESET_PAT)?;
    vmwrite(VMCS_GUEST_EFER, (1 << 8) | (1 << 10) | (1 << 11))?;
    vmwrite(VMCS_LINK_POINTER, u64::MAX)
}

fn write_host_state(cpu: usize) -> Result<(), Error> {
    let mut gdtr = [0_u8; 10];
    let mut idtr = [0_u8; 10];
    unsafe {
        asm!("sgdt [{}]", in(reg) gdtr.as_mut_ptr(), options(nostack));
        asm!("sidt [{}]", in(reg) idtr.as_mut_ptr(), options(nostack));
    }
    let gdtr_base = unsafe { read_unaligned(gdtr.as_ptr().add(2).cast::<u64>()) };
    let idtr_base = unsafe { read_unaligned(idtr.as_ptr().add(2).cast::<u64>()) };
    let tr = read_tr();
    let tr_base = descriptor_base(gdtr_base, tr)?;
    for (field, value) in [
        (VMCS_HOST_ES_SELECTOR, u64::from(read_es() & !7)),
        (VMCS_HOST_CS_SELECTOR, u64::from(read_cs() & !7)),
        (VMCS_HOST_SS_SELECTOR, u64::from(read_ss() & !7)),
        (VMCS_HOST_DS_SELECTOR, u64::from(read_ds() & !7)),
        (VMCS_HOST_FS_SELECTOR, u64::from(read_fs() & !7)),
        (VMCS_HOST_GS_SELECTOR, u64::from(read_gs() & !7)),
        (VMCS_HOST_TR_SELECTOR, u64::from(tr & !7)),
    ] {
        vmwrite(field, value)?;
    }
    vmwrite(VMCS_HOST_CR0, read_cr0())?;
    vmwrite(VMCS_HOST_CR3, read_cr3())?;
    vmwrite(VMCS_HOST_CR4, read_cr4())?;
    vmwrite(VMCS_HOST_FS_BASE, read_msr(IA32_FS_BASE))?;
    vmwrite(VMCS_HOST_GS_BASE, read_msr(IA32_GS_BASE))?;
    vmwrite(VMCS_HOST_TR_BASE, tr_base)?;
    vmwrite(VMCS_HOST_GDTR_BASE, gdtr_base)?;
    vmwrite(VMCS_HOST_IDTR_BASE, idtr_base)?;
    vmwrite(VMCS_HOST_SYSENTER_CS, 0)?;
    vmwrite(VMCS_HOST_SYSENTER_ESP, 0)?;
    vmwrite(VMCS_HOST_SYSENTER_EIP, 0)?;
    vmwrite(VMCS_HOST_PAT, read_msr(IA32_PAT))?;
    vmwrite(VMCS_HOST_EFER, read_msr(IA32_EFER))?;
    let stack = unsafe { &(*HOST_STACKS.0.get())[cpu] };
    vmwrite(
        VMCS_HOST_RSP,
        stack.as_ptr() as u64 + HOST_STACK_SIZE as u64 - 8,
    )?;
    vmwrite(
        VMCS_HOST_RIP,
        x86_64_vmexit_entry as *const () as usize as u64,
    )
}

fn active_context_pointer() -> *mut VcpuContext {
    let cpu = super::current_cpu_index();
    let address = ACTIVE_CONTEXT
        .get(cpu)
        .map(|slot| slot.load(Ordering::Acquire))
        .unwrap_or(0);
    if address == 0 {
        crate::kernel::boot::fail("VMX active-context lookup", cpu);
    }
    address as *mut VcpuContext
}

fn synchronize_guest_msrs(context: &mut VcpuContext) {
    let cpu = super::current_cpu_index();
    // SAFETY: VM exit completed its store list before transferring control to
    // this dispatcher, and hardware will not access it again until VM entry.
    let Some(list) = (unsafe { &*GUEST_MSR_LISTS.0.get() }).get(cpu) else {
        crate::kernel::boot::fail("VMX guest-MSR list lookup", cpu);
    };
    context.msrs = VcpuMsrState {
        star: list.0[0].value,
        lstar: list.0[1].value,
        cstar: list.0[2].value,
        sfmask: list.0[3].value,
        kernel_gs_base: list.0[4].value,
        tsc_aux: context.msrs.tsc_aux,
    };
}

fn update_guest_msr(msr: u32, value: u64) {
    let cpu = super::current_cpu_index();
    // SAFETY: This runs between VM exit and VM entry on the owning CPU, while
    // VMX hardware is not accessing the guest load/store list.
    let Some(list) = (unsafe { &mut *GUEST_MSR_LISTS.0.get() }).get_mut(cpu) else {
        crate::kernel::boot::fail("VMX guest-MSR list update", cpu);
    };
    if let Some(entry) = list.0.iter_mut().find(|entry| entry.index == msr) {
        entry.value = value;
    }
}

fn save_guest_context(registers: &ExitRegisters, context: &mut VcpuContext) {
    context.general = [
        registers.rax,
        registers.rbx,
        registers.rcx,
        registers.rdx,
        read_vmcs(VMCS_GUEST_RSP),
        registers.rbp,
        registers.rsi,
        registers.rdi,
        registers.r8,
        registers.r9,
        registers.r10,
        registers.r11,
        registers.r12,
        registers.r13,
        registers.r14,
        registers.r15,
    ];
    context.instruction_pointer = read_vmcs(VMCS_GUEST_RIP);
    context.flags = read_vmcs(VMCS_GUEST_RFLAGS);
}

#[unsafe(no_mangle)]
extern "C" fn x86_64_vmexit_dispatch(registers: &mut ExitRegisters) {
    let context = active_context_pointer();
    // SAFETY: VM entry published the pinned active context for this CPU, and
    // VM exit is serialized with any future vCPU migration.
    synchronize_guest_msrs(unsafe { &mut *context });
    let reason = read_vmcs(VMCS_EXIT_REASON) as u32 & 0xffff;
    match reason {
        EXIT_EXTERNAL_INTERRUPT => handle_external_interrupt(),
        EXIT_INTERRUPT_WINDOW => {}
        EXIT_CPUID => {
            emulate_cpuid(registers);
            advance_rip();
        }
        EXIT_HLT => advance_rip(),
        EXIT_IO => {
            emulate_io(registers);
            advance_rip();
        }
        EXIT_RDMSR => {
            emulate_rdmsr(registers, unsafe { &*context });
            advance_rip();
        }
        EXIT_WRMSR => {
            emulate_wrmsr(registers, unsafe { &mut *context });
            advance_rip();
        }
        EXIT_EPT_VIOLATION => handle_ept_violation(),
        _ => crate::kernel::boot::fail("unhandled VMX exit", reason),
    }
    prepare_guest_interrupt();
    // SAFETY: No subsystem borrow of the active vCPU survives its callback.
    save_guest_context(registers, unsafe { &mut *context });
}

fn emulate_rdmsr(registers: &mut ExitRegisters, context: &VcpuContext) {
    let msr = registers.rcx as u32;
    let value = match msr {
        IA32_SYSENTER_CS => read_vmcs(VMCS_GUEST_SYSENTER_CS),
        IA32_SYSENTER_ESP => read_vmcs(VMCS_GUEST_SYSENTER_ESP),
        IA32_SYSENTER_EIP => read_vmcs(VMCS_GUEST_SYSENTER_EIP),
        IA32_EFER => read_vmcs(VMCS_GUEST_EFER),
        IA32_FS_BASE => read_vmcs(VMCS_GUEST_FS_BASE),
        IA32_GS_BASE => read_vmcs(VMCS_GUEST_GS_BASE),
        IA32_APIC_BASE => 0,
        0x10 => read_msr(0x10),
        IA32_PAT => read_vmcs(VMCS_GUEST_PAT),
        IA32_STAR => context.msrs.star,
        IA32_LSTAR => context.msrs.lstar,
        IA32_CSTAR => context.msrs.cstar,
        IA32_SFMASK => context.msrs.sfmask,
        IA32_KERNEL_GS_BASE => context.msrs.kernel_gs_base,
        IA32_TSC_AUX => context.msrs.tsc_aux,
        _ => 0,
    };
    registers.rax = value as u32 as u64;
    registers.rdx = (value >> 32) as u32 as u64;
}

fn emulate_wrmsr(registers: &ExitRegisters, context: &mut VcpuContext) {
    let msr = registers.rcx as u32;
    let value = (registers.rax as u32 as u64) | ((registers.rdx as u32 as u64) << 32);
    match msr {
        IA32_SYSENTER_CS => {
            write_vmcs(VMCS_GUEST_SYSENTER_CS, value);
        }
        IA32_SYSENTER_ESP => {
            write_vmcs(VMCS_GUEST_SYSENTER_ESP, value);
        }
        IA32_SYSENTER_EIP => {
            write_vmcs(VMCS_GUEST_SYSENTER_EIP, value);
        }
        IA32_EFER => {
            write_vmcs(VMCS_GUEST_EFER, value);
        }
        IA32_FS_BASE => {
            write_vmcs(VMCS_GUEST_FS_BASE, value);
        }
        IA32_GS_BASE => {
            write_vmcs(VMCS_GUEST_GS_BASE, value);
        }
        IA32_PAT => {
            write_vmcs(VMCS_GUEST_PAT, value);
        }
        IA32_STAR => context.msrs.star = value,
        IA32_LSTAR => context.msrs.lstar = value,
        IA32_CSTAR => context.msrs.cstar = value,
        IA32_SFMASK => context.msrs.sfmask = value,
        IA32_KERNEL_GS_BASE => context.msrs.kernel_gs_base = value,
        IA32_TSC_AUX => context.msrs.tsc_aux = value,
        _ => {}
    }
    update_guest_msr(msr, value);
}

fn handle_ept_violation() {
    let qualification = read_vmcs(VMCS_EXIT_QUALIFICATION);
    let address = read_vmcs(VMCS_GUEST_PHYSICAL_ADDRESS);
    let access = if qualification & (1 << 1) != 0 {
        super::GuestMemoryAccess::Write
    } else if qualification & (1 << 2) != 0 {
        super::GuestMemoryAccess::Execute
    } else {
        super::GuestMemoryAccess::Read
    };
    let mut frame = super::GuestSyncFrame::translation(super::GuestTranslationFault {
        address,
        access,
        during_page_walk: false,
    });
    if !crate::kernel::vm::handle_guest_sync(&mut frame) {
        crate::kernel::boot::fail("unhandled EPT violation", (address, qualification));
    }
}

#[unsafe(no_mangle)]
extern "C" fn x86_64_vmx_instruction_failure() -> ! {
    match vmread(VMCS_INSTRUCTION_ERROR) {
        Ok(error) => crate::kernel::boot::fail("VMX instruction", error),
        Err(error) => crate::kernel::boot::fail("VMX instruction without VMCS error", error),
    }
}

fn handle_external_interrupt() {
    let info = read_vmcs(VMCS_EXIT_INTERRUPT_INFO);
    if info & (1 << 31) != 0 {
        let vector = info as u32 & 0xff;
        crate::kernel::irq::interrupt::dispatch(hyper::hal::interrupt::InterruptId::new(vector));
        if vector == super::platform::TIMER_VECTOR {
            TIMER_PENDING.store(true, Ordering::Release);
        }
    }
}

fn prepare_guest_interrupt() {
    let mut primary = read_vmcs(VMCS_CTRL_PRIMARY) as u32;
    if !TIMER_PENDING.load(Ordering::Acquire) {
        primary &= !PRIMARY_INTERRUPT_WINDOW;
        write_vmcs(VMCS_CTRL_PRIMARY, u64::from(primary));
        return;
    }
    let flags = read_vmcs(VMCS_GUEST_RFLAGS);
    let blocked = read_vmcs(VMCS_GUEST_INTERRUPTIBILITY) != 0;
    if flags & (1 << 9) != 0 && !blocked {
        let vector = match crate::kernel::vm::legacy_timer_vector() {
            Ok(Some(vector)) => vector,
            Ok(None) => {
                TIMER_PENDING.store(false, Ordering::Release);
                primary &= !PRIMARY_INTERRUPT_WINDOW;
                write_vmcs(VMCS_CTRL_PRIMARY, u64::from(primary));
                return;
            }
            Err(error) => crate::kernel::boot::fail("legacy PIC timer routing", error),
        };
        write_vmcs(VMCS_CTRL_ENTRY_INTERRUPT, (1 << 31) | u64::from(vector));
        TIMER_PENDING.store(false, Ordering::Release);
        primary &= !PRIMARY_INTERRUPT_WINDOW;
    } else {
        primary |= PRIMARY_INTERRUPT_WINDOW;
    }
    write_vmcs(VMCS_CTRL_PRIMARY, u64::from(primary));
}

fn emulate_cpuid(registers: &mut ExitRegisters) {
    let requested_leaf = registers.rax as u32;
    if (0x4000_0000..0x5000_0000).contains(&requested_leaf) {
        registers.rax = 0x4000_0000;
        registers.rbx = 0x6570_7948;
        registers.rcx = 0x7079_4852;
        registers.rdx = 0x5648_5265;
        return;
    }
    let leaf = core::arch::x86_64::__cpuid_count(requested_leaf, registers.rcx as u32);
    registers.rax = u64::from(leaf.eax);
    registers.rbx = u64::from(leaf.ebx);
    registers.rcx = u64::from(leaf.ecx);
    registers.rdx = u64::from(leaf.edx);
    if requested_leaf == 1 {
        // Present a single logical processor and do not expose facilities whose
        // architectural state is not part of the initial VM service contract.
        registers.rbx = (registers.rbx & !0xffff_0000) | (1 << 16);
        registers.rcx &= !((1 << 5)
            | (1 << 3)
            | (1 << 12)
            | (1 << 21)
            | (1 << 24)
            | (1 << 26)
            | (1 << 27)
            | (1 << 28)
            | (1 << 29));
        registers.rcx |= 1 << 31;
        registers.rdx &= !((1 << 7) | (1 << 9) | (1 << 12) | (1 << 14) | (1 << 22) | (1 << 28));
    } else if requested_leaf == 4 {
        registers.rax &= !(0x3f << 26);
    } else if requested_leaf == 0x0b || requested_leaf == 0x1f {
        registers.rax = 0;
        registers.rbx = 0;
        registers.rcx = 0;
        registers.rdx = 0;
    } else if requested_leaf == 7 {
        registers.rbx &= !((1 << 5)
            | (1 << 16)
            | (1 << 17)
            | (1 << 21)
            | (1 << 26)
            | (1 << 27)
            | (1 << 28)
            | (1 << 30)
            | (1 << 31));
        registers.rcx = 0;
        registers.rdx = 0;
    } else if requested_leaf == 0x8000_0001 {
        registers.rcx &= !(1 << 2);
        registers.rdx &= !(1 << 27);
    } else if requested_leaf == 0x8000_0008 {
        registers.rcx &= !0xff;
    }
}

fn emulate_io(registers: &mut ExitRegisters) {
    let qualification = read_vmcs(VMCS_EXIT_QUALIFICATION);
    let size = match qualification & 7 {
        0 => 1,
        1 => 2,
        3 => 4,
        _ => crate::kernel::boot::fail("invalid VMX I/O access size", qualification),
    };
    let input = qualification & (1 << 3) != 0;
    let port = (qualification >> 16) as u16;
    if qualification & ((1 << 4) | (1 << 5)) != 0 {
        crate::kernel::boot::fail("unsupported VMX string I/O", qualification);
    }
    let result = match crate::kernel::vm::handle_port_io(port, size, !input, registers.rax as u32) {
        Ok(result) => result,
        Err(error) => crate::kernel::boot::fail("legacy PC port I/O", error),
    };
    if let Some(value) = result {
        let mask = match size {
            1 => 0xff,
            2 => 0xffff,
            _ => u32::MAX,
        };
        if size == 4 {
            registers.rax = u64::from(value);
        } else {
            registers.rax = (registers.rax & !u64::from(mask)) | u64::from(value & mask);
        }
    }
}

fn advance_rip() {
    let rip = read_vmcs(VMCS_GUEST_RIP);
    let length = read_vmcs(VMCS_EXIT_INSTRUCTION_LENGTH);
    write_vmcs(VMCS_GUEST_RIP, rip.wrapping_add(length));
}

fn read_vmcs(field: u64) -> u64 {
    match vmread(field) {
        Ok(value) => value,
        Err(error) => crate::kernel::boot::fail("VMCS read", (field, error)),
    }
}

fn write_vmcs(field: u64, value: u64) {
    if let Err(error) = vmwrite(field, value) {
        crate::kernel::boot::fail("VMCS write", (field, error));
    }
}

fn adjust_control(desired: u32, msr: u32) -> Result<u32, Error> {
    let capability = read_msr(msr);
    let required = capability as u32;
    let allowed = (capability >> 32) as u32;
    let value = (desired | required) & allowed;
    ((value & desired) == desired)
        .then_some(value)
        .ok_or(Error::InvalidControl)
}

fn control_msr(legacy: u32, true_controls: u32) -> u32 {
    if read_msr(IA32_VMX_BASIC) & (1 << 55) != 0 {
        true_controls
    } else {
        legacy
    }
}

fn descriptor_base(gdt: u64, selector: u16) -> Result<u64, Error> {
    let address = gdt
        .checked_add(u64::from(selector & !7))
        .ok_or(Error::InvalidAddress)? as *const u64;
    let low = unsafe { read_unaligned(address) };
    let high = unsafe { read_unaligned(address.add(1)) };
    Ok(((low >> 16) & 0xffff)
        | (((low >> 32) & 0xff) << 16)
        | (((low >> 56) & 0xff) << 24)
        | ((high & 0xffff_ffff) << 32))
}

#[repr(C, align(16))]
struct InveptDescriptor {
    eptp: u64,
    reserved: u64,
}

pub(super) fn invalidate_ept(root: u64) -> Result<(), Error> {
    let descriptor = InveptDescriptor {
        eptp: root | 6 | (3 << 3),
        reserved: 0,
    };
    let failed: u8;
    unsafe {
        asm!(
            "invept {kind}, [{descriptor}]",
            "setna {failed}",
            kind = in(reg) 1_u64,
            descriptor = in(reg) &descriptor,
            failed = out(reg_byte) failed,
            options(nostack)
        )
    };
    (failed == 0).then_some(()).ok_or(Error::VmxInstruction)
}

fn kernel_physical(virtual_address: usize) -> Result<u64, Error> {
    crate::kernel::boot::with_boot_state(|state| {
        (virtual_address as u64)
            .checked_sub(state.memory.kernel_base())
            .and_then(|offset| state.image_physical_start.checked_add(offset))
    })
    .ok_or(Error::InvalidAddress)
}

fn vmxon(address: u64) -> Result<(), Error> {
    let failed: u8;
    unsafe {
        asm!("vmxon [{}]", "setna {}", in(reg) &address, out(reg_byte) failed, options(nostack))
    };
    vmx_status(failed)
}

fn vmclear(address: u64) -> Result<(), Error> {
    let failed: u8;
    unsafe {
        asm!("vmclear [{}]", "setna {}", in(reg) &address, out(reg_byte) failed, options(nostack))
    };
    vmx_status(failed)
}

fn vmptrld(address: u64) -> Result<(), Error> {
    let failed: u8;
    unsafe {
        asm!("vmptrld [{}]", "setna {}", in(reg) &address, out(reg_byte) failed, options(nostack))
    };
    vmx_status(failed)
}

fn vmx_status(failed: u8) -> Result<(), Error> {
    (failed == 0).then_some(()).ok_or(Error::VmxInstruction)
}

fn vmwrite(field: u64, value: u64) -> Result<(), Error> {
    let failed: u8;
    unsafe {
        asm!("vmwrite {field}, {value}", "setna {failed}", value = in(reg) value, field = in(reg) field, failed = out(reg_byte) failed, options(nostack))
    };
    (failed == 0).then_some(()).ok_or(Error::VmxInstruction)
}

fn vmread(field: u64) -> Result<u64, Error> {
    let value: u64;
    let failed: u8;
    unsafe {
        asm!("vmread {value}, {field}", "setna {failed}", field = in(reg) field, value = out(reg) value, failed = out(reg_byte) failed, options(nostack))
    };
    (failed == 0).then_some(value).ok_or(Error::VmxInstruction)
}

fn read_msr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;
    unsafe {
        asm!("rdmsr", in("ecx") msr, out("eax") low, out("edx") high, options(nomem, nostack))
    };
    (u64::from(high) << 32) | u64::from(low)
}

fn write_msr(msr: u32, value: u64) {
    unsafe {
        asm!("wrmsr", in("ecx") msr, in("eax") value as u32, in("edx") (value >> 32) as u32, options(nostack))
    };
}

macro_rules! read_register {
    ($name:ident, $instruction:literal, $type:ty) => {
        fn $name() -> $type {
            let value: $type;
            unsafe { asm!($instruction, out(reg) value, options(nomem, nostack)) };
            value
        }
    };
}
read_register!(read_cr0, "mov {}, cr0", u64);
read_register!(read_cr3, "mov {}, cr3", u64);
read_register!(read_cr4, "mov {}, cr4", u64);
read_register!(read_cs, "mov {:x}, cs", u16);
read_register!(read_ss, "mov {:x}, ss", u16);
read_register!(read_ds, "mov {:x}, ds", u16);
read_register!(read_es, "mov {:x}, es", u16);
read_register!(read_fs, "mov {:x}, fs", u16);
read_register!(read_gs, "mov {:x}, gs", u16);
read_register!(read_tr, "str {:x}", u16);

fn write_cr0(value: u64) {
    unsafe { asm!("mov cr0, {}", in(reg) value, options(nostack)) };
}
fn write_cr4(value: u64) {
    unsafe { asm!("mov cr4, {}", in(reg) value, options(nostack)) };
}
