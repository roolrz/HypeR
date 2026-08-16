//! Kernel policy that binds reusable virtual-device models to a VM runtime.

#[cfg(target_arch = "aarch64")]
pub(super) mod console;
#[cfg(target_arch = "aarch64")]
pub(super) mod gicv3;
#[cfg(target_arch = "x86_64")]
pub(super) mod legacy_pc;
