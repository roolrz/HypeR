//! Architecture-neutral exception diagnostics passed to kernel policy.

/// Architectural origin represented by a vector-table entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExceptionOrigin {
    CurrentSp0,
    CurrentSpx,
    LowerAarch64,
    LowerAarch32,
}

/// Top-level exception class independent of a concrete instruction set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExceptionKind {
    Synchronous,
    Irq,
    Fiq,
    SystemError,
}

/// Immutable machine state used by the kernel fatal-exception policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExceptionReport {
    pub origin: ExceptionOrigin,
    pub kind: ExceptionKind,
    pub architecture_class: u8,
    pub description: &'static str,
    pub syndrome: u64,
    pub instruction_pointer: u64,
    pub fault_address_register: u64,
    pub status: u64,
    pub stack_pointer: u64,
}
