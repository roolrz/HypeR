//! Per-VM interrupt topology for the RISC-V H-extension backend.

use hyper::vm::interrupt::VirtualInterruptId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidVcpuCount,
}

pub struct VmInterruptController;

impl VmInterruptController {
    pub fn new(vcpu_count: u32, _timer_interrupt: VirtualInterruptId) -> Result<Self, Error> {
        if vcpu_count == 0 {
            return Err(Error::InvalidVcpuCount);
        }
        Ok(Self)
    }
}
