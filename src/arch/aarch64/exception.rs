use core::arch::asm;
use core::mem::{offset_of, size_of};
use core::ptr::addr_of;

use hyper::hal::exception::{ExceptionKind, ExceptionOrigin, ExceptionReport};
use hyper::sync::atomic::{AtomicU64, Ordering};

use super::registers;

const ESR_EXCEPTION_CLASS_SHIFT: u64 = 26;
const ESR_EXCEPTION_CLASS_MASK: u64 = 0x3f;
const ESR_EXCEPTION_CLASS_BRK64: u64 = 0x3c;
const VECTOR_CURRENT_SP0_SYNCHRONOUS: u64 = 0;
const VECTOR_CURRENT_SPX_SYNCHRONOUS: u64 = 4;
const VECTOR_TEST_IMMEDIATE: u64 = 0x4859;
const NO_VECTOR_TEST: u64 = u64::MAX;

static VECTOR_TEST_EXPECTED: AtomicU64 = AtomicU64::new(NO_VECTOR_TEST);

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
}

pub fn runtime_vector_address() -> u64 {
    addr_of!(aarch64_runtime_vectors) as u64
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
    VECTOR_TEST_EXPECTED.store(VECTOR_CURRENT_SPX_SYNCHRONOUS, Ordering::Release);
    // SAFETY: The dispatcher recognizes this private BRK immediate, advances
    // ELR past the instruction, and restores the complete interrupted state.
    unsafe { asm!("brk #0x4859", options(nostack)) };
    if VECTOR_TEST_EXPECTED.load(Ordering::Acquire) != NO_VECTOR_TEST {
        return Err(ValidationError::ExceptionDidNotReturn);
    }

    VECTOR_TEST_EXPECTED.store(VECTOR_CURRENT_SP0_SYNCHRONOUS, Ordering::Release);
    let _saved_sp_el0: u64;
    // SAFETY: No stack access occurs while SPSel selects the deliberately
    // unusable SP_EL0 value. Slot zero must select SP_EL2 before constructing
    // its frame. ERET restores EL2t, after which SPSel is immediately reset.
    unsafe {
        asm!(
            "mrs {saved}, sp_el0",
            "mov x9, #0x1000",
            "msr sp_el0, x9",
            "msr spsel, #0",
            "brk #0x4859",
            "msr spsel, #1",
            "msr sp_el0, {saved}",
            saved = lateout(reg) _saved_sp_el0,
            out("x9") _,
            options(nostack)
        );
    }
    if VECTOR_TEST_EXPECTED.load(Ordering::Acquire) != NO_VECTOR_TEST {
        return Err(ValidationError::ExceptionDidNotReturn);
    }
    Ok(())
}

#[unsafe(no_mangle)]
extern "C" fn aarch64_exception_dispatch(frame: &mut ExceptionFrame) {
    let exception_class = (frame.esr >> ESR_EXCEPTION_CLASS_SHIFT) & ESR_EXCEPTION_CLASS_MASK;
    if matches!(
        frame.vector,
        VECTOR_CURRENT_SP0_SYNCHRONOUS | VECTOR_CURRENT_SPX_SYNCHRONOUS
    ) && exception_class == ESR_EXCEPTION_CLASS_BRK64
        && frame.esr & 0xffff == VECTOR_TEST_IMMEDIATE
        && VECTOR_TEST_EXPECTED
            .compare_exchange(
                frame.vector,
                NO_VECTOR_TEST,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    {
        frame.elr = frame.elr.wrapping_add(4);
        return;
    }

    let Some((kind, origin)) = decode_vector(frame.vector) else {
        crate::kernel::exception::fatal_invalid_vector(
            frame.vector,
            frame.esr,
            frame.elr,
            frame.far,
            frame.spsr,
        );
    };
    if kind == ExceptionKind::Irq {
        crate::kernel::interrupt::dispatch();
        return;
    }
    if kind == ExceptionKind::Synchronous && origin == ExceptionOrigin::LowerAarch64 {
        let mut guest_frame = super::GuestSyncFrame::new(
            &mut frame.general,
            &mut frame.elr,
            &mut frame.spsr,
            frame.esr,
            frame.far,
            guest_physical_address(frame.far),
        );
        if crate::kernel::vm::handle_guest_sync(&mut guest_frame) {
            return;
        }
    }

    let stack_pointer = interrupted_stack_pointer(frame, origin);
    let (architecture_class, description) = if kind == ExceptionKind::Fiq {
        (0, "FIQ exception")
    } else {
        (exception_class as u8, syndrome_description(exception_class))
    };
    crate::kernel::exception::fatal(ExceptionReport {
        origin,
        kind,
        architecture_class,
        description,
        syndrome: frame.esr,
        instruction_pointer: frame.elr,
        fault_address_register: frame.far,
        status: frame.spsr,
        stack_pointer,
    })
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
    ((hpfar & 0x0000_00ff_ffff_fff0) << 8) | (fault_address & 0xfff)
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
        ExceptionOrigin::LowerAarch64 if frame.spsr & 0xf != 0x5 => frame.sp_el0,
        ExceptionOrigin::LowerAarch32 if matches!(frame.spsr & 0x1f, 0x10 | 0x1f) => frame.sp_el0,
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
        0x00 => "unknown reason",
        0x01 => "trapped WFI or WFE",
        0x03 => "trapped MCR or MRC",
        0x04 => "trapped MCRR or MRRC",
        0x05 => "trapped MCR or MRC access",
        0x06 => "trapped LDC or STC",
        0x07 => "trapped SIMD or floating-point access",
        0x0c => "trapped MRRC access",
        0x0e => "illegal execution state",
        0x11 => "supervisor call from AArch32",
        0x12 => "hypervisor call from AArch32",
        0x13 => "monitor call from AArch32",
        0x15 => "supervisor call from AArch64",
        0x16 => "hypervisor call from AArch64",
        0x17 => "monitor call from AArch64",
        0x18 => "trapped system register access",
        0x19 => "trapped SVE access",
        0x1c => "pointer authentication failure",
        0x20 => "instruction abort from lower EL",
        0x21 => "instruction abort at current EL",
        0x22 => "PC alignment fault",
        0x24 => "data abort from lower EL",
        0x25 => "data abort at current EL",
        0x26 => "SP alignment fault",
        0x28 => "floating-point exception from AArch32",
        0x2c => "floating-point exception from AArch64",
        0x2f => "SError interrupt",
        0x30 | 0x31 => "hardware breakpoint",
        0x32 | 0x33 => "software step",
        0x34 | 0x35 => "watchpoint",
        0x38 => "BKPT instruction",
        0x3a => "vector catch",
        0x3c => "BRK instruction",
        _ => "reserved or implementation-defined exception class",
    }
}
