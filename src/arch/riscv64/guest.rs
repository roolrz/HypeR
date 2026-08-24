// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! RISC-V guest-exit decoding and the initial supervisor binary interface.

use core::arch::asm;
use hyper::vm::exit::{GuestMemoryFault, GuestPhysicalAddress, MemoryAccess};

use super::VcpuContext;

const CAUSE_VIRTUAL_SUPERVISOR_ECALL: u64 = 10;
const CAUSE_VIRTUAL_INSTRUCTION: u64 = 22;
const CAUSE_INSTRUCTION_GUEST_PAGE_FAULT: u64 = 20;
const CAUSE_LOAD_GUEST_PAGE_FAULT: u64 = 21;
const CAUSE_STORE_GUEST_PAGE_FAULT: u64 = 23;
const HVIP_VSTIP: usize = 1 << 6;

const SBI_EXT_BASE: u64 = 0x10;
const SBI_EXT_TIME: u64 = 0x5449_4d45;
const SBI_EXT_IPI: u64 = 0x0073_5049;
const SBI_EXT_RFENCE: u64 = 0x5246_4e43;
const SBI_SUCCESS: u64 = 0;
const SBI_NOT_SUPPORTED: u64 = (-2isize) as u64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GuestSyncAction {
    Resume,
    #[allow(dead_code)]
    Injected,
    #[allow(dead_code)]
    SoftwareInterrupt(u64),
    Unhandled,
}

pub(crate) struct GuestSyncFrame<'a> {
    trap: &'a mut super::exception::TrapFrame,
}

impl<'a> GuestSyncFrame<'a> {
    pub(crate) fn new(trap: &'a mut super::exception::TrapFrame) -> Self {
        Self { trap }
    }

    pub(crate) fn guest_memory_fault(&self) -> Option<GuestMemoryFault> {
        let guest_access = match self.trap.scause {
            CAUSE_INSTRUCTION_GUEST_PAGE_FAULT => MemoryAccess::Execute,
            CAUSE_LOAD_GUEST_PAGE_FAULT => MemoryAccess::Read,
            CAUSE_STORE_GUEST_PAGE_FAULT => MemoryAccess::Write,
            _ => return None,
        };
        // HTVAL is permitted to be zero when the implementation does not
        // provide the faulting GPA. Such a fault cannot be resolved safely by
        // demand paging because zero is also a valid encoded GPA.
        if self.trap.htval == 0 {
            return None;
        }
        let (access, during_guest_page_walk) = match guest_page_walk_access(self.trap.htinst) {
            Some(access) => (access, true),
            None => (guest_access, false),
        };
        Some(GuestMemoryFault::new(
            GuestPhysicalAddress::new(guest_physical_fault_address(
                self.trap.htval,
                self.trap.stval,
                during_guest_page_walk,
            )),
            access,
            during_guest_page_walk,
        ))
    }
}

fn guest_page_walk_access(instruction: u64) -> Option<MemoryAccess> {
    // The privileged ISA requires one of these values when an implicit
    // VS-stage page-table access faults and HTVAL is nonzero. Other nonzero
    // HTINST values encode the explicit instruction which faulted.
    // https://docs.riscv.org/reference/isa/priv/hypervisor.html
    match instruction {
        0x2000 | 0x3000 => Some(MemoryAccess::Read),
        0x2020 | 0x3020 => Some(MemoryAccess::Write),
        _ => None,
    }
}

fn guest_physical_fault_address(htval: u64, stval: u64, during_guest_page_walk: bool) -> u64 {
    let low_bits = if during_guest_page_walk { 0 } else { stval & 3 };
    (htval << 2) | low_bits
}

pub(crate) fn handle_guest_sync(
    context: &mut VcpuContext,
    vcpu_id: u32,
    frame: &mut GuestSyncFrame<'_>,
) -> GuestSyncAction {
    if frame.trap.scause == CAUSE_VIRTUAL_INSTRUCTION {
        return if emulate_virtual_instruction(context, frame) {
            frame.trap.sepc = frame.trap.sepc.wrapping_add(4);
            GuestSyncAction::Resume
        } else {
            GuestSyncAction::Unhandled
        };
    }
    if frame.trap.scause != CAUSE_VIRTUAL_SUPERVISOR_ECALL {
        return GuestSyncAction::Unhandled;
    }
    emulate_sbi(vcpu_id, frame);
    frame.trap.sepc = frame.trap.sepc.wrapping_add(4);
    GuestSyncAction::Resume
}

