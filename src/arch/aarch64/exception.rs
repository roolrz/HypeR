use core::arch::asm;
use core::mem::{offset_of, size_of};
use core::ptr::addr_of;

use hyper::sync::atomic::{AtomicBool, Ordering};

use super::registers;

const ESR_EXCEPTION_CLASS_SHIFT: u64 = 26;
const ESR_EXCEPTION_CLASS_MASK: u64 = 0x3f;
const ESR_EXCEPTION_CLASS_BRK64: u64 = 0x3c;
const VECTOR_CURRENT_SPX_SYNCHRONOUS: u64 = 4;
const VECTOR_TEST_IMMEDIATE: u64 = 0x4859;

static VECTOR_TEST_EXPECTED: AtomicBool = AtomicBool::new(false);

#[repr(C, align(16))]
struct ExceptionFrame {
    general: [u64; 31],
    elr: u64,
    spsr: u64,
    esr: u64,
    far: u64,
    vector: u64,
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
    VECTOR_TEST_EXPECTED.store(true, Ordering::Release);
    // SAFETY: The dispatcher recognizes this private BRK immediate, advances
    // ELR past the instruction, and restores the complete interrupted state.
    unsafe { asm!("brk #0x4859", options(nomem, nostack)) };
    if VECTOR_TEST_EXPECTED.swap(false, Ordering::AcqRel) {
        Err(ValidationError::ExceptionDidNotReturn)
    } else {
        Ok(())
    }
}

#[unsafe(no_mangle)]
extern "C" fn aarch64_exception_dispatch(frame: &mut ExceptionFrame) {
    let exception_class = (frame.esr >> ESR_EXCEPTION_CLASS_SHIFT) & ESR_EXCEPTION_CLASS_MASK;
    if frame.vector == VECTOR_CURRENT_SPX_SYNCHRONOUS
        && exception_class == ESR_EXCEPTION_CLASS_BRK64
        && frame.esr & 0xffff == VECTOR_TEST_IMMEDIATE
        && VECTOR_TEST_EXPECTED.swap(false, Ordering::AcqRel)
    {
        frame.elr = frame.elr.wrapping_add(4);
        return;
    }
    match frame.vector {
        1 | 5 | 9 | 13 => crate::kernel::interrupt::dispatch(),
        vector => {
            crate::kernel::exception::fatal(vector, frame.esr, frame.elr, frame.far, frame.spsr)
        }
    }
}
