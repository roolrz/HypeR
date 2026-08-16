//! AArch64 GICv3 virtualization system-register backend.

use core::arch::asm;

use hyper::vm::interrupt::gicv3::{decode_list_register, encode_list_register};
use hyper::vm::interrupt::{InterruptGroup, ListEntry, ListState, VirtualInterruptId};

use super::registers;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    IncompatibleCpuInterface,
    InvalidListRegisterCount,
    InvalidTypeRegister,
    InvalidVirtualInterrupt,
    StateMismatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Capabilities {
    pub list_registers: u8,
    pub priority_bits: u8,
    pub preemption_bits: u8,
    pub interrupt_id_bits: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaintenanceState {
    pub status: u64,
    pub eoi_list_registers: u64,
    pub empty_list_registers: u64,
}

/// Per-vCPU hardware state retained while the vCPU is not running.
pub struct CpuContext {
    control: u64,
    virtual_machine_control: u64,
    active_priorities_group0: [u64; 4],
    active_priorities_group1: [u64; 4],
    list_registers: [Option<ListEntry>; registers::ICH_MAX_LIST_REGISTERS],
    list_register_count: u8,
    active_priority_register_count: u8,
}

impl CpuContext {
    pub const fn empty() -> Self {
        Self {
            control: registers::ICH_HCR_ENABLE,
            virtual_machine_control: registers::ICH_VMCR_ENABLE_GROUP1
                | registers::ICH_VMCR_PRIORITY_MASK_ALLOW_ALL,
            active_priorities_group0: [0; 4],
            active_priorities_group1: [0; 4],
            list_registers: [None; registers::ICH_MAX_LIST_REGISTERS],
            list_register_count: 0,
            active_priority_register_count: 0,
        }
    }

    pub fn slots(&self) -> &[Option<ListEntry>] {
        &self.list_registers[..usize::from(self.list_register_count)]
    }

    pub fn slots_mut(&mut self) -> &mut [Option<ListEntry>] {
        &mut self.list_registers[..usize::from(self.list_register_count)]
    }

    pub const fn list_register_count(&self) -> u8 {
        self.list_register_count
    }
}

impl Default for CpuContext {
    fn default() -> Self {
        Self::empty()
    }
}

pub fn capabilities() -> Result<Capabilities, Error> {
    let value: u64;
    // SAFETY: ICH_VTR_EL2 is a read-only capability register available at EL2.
    unsafe {
        asm!(
            "mrs {value}, ICH_VTR_EL2",
            value = out(reg) value,
            options(nomem, nostack, preserves_flags)
        );
    }
    let list_registers = ((value & registers::ICH_VTR_LIST_REGISTERS_MASK) + 1) as u8;
    let priority_bits = (((value >> registers::ICH_VTR_PRIORITY_BITS_SHIFT)
        & registers::ICH_VTR_BITS_MASK)
        + 1) as u8;
    let preemption_bits = (((value >> registers::ICH_VTR_PREEMPTION_BITS_SHIFT)
        & registers::ICH_VTR_BITS_MASK)
        + 1) as u8;
    let interrupt_id_bits =
        match (value >> registers::ICH_VTR_ID_BITS_SHIFT) & registers::ICH_VTR_ID_BITS_MASK {
            0 => 16,
            1 => 24,
            _ => return Err(Error::InvalidTypeRegister),
        };
    if list_registers == 0
        || usize::from(list_registers) > registers::ICH_MAX_LIST_REGISTERS
        || !(5..=8).contains(&priority_bits)
        || !(5..=7).contains(&preemption_bits)
        || preemption_bits > priority_bits
    {
        return Err(Error::InvalidTypeRegister);
    }
    Ok(Capabilities {
        list_registers,
        priority_bits,
        preemption_bits,
        interrupt_id_bits,
    })
}

pub fn initialize_context(context: &mut CpuContext) -> Result<Capabilities, Error> {
    let capabilities = capabilities()?;
    context.list_register_count = capabilities.list_registers;
    context.active_priority_register_count = active_priority_register_count(capabilities);
    Ok(capabilities)
}

/// Exercises every implemented APR and LR bank through a save/restore cycle.
pub fn validate_context_switch() -> Result<Capabilities, Error> {
    let mut context = CpuContext::empty();
    let capabilities = initialize_context(&mut context)?;
    let probe = ListEntry {
        interrupt: VirtualInterruptId::new(31).ok_or(Error::InvalidVirtualInterrupt)?,
        priority: 0xa0,
        group: InterruptGroup::Group1,
        state: ListState::Pending,
        request_eoi_maintenance: false,
    };
    let Some(first) = context.slots_mut().first_mut() else {
        return Err(Error::InvalidListRegisterCount);
    };
    *first = Some(probe);
    // SAFETY: Boot validation owns the local virtual interface and no vCPU can
    // be active before the kernel vGIC subsystem is installed.
    unsafe {
        activate(&context)?;
        deactivate(&mut context)?;
    }
    if context.slots().first().copied() != Some(Some(probe)) {
        return Err(Error::StateMismatch);
    }

    context.slots_mut().fill(None);
    // SAFETY: This final load clears every implemented LR. Delivery is
    // disabled immediately, leaving no validation state resident in hardware.
    unsafe { activate(&context)? };
    disable();
    Ok(capabilities)
}

/// Installs a complete vCPU interface state and enables virtual delivery.
///
/// # Safety
///
/// The context must be exclusively owned by the vCPU scheduled on this CPU.
pub unsafe fn activate(context: &CpuContext) -> Result<(), Error> {
    let capabilities = capabilities()?;
    let implemented = usize::from(capabilities.list_registers);
    let count = usize::from(context.list_register_count);
    let active_priority_count = active_priority_register_count(capabilities);
    if count != implemented
        || context.active_priority_register_count != active_priority_count
        || count > registers::ICH_MAX_LIST_REGISTERS
    {
        return Err(Error::IncompatibleCpuInterface);
    }
    // SAFETY: Guest delivery is disabled while the complete banked virtual CPU
    // interface state is installed. HCR is enabled only after all state writes.
    unsafe {
        asm!("msr ICH_HCR_EL2, xzr", "isb", options(nomem, nostack));
        write_priority_state(context, active_priority_count);
        asm!(
            "msr ICH_VMCR_EL2, {value}",
            value = in(reg) context.virtual_machine_control,
            options(nomem, nostack)
        );
        for index in 0..implemented {
            let value = if index < count {
                encode_list_register(context.list_registers[index])
            } else {
                0
            };
            write_list_register(index, value);
        }
        asm!(
            "msr ICH_HCR_EL2, {value}",
            "isb",
            value = in(reg) context.control | registers::ICH_HCR_ENABLE,
            options(nomem, nostack)
        );
    }
    Ok(())
}

/// Disables guest delivery and snapshots the complete local virtual interface.
///
/// # Safety
///
/// `context` must describe the vCPU currently loaded on this CPU.
pub unsafe fn deactivate(context: &mut CpuContext) -> Result<(), Error> {
    let capabilities = capabilities()?;
    let implemented = usize::from(capabilities.list_registers);
    let count = usize::from(context.list_register_count);
    let active_priority_count = active_priority_register_count(capabilities);
    if count != implemented
        || context.active_priority_register_count != active_priority_count
        || count > registers::ICH_MAX_LIST_REGISTERS
    {
        return Err(Error::IncompatibleCpuInterface);
    }
    let control: u64;
    let virtual_machine_control: u64;
    // SAFETY: The caller has stopped guest execution. Disabling HCR prevents
    // further virtual delivery while the banked state is sampled.
    unsafe {
        asm!(
            "mrs {control}, ICH_HCR_EL2",
            "msr ICH_HCR_EL2, xzr",
            "isb",
            "mrs {vmcr}, ICH_VMCR_EL2",
            control = out(reg) control,
            vmcr = out(reg) virtual_machine_control,
            options(nomem, nostack)
        );
        context.control = control;
        context.virtual_machine_control = virtual_machine_control;
        read_priority_state(context, active_priority_count);
        for index in 0..count {
            context.list_registers[index] = decode_list_register(read_list_register(index))
                .map_err(|_| Error::InvalidVirtualInterrupt)?;
        }
        for slot in &mut context.list_registers[count..] {
            *slot = None;
        }
    }
    Ok(())
}

pub fn maintenance_state() -> MaintenanceState {
    let status: u64;
    let eoi: u64;
    let empty: u64;
    // SAFETY: These registers are read-only snapshots of the local virtual CPU
    // interface and have no memory side effects.
    unsafe {
        asm!(
            "mrs {status}, ICH_MISR_EL2",
            "mrs {eoi}, ICH_EISR_EL2",
            "mrs {empty}, ICH_ELRSR_EL2",
            status = out(reg) status,
            eoi = out(reg) eoi,
            empty = out(reg) empty,
            options(nomem, nostack, preserves_flags)
        );
    }
    MaintenanceState {
        status,
        eoi_list_registers: eoi,
        empty_list_registers: empty,
    }
}

/// Disables virtual interrupt delivery on the current CPU.
pub fn disable() {
    // SAFETY: Disabling ICH_HCR_EL2 is always valid at EL2 and is the fail-safe
    // response when maintenance arrives without an active vCPU owner.
    unsafe {
        asm!("msr ICH_HCR_EL2, xzr", "isb", options(nomem, nostack));
    }
}

const fn active_priority_register_count(capabilities: Capabilities) -> u8 {
    match capabilities.preemption_bits {
        5 => 1,
        6 => 2,
        _ => 4,
    }
}

unsafe fn write_priority_state(context: &CpuContext, count: u8) {
    // SAFETY: The caller disabled ICH_HCR_EL2 and owns the local interface.
    unsafe {
        match count {
            1 => asm!(
                "msr ICH_AP0R0_EL2, {ap0}",
                "msr ICH_AP1R0_EL2, {ap1}",
                ap0 = in(reg) context.active_priorities_group0[0],
                ap1 = in(reg) context.active_priorities_group1[0],
                options(nomem, nostack)
            ),
            2 => asm!(
                "msr ICH_AP0R0_EL2, {ap0_0}",
                "msr ICH_AP0R1_EL2, {ap0_1}",
                "msr ICH_AP1R0_EL2, {ap1_0}",
                "msr ICH_AP1R1_EL2, {ap1_1}",
                ap0_0 = in(reg) context.active_priorities_group0[0],
                ap0_1 = in(reg) context.active_priorities_group0[1],
                ap1_0 = in(reg) context.active_priorities_group1[0],
                ap1_1 = in(reg) context.active_priorities_group1[1],
                options(nomem, nostack)
            ),
            _ => asm!(
                "msr ICH_AP0R0_EL2, {ap0_0}",
                "msr ICH_AP0R1_EL2, {ap0_1}",
                "msr ICH_AP0R2_EL2, {ap0_2}",
                "msr ICH_AP0R3_EL2, {ap0_3}",
                "msr ICH_AP1R0_EL2, {ap1_0}",
                "msr ICH_AP1R1_EL2, {ap1_1}",
                "msr ICH_AP1R2_EL2, {ap1_2}",
                "msr ICH_AP1R3_EL2, {ap1_3}",
                ap0_0 = in(reg) context.active_priorities_group0[0],
                ap0_1 = in(reg) context.active_priorities_group0[1],
                ap0_2 = in(reg) context.active_priorities_group0[2],
                ap0_3 = in(reg) context.active_priorities_group0[3],
                ap1_0 = in(reg) context.active_priorities_group1[0],
                ap1_1 = in(reg) context.active_priorities_group1[1],
                ap1_2 = in(reg) context.active_priorities_group1[2],
                ap1_3 = in(reg) context.active_priorities_group1[3],
                options(nomem, nostack)
            ),
        }
    }
}

unsafe fn read_priority_state(context: &mut CpuContext, count: u8) {
    // SAFETY: The caller disabled ICH_HCR_EL2 and owns the local interface.
    unsafe {
        match count {
            1 => asm!(
                "mrs {ap0}, ICH_AP0R0_EL2",
                "mrs {ap1}, ICH_AP1R0_EL2",
                ap0 = out(reg) context.active_priorities_group0[0],
                ap1 = out(reg) context.active_priorities_group1[0],
                options(nomem, nostack)
            ),
            2 => asm!(
                "mrs {ap0_0}, ICH_AP0R0_EL2",
                "mrs {ap0_1}, ICH_AP0R1_EL2",
                "mrs {ap1_0}, ICH_AP1R0_EL2",
                "mrs {ap1_1}, ICH_AP1R1_EL2",
                ap0_0 = out(reg) context.active_priorities_group0[0],
                ap0_1 = out(reg) context.active_priorities_group0[1],
                ap1_0 = out(reg) context.active_priorities_group1[0],
                ap1_1 = out(reg) context.active_priorities_group1[1],
                options(nomem, nostack)
            ),
            _ => asm!(
                "mrs {ap0_0}, ICH_AP0R0_EL2",
                "mrs {ap0_1}, ICH_AP0R1_EL2",
                "mrs {ap0_2}, ICH_AP0R2_EL2",
                "mrs {ap0_3}, ICH_AP0R3_EL2",
                "mrs {ap1_0}, ICH_AP1R0_EL2",
                "mrs {ap1_1}, ICH_AP1R1_EL2",
                "mrs {ap1_2}, ICH_AP1R2_EL2",
                "mrs {ap1_3}, ICH_AP1R3_EL2",
                ap0_0 = out(reg) context.active_priorities_group0[0],
                ap0_1 = out(reg) context.active_priorities_group0[1],
                ap0_2 = out(reg) context.active_priorities_group0[2],
                ap0_3 = out(reg) context.active_priorities_group0[3],
                ap1_0 = out(reg) context.active_priorities_group1[0],
                ap1_1 = out(reg) context.active_priorities_group1[1],
                ap1_2 = out(reg) context.active_priorities_group1[2],
                ap1_3 = out(reg) context.active_priorities_group1[3],
                options(nomem, nostack)
            ),
        }
    }
}

macro_rules! read_lr {
    ($register:literal) => {{
        let value: u64;
        // SAFETY: The caller owns the local GIC virtualization interface.
        unsafe {
            asm!(
                concat!("mrs {value}, ", $register),
                value = out(reg) value,
                options(nomem, nostack, preserves_flags)
            );
        }
        value
    }};
}

macro_rules! write_lr {
    ($register:literal, $value:expr) => {{
        let value = $value;
        // SAFETY: The caller owns the local GIC virtualization interface.
        unsafe {
            asm!(
                concat!("msr ", $register, ", {value}"),
                value = in(reg) value,
                options(nomem, nostack, preserves_flags)
            );
        }
    }};
}

unsafe fn read_list_register(index: usize) -> u64 {
    match index {
        0 => read_lr!("ICH_LR0_EL2"),
        1 => read_lr!("ICH_LR1_EL2"),
        2 => read_lr!("ICH_LR2_EL2"),
        3 => read_lr!("ICH_LR3_EL2"),
        4 => read_lr!("ICH_LR4_EL2"),
        5 => read_lr!("ICH_LR5_EL2"),
        6 => read_lr!("ICH_LR6_EL2"),
        7 => read_lr!("ICH_LR7_EL2"),
        8 => read_lr!("ICH_LR8_EL2"),
        9 => read_lr!("ICH_LR9_EL2"),
        10 => read_lr!("ICH_LR10_EL2"),
        11 => read_lr!("ICH_LR11_EL2"),
        12 => read_lr!("ICH_LR12_EL2"),
        13 => read_lr!("ICH_LR13_EL2"),
        14 => read_lr!("ICH_LR14_EL2"),
        15 => read_lr!("ICH_LR15_EL2"),
        _ => 0,
    }
}

unsafe fn write_list_register(index: usize, value: u64) {
    match index {
        0 => write_lr!("ICH_LR0_EL2", value),
        1 => write_lr!("ICH_LR1_EL2", value),
        2 => write_lr!("ICH_LR2_EL2", value),
        3 => write_lr!("ICH_LR3_EL2", value),
        4 => write_lr!("ICH_LR4_EL2", value),
        5 => write_lr!("ICH_LR5_EL2", value),
        6 => write_lr!("ICH_LR6_EL2", value),
        7 => write_lr!("ICH_LR7_EL2", value),
        8 => write_lr!("ICH_LR8_EL2", value),
        9 => write_lr!("ICH_LR9_EL2", value),
        10 => write_lr!("ICH_LR10_EL2", value),
        11 => write_lr!("ICH_LR11_EL2", value),
        12 => write_lr!("ICH_LR12_EL2", value),
        13 => write_lr!("ICH_LR13_EL2", value),
        14 => write_lr!("ICH_LR14_EL2", value),
        15 => write_lr!("ICH_LR15_EL2", value),
        _ => {}
    }
}