fn emulate_virtual_instruction(context: &mut VcpuContext, frame: &mut GuestSyncFrame<'_>) -> bool {
    const SYSTEM_OPCODE: u32 = 0x73;
    const WFI: u32 = 0x1050_0073;
    const CSR_SIE: u32 = 0x104;
    const CSR_SIP: u32 = 0x144;
    const CSR_SCOUNTEREN: u32 = 0x106;
    const CSR_SENVCFG: u32 = 0x10a;

    // For a virtual-instruction exception, stval contains the trapped
    // instruction bits. htinst is reserved for transformed instructions on
    // guest-page faults and must not be preferred here.
    let instruction = frame.trap.stval as u32;
    if instruction == WFI {
        return true;
    }
    if instruction & 0x7f != SYSTEM_OPCODE {
        return false;
    }
    let function = (instruction >> 12) & 7;
    let source_register = ((instruction >> 15) & 0x1f) as usize;
    let destination_register = ((instruction >> 7) & 0x1f) as usize;
    let csr = instruction >> 20;
    let old = match csr {
        CSR_SIE => read_vsie(),
        CSR_SIP => read_vsip(),
        CSR_SCOUNTEREN => context.scounteren,
        CSR_SENVCFG => context.senvcfg,
        _ => return false,
    };
    let source = if function >= 5 {
        source_register as u64
    } else {
        frame.trap.general[source_register]
    };
    let new_value = match function {
        1 | 5 => Some(source),
        2 | 6 if source != 0 => Some(old | source),
        3 | 7 if source != 0 => Some(old & !source),
        2 | 3 | 6 | 7 => None,
        _ => return false,
    };
    if let Some(value) = new_value {
        match csr {
            CSR_SIE => write_vsie(value),
            CSR_SIP => write_vsip(value),
            CSR_SCOUNTEREN => {
                context.scounteren = value;
                write_hcounteren(value);
            }
            CSR_SENVCFG => context.senvcfg = value,
            _ => return false,
        }
    }
    if destination_register != 0 {
        frame.trap.general[destination_register] = old;
    }
    true
}

fn read_vsie() -> u64 {
    let value: u64;
    // SAFETY: VSIE is accessible in HS mode while the guest context is active.
    unsafe { asm!("csrr {value}, vsie", value = out(reg) value, options(nomem, nostack)) };
    value
}

fn write_vsie(value: u64) {
    // SAFETY: VSIE is writable in HS mode while the guest context is active.
    unsafe { asm!("csrw vsie, {value}", value = in(reg) value, options(nostack)) };
}

fn read_vsip() -> u64 {
    let value: u64;
    // SAFETY: VSIP is accessible in HS mode while the guest context is active.
    unsafe { asm!("csrr {value}, vsip", value = out(reg) value, options(nomem, nostack)) };
    value
}

fn write_vsip(value: u64) {
    // SAFETY: VSIP is writable in HS mode while the guest context is active.
    unsafe { asm!("csrw vsip, {value}", value = in(reg) value, options(nostack)) };
}

fn write_hcounteren(value: u64) {
    // SAFETY: HCOUNTEREN is writable in HS mode.
    unsafe { asm!("csrw hcounteren, {value}", value = in(reg) value, options(nostack)) };
}

fn emulate_sbi(vcpu_id: u32, frame: &mut GuestSyncFrame<'_>) {
    let extension = frame.trap.general[17];
    let function = frame.trap.general[16];
    let arguments = [
        frame.trap.general[10],
        frame.trap.general[11],
        frame.trap.general[12],
        frame.trap.general[13],
        frame.trap.general[14],
        frame.trap.general[15],
    ];

    // Legacy SBI v0.1 calls return their value directly in a0.
    match extension {
        0 => {
            set_timer(vcpu_id, arguments[0]);
            frame.trap.general[10] = SBI_SUCCESS;
            return;
        }
        1 => {
            crate::kernel::log::console::write_raw_byte(arguments[0] as u8);
            frame.trap.general[10] = SBI_SUCCESS;
            return;
        }
        2 => {
            frame.trap.general[10] = u64::MAX;
            return;
        }
        3..=7 => {
            frame.trap.general[10] = SBI_SUCCESS;
            return;
        }
        _ => {}
    }

    let (error, value) = match (extension, function) {
        (SBI_EXT_BASE, 0) => (SBI_SUCCESS, 0x0000_0003),
        (SBI_EXT_BASE, 1 | 2 | 4..=6) => (SBI_SUCCESS, 0),
        (SBI_EXT_BASE, 3) => {
            let available = matches!(arguments[0], SBI_EXT_TIME | SBI_EXT_IPI | SBI_EXT_RFENCE);
            (SBI_SUCCESS, u64::from(available))
        }
        (SBI_EXT_TIME, 0) => {
            set_timer(vcpu_id, arguments[0]);
            (SBI_SUCCESS, 0)
        }
        (SBI_EXT_IPI | SBI_EXT_RFENCE, _) => (SBI_SUCCESS, 0),
        _ => (SBI_NOT_SUPPORTED, 0),
    };
    frame.trap.general[10] = error;
    frame.trap.general[11] = value;
}

fn set_timer(vcpu_id: u32, deadline: u64) {
    let _ = vcpu_id;
    // SAFETY: These hypervisor CSRs are writable in HS mode with SSTC enabled.
    unsafe {
        asm!(
            "csrs henvcfg, {stce}",
            "csrw 0x24d, {deadline}",
            "csrc hvip, {mask}",
            stce = in(reg) 1usize << 63,
            deadline = in(reg) deadline,
            mask = in(reg) HVIP_VSTIP,
            options(nostack)
        )
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationError {
    InvalidGuestMemoryFaultDecoder,
}

pub(super) fn validate() -> Result<(), ValidationError> {
    if guest_page_walk_access(0x3000) != Some(MemoryAccess::Read)
        || guest_page_walk_access(0x3020) != Some(MemoryAccess::Write)
        || guest_page_walk_access(0x0000_20c3).is_some()
        || guest_physical_fault_address(0x1234, 3, true) != 0x48d0
        || guest_physical_fault_address(0x1234, 3, false) != 0x48d3
    {
        return Err(ValidationError::InvalidGuestMemoryFaultDecoder);
    }
    Ok(())
}
