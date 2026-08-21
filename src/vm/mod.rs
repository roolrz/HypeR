//! Architecture-neutral VM image formats and loading contracts.

#[cfg(any(CONFIG_ARCH_AARCH64, feature = "host-vm-model-tests"))]
pub mod aarch64;
pub mod bundle;
pub mod exit;
pub mod fdt;
pub mod interrupt;
#[cfg(any(CONFIG_ARCH_X86_64, feature = "host-vm-model-tests"))]
pub mod x86;
