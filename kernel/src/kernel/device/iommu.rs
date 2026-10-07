// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Host ownership of SMMU registers and coherent control-memory allocations.

pub(crate) mod runtime;

use crate::kernel::mm::{memory, page_block::PageBlock};
use hyper::drivers::{
    iommu::smmuv3::{self, DmaMemory, Environment},
    platform::{DriverServices, PlatformDevice},
};
use hyper::hal::barrier::{Barrier, BarrierAccess, BarrierDomain};
use hyper::mm::FallibleArc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    Driver(smmuv3::Error),
    Interrupt(crate::kernel::irq::interrupt::Error),
    Scheduler(crate::kernel::task::scheduler::Error),
}
impl From<smmuv3::Error> for Error {
    fn from(error: smmuv3::Error) -> Self {
        Self::Driver(error)
    }
}

pub(super) type Controller = smmuv3::Controller<KernelEnvironment>;
pub(crate) struct KernelEnvironment;
pub(crate) struct CoherentMemory {
    page: PageBlock,
    address: usize,
}

// SAFETY: PageBlock exclusively owns this aligned contiguous allocation, and
// the linear mapping remains stable. Admission below requires coherent hardware.
// No Rust references into the bytes escape this ownership wrapper.
unsafe impl DmaMemory for CoherentMemory {
    fn physical(&self) -> u64 {
        self.page.physical().get()
    }
    fn virtual_address(&self) -> usize {
        self.address
    }
    fn order(&self) -> usize {
        self.page.order()
    }
}

impl Environment for KernelEnvironment {
    type Memory = CoherentMemory;
    fn allocate(order: usize) -> Result<Self::Memory, smmuv3::Error> {
        let page = PageBlock::allocate(order).map_err(|_| smmuv3::Error::Allocation)?;
        let address =
            memory::linear_address(page.physical().get()).ok_or(smmuv3::Error::Address)?;
        Ok(CoherentMemory { page, address })
    }
    fn synchronize() {
        crate::hal::memory::Barrier::data_synchronization(
            BarrierDomain::FullSystem,
            BarrierAccess::All,
        );
    }
    fn now_microseconds() -> u64 {
        crate::kernel::time::monotonic_microseconds()
    }
}

pub(super) fn host_owned(node: &PlatformDevice) -> bool {
    node.is_compatible("arm,smmu-v3") || node.is_compatible("pci-host-ecam-generic")
}

pub(super) fn initialize(
    nodes: &[PlatformDevice],
    services: &dyn DriverServices,
    root: crate::kernel::irq::interrupt::IrqDomainId,
) -> Result<Option<FallibleArc<runtime::Runtime>>, Error> {
    let mut smmus = nodes
        .iter()
        .filter(|node| node.is_compatible("arm,smmu-v3"));
    let Some(node) = smmus.next() else {
        return Ok(None);
    };
    if smmus.next().is_some()
        || node.property("dma-coherent").is_none()
        || node.registers().len() != 1
    {
        return Err(smmuv3::Error::Unsupported.into());
    }
    let routes = interrupt_routes(node)?;
    let mapping = services
        .map_mmio(node.registers()[0])
        .map_err(|_| smmuv3::Error::Address)?;
    // SAFETY: Boot owns this discovered SMMU before any device assignment; the
    // firmware entry contract requires quiescent bus masters. The catalogue
    // reserves its MMIO and PCI configuration apertures for the host permanently.
    let controller = unsafe { Controller::initialize(mapping) }?;
    let caps = controller.capabilities();
    crate::pr_info!(
        "HypeR: SMMUv3 stage-2 active: {} StreamID bits, {}-bit PA; default deny",
        caps.stream_bits,
        caps.address_bits
    );
    #[cfg(all(CONFIG_ARCH_AARCH64, feature = "kernel-smmuv3-test"))]
    let controller = {
        let mut controller = controller;
        crate::kernel_tests::smmuv3::run(&mut controller, node, nodes, services)?;
        crate::kernel_tests::smmuv3::runtime::prepare(node, nodes, services)?;
        controller
    };
    runtime::install(controller, routes, root).map(Some)
}

fn interrupt_routes(
    node: &PlatformDevice,
) -> Result<
    (
        hyper::platform::PlatformInterrupt,
        Option<hyper::platform::PlatformInterrupt>,
    ),
    Error,
> {
    let names = node
        .property("interrupt-names")
        .ok_or(smmuv3::Error::Unsupported)?;
    let routes = smmuv3::firmware::wired_interrupts(names, node.interrupt_cells())?;
    let decode = |cells: [u32; 3]| {
        crate::hal::irq::decode_platform(&cells)
            .map_err(|_| Error::Driver(smmuv3::Error::Unsupported))
    };
    Ok((
        decode(routes.event)?,
        routes.global_error.map(decode).transpose()?,
    ))
}
