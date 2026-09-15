// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! BCM2712 SDIO1 policy. Firmware validation selects an upstream-compatible
//! device graph; only the kernel's atomic bundle claim confers ownership.

mod graph;
#[cfg(any(test, all(target_os = "hyper", target_arch = "aarch64")))]
mod registers;
mod snapshot;
#[cfg(all(target_os = "hyper", target_arch = "aarch64"))]
pub mod worker;

use hyper_os::device::{self, BundleEntry, FirmwareIdentity};
use hyper_os::handle::{
    DeviceAssignmentAuthorityObject, HandleRef, OwnedHandle, PhysicalDeviceObject,
};
use hyper_os::{Error, Result, Status};

#[derive(Clone, Copy, Debug)]
struct MmioResource {
    base: u64,
    length: u64,
}
impl MmioResource {
    fn start(self) -> u64 {
        self.base
    }
    fn size(self) -> u64 {
        self.length
    }
}

#[derive(Clone, Debug)]
struct FirmwareNode {
    id: u32,
    path: String,
    compatible: Vec<String>,
    registers: Vec<MmioResource>,
    properties: Vec<(String, Vec<u8>)>,
    kernel_owned: bool,
    interrupt: Option<(u32, bool)>,
}
impl FirmwareNode {
    fn id(&self) -> u32 {
        self.id
    }
    fn path(&self) -> &str {
        &self.path
    }
    fn is_compatible(&self, value: &str) -> bool {
        self.compatible.iter().any(|item| item == value)
    }
    fn registers(&self) -> &[MmioResource] {
        &self.registers
    }
    fn kernel_claimed(&self) -> bool {
        self.kernel_owned
    }
    fn property(&self, name: &str) -> Option<&[u8]> {
        self.properties
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_slice())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceInfo {
    pub offset: u64,
    pub length: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProfileInfo {
    pub revision: u32,
    pub clock_hz: u64,
    pub gpio_widths: [u32; 2],
    pub resources: [ResourceInfo; 5],
}

struct Plan {
    entries: [BundleEntry; 5],
    irq_node: u32,
    profile: ProfileInfo,
}

fn plan(nodes: &[FirmwareNode], identity: FirmwareIdentity<'_>) -> Result<Plan> {
    let mut selected = nodes.iter().filter(|node| match identity {
        FirmwareIdentity::Compatible(value) => node.is_compatible(value),
        FirmwareIdentity::FdtPath(value) => node.path() == value,
    });
    let device = selected.next().ok_or(Error::Status(Status::NOT_FOUND))?;
    if selected.next().is_some() {
        return Err(Error::Status(Status::BUSY));
    }
    let graph =
        graph::parse(device, nodes, |_| false).ok_or(Error::Status(Status::NOT_SUPPORTED))?;
    let offsets = [0, 0x400, 0x1100, 0x2700, 0x3c00];
    Ok(Plan {
        entries: core::array::from_fn(|index| BundleEntry {
            node: graph.owners[index].0,
            resource: graph.owners[index].1,
            offset: offsets[index],
        }),
        irq_node: device.id(),
        profile: ProfileInfo {
            revision: graph.revision,
            clock_hz: 200_000_000,
            gpio_widths: graph.widths,
            resources: core::array::from_fn(|index| ResourceInfo {
                offset: offsets[index],
                length: graph.ranges[index].size(),
            }),
        },
    })
}

/// Performs no hardware access. Snapshot checks are policy only;
/// `claim_bundle` atomically excludes kernel owners and other active bundles.
pub fn claim(
    authority: HandleRef<'_, DeviceAssignmentAuthorityObject>,
    identity: FirmwareIdentity<'_>,
) -> Result<(OwnedHandle<PhysicalDeviceObject>, ProfileInfo)> {
    let nodes = snapshot::read(authority)?;
    let plan = plan(&nodes, identity)?;
    let physical = device::claim_bundle(authority, &plan.entries, plan.irq_node)?;
    Ok((physical, plan.profile))
}

/// Test-only generic device claim. Exact firmware identity is mandatory; no
/// pre-install MMIO probing or implicit selection of another device is allowed.
#[cfg(feature = "userspace-device-test")]
pub fn claim_virtio_test(
    authority: HandleRef<'_, DeviceAssignmentAuthorityObject>,
    identity: FirmwareIdentity<'_>,
) -> Result<OwnedHandle<PhysicalDeviceObject>> {
    let nodes = snapshot::read(authority)?;
    let node = virtio_test_node(&nodes, identity)?;
    device::claim_bundle(
        authority,
        &[BundleEntry {
            node,
            resource: 0,
            offset: 0,
        }],
        node,
    )
}

#[cfg(feature = "userspace-device-test")]
fn virtio_test_node(nodes: &[FirmwareNode], identity: FirmwareIdentity<'_>) -> Result<u32> {
    let FirmwareIdentity::FdtPath(path) = identity else {
        return Err(Error::Status(Status::INVALID_ARGUMENT));
    };
    let mut matching = nodes.iter().filter(|node| node.path == path);
    let node = matching.next().ok_or(Error::Status(Status::NOT_FOUND))?;
    if matching.next().is_some() {
        return Err(Error::Status(Status::BUSY));
    }
    if node.kernel_owned
        || !node.is_compatible("virtio,mmio")
        || node.interrupt.is_none_or(|(_, level)| !level)
        || node.registers.len() != 1
        || node.registers[0].length < 0x100
        || node.registers[0].length > 65536
        || !node.registers[0].base.is_multiple_of(4)
        || node.registers[0]
            .base
            .checked_add(node.registers[0].length)
            .is_none()
    {
        return Err(Error::Status(Status::NOT_SUPPORTED));
    }
    Ok(node.id)
}

#[cfg(test)]
#[path = "../../tests/sdhci_profile.rs"]
mod tests;
