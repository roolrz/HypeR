// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! AMD SVM MSR, VMCB, intercept, and exit-code definitions.

pub(super) const MSR_VM_CR: u32 = 0xc001_0114;
pub(super) const MSR_VM_HSAVE_PA: u32 = 0xc001_0117;
pub(super) const EFER_SVME: u64 = 1 << 12;
pub(super) const VM_CR_SVM_DISABLE: u64 = 1 << 4;

pub(super) const VMCB_INTERCEPT_WORD3: usize = 0x0c;
pub(super) const VMCB_INTERCEPT_WORD4: usize = 0x10;
pub(super) const VMCB_IOPM_BASE: usize = 0x40;
pub(super) const VMCB_MSRPM_BASE: usize = 0x48;
pub(super) const VMCB_ASID: usize = 0x58;
pub(super) const VMCB_TLB_CONTROL: usize = 0x5c;
pub(super) const VMCB_INT_CONTROL: usize = 0x60;
pub(super) const VMCB_INT_VECTOR: usize = 0x64;
pub(super) const VMCB_EXIT_CODE: usize = 0x70;
pub(super) const VMCB_EXIT_INFO1: usize = 0x78;
pub(super) const VMCB_EXIT_INFO2: usize = 0x80;
pub(super) const VMCB_NESTED_CONTROL: usize = 0x90;
pub(super) const VMCB_EVENT_INJECTION: usize = 0xa8;
pub(super) const VMCB_EVENT_ERROR: usize = 0xac;
pub(super) const VMCB_NESTED_CR3: usize = 0xb0;
pub(super) const VMCB_CLEAN_BITS: usize = 0xc0;
pub(super) const VMCB_NEXT_RIP: usize = 0xc8;

pub(super) const SAVE_ES: usize = 0x400;
pub(super) const SAVE_CS: usize = 0x410;
pub(super) const SAVE_SS: usize = 0x420;
pub(super) const SAVE_DS: usize = 0x430;
pub(super) const SAVE_FS: usize = 0x440;
pub(super) const SAVE_GS: usize = 0x450;
pub(super) const SAVE_GDTR: usize = 0x460;
pub(super) const SAVE_LDTR: usize = 0x470;
pub(super) const SAVE_IDTR: usize = 0x480;
pub(super) const SAVE_TR: usize = 0x490;
pub(super) const SAVE_CPL: usize = 0x4cb;
pub(super) const SAVE_EFER: usize = 0x4d0;
pub(super) const SAVE_CR4: usize = 0x548;
pub(super) const SAVE_CR3: usize = 0x550;
pub(super) const SAVE_CR0: usize = 0x558;
pub(super) const SAVE_DR7: usize = 0x560;
pub(super) const SAVE_DR6: usize = 0x568;
pub(super) const SAVE_RFLAGS: usize = 0x570;
pub(super) const SAVE_RIP: usize = 0x578;
pub(super) const SAVE_RSP: usize = 0x5d8;
pub(super) const SAVE_RAX: usize = 0x5f8;
pub(super) const SAVE_STAR: usize = 0x600;
pub(super) const SAVE_LSTAR: usize = 0x608;
pub(super) const SAVE_CSTAR: usize = 0x610;
pub(super) const SAVE_SFMASK: usize = 0x618;
pub(super) const SAVE_KERNEL_GS_BASE: usize = 0x620;
pub(super) const SAVE_SYSENTER_CS: usize = 0x628;
pub(super) const SAVE_SYSENTER_ESP: usize = 0x630;
pub(super) const SAVE_SYSENTER_EIP: usize = 0x638;
pub(super) const SAVE_PAT: usize = 0x668;

pub(super) const INTERCEPT_INTR: u32 = 1;
pub(super) const INTERCEPT_CPUID: u32 = 1 << 18;
pub(super) const INTERCEPT_HLT: u32 = 1 << 24;
pub(super) const INTERCEPT_IO: u32 = 1 << 27;
pub(super) const INTERCEPT_MSR: u32 = 1 << 28;
pub(super) const INTERCEPT_SHUTDOWN: u32 = 1 << 31;
pub(super) const INTERCEPT_SVM_INSTRUCTIONS: u32 = 0x7f;

pub(super) const EXIT_INTR: u64 = 0x60;
pub(super) const EXIT_CPUID: u64 = 0x72;
pub(super) const EXIT_HLT: u64 = 0x78;
pub(super) const EXIT_IO: u64 = 0x7b;
pub(super) const EXIT_MSR: u64 = 0x7c;
pub(super) const EXIT_SHUTDOWN: u64 = 0x7f;
pub(super) const EXIT_NPF: u64 = 0x400;

pub(super) const V_IRQ: u32 = 1 << 8;
pub(super) const V_INTR_PRIORITY_SHIFT: u32 = 16;
pub(super) const V_IGNORE_TPR: u32 = 1 << 20;
pub(super) const V_INTR_MASKING: u32 = 1 << 24;
pub(super) const EVENT_VALID: u32 = 1 << 31;
pub(super) const EVENT_ERROR_VALID: u32 = 1 << 11;
pub(super) const EVENT_EXCEPTION: u32 = 3 << 8;
pub(super) const EXCEPTION_GENERAL_PROTECTION: u32 = 13;
