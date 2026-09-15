// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Admitted I/O devices and reserved, page-backed inter-VM RAM aliases.

use super::{Aarch64LinuxBoot, Builder, Error, hex_node_name};

mod sdhci;
pub use sdhci::{MmioWindow, SdhciDevice, SdhciRevision};

const PAGE_SIZE: u64 = 4096;
const SHARED_PHANDLE: u32 = 4;
const MAX_DMA_RANGES: usize = 8;
/// Imported physical pages use one immutable bus translation. This keeps low
/// Pi host RAM from colliding with the reference guest's MMIO addresses.
pub const DYNAMIC_ALIAS_OFFSET: u64 = hyper_abi::HYPER_NATIVE_GUEST_DYNAMIC_ALIAS_OFFSET;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MmioDevice {
    pub base: u64,
    pub size: u64,
    /// Full GIC INTID, including the SPI offset of 32.
    pub irq: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DmaRange {
    /// Physical address observed by the assigned device (`HypeR` host PA).
    pub dma_base: u64,
    /// Linux I/O VM CPU physical address for the same backing pages.
    pub cpu_base: u64,
    pub size: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SharedMemory {
    /// Alias in the I/O VM's CPU physical address space.
    pub base: u64,
    pub size: u64,
    /// Original business VM GPA, used to configure the vhost memory table.
    pub guest_base: u64,
}

/// One independently authorized backend context. Dynamic windows describe an
/// fixed affine DMA address domain, not admitted RAM or a preallocated guest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IoClient {
    pub id: u32,
    pub shared_memory: SharedMemory,
    pub mailbox: MmioDevice,
    pub notification: MmioDevice,
    pub dynamic: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IoDevices<'ranges> {
    pub clients: &'ranges [IoClient],
    pub virtio: Option<MmioDevice>,
    pub sdhci: Option<SdhciDevice>,
    /// Empty for a virtual frontend; physical assignment requires complete
    /// coverage of ordinary and imported RAM through explicit DMA translations.
    pub dma_ranges: &'ranges [DmaRange],
    pub shared_memory: Option<SharedMemory>,
    pub mailbox: Option<MmioDevice>,
    pub notification: Option<MmioDevice>,
}

impl IoDevices<'_> {
    pub const fn empty() -> Self {
        Self {
            clients: &[],
            virtio: None,
            sdhci: None,
            dma_ranges: &[],
            shared_memory: None,
            mailbox: None,
            notification: None,
        }
    }

    pub(super) fn validate(self, boot: Aarch64LinuxBoot<'_>) -> Result<(), Error> {
        if let Some(sdhci) = self.sdhci {
            sdhci.validate(self, boot)?;
        }
        if !self.clients.is_empty() {
            return self.validate_clients(boot);
        }
        let own_end = end(boot.memory_base, boot.memory_size)?;
        if let Some(shared) = self.shared_memory {
            let shared_end = end(shared.base, shared.size)?;
            end(shared.guest_base, shared.size)?;
            if overlaps(shared.base, shared_end, boot.memory_base, own_end)
                || boot
                    .initramfs
                    .is_some_and(|(base, limit)| overlaps(shared.base, shared_end, base, limit))
            {
                return Err(Error::InvalidInput);
            }
        }
        let slots = [self.physical_node(), self.mailbox, self.notification];
        for (index, device) in slots.iter().enumerate() {
            let Some(device) = device else {
                continue;
            };
            if device.size == 0
                || device.size > PAGE_SIZE
                || !device.base.is_multiple_of(PAGE_SIZE)
                || !(40..64).contains(&device.irq)
                || device.base.checked_add(PAGE_SIZE).is_none()
            {
                return Err(Error::InvalidInput);
            }
            let physical = index == 0 && !self.dma_ranges.is_empty();
            let aperture = if physical {
                0x0b00_0000..0x0c00_0000
            } else {
                0x0a00_0000..0x0b00_0000
            };
            if !aperture.contains(&device.base)
                || (index != 0 && device.size != PAGE_SIZE)
                || overlaps(
                    device.base,
                    device.base + PAGE_SIZE,
                    boot.memory_base,
                    own_end,
                )
                || self.shared_memory.is_some_and(|shared| {
                    overlaps(
                        device.base,
                        device.base + PAGE_SIZE,
                        shared.base,
                        shared.base + shared.size,
                    )
                })
                || slots[..index]
                    .iter()
                    .flatten()
                    .any(|other| other.base == device.base || other.irq == device.irq)
            {
                return Err(Error::InvalidInput);
            }
        }
        if self.dma_ranges.is_empty() {
            return Ok(());
        }
        if self.physical_node().is_none() || self.dma_ranges.len() > MAX_DMA_RANGES {
            return Err(Error::InvalidInput);
        }
        let mut covered = 0u64;
        for (index, range) in self.dma_ranges.iter().enumerate() {
            let cpu_end = end(range.cpu_base, range.size)?;
            let dma_end = end(range.dma_base, range.size)?;
            let in_own = range.cpu_base >= boot.memory_base && cpu_end <= own_end;
            let in_shared = self.shared_memory.is_some_and(|shared| {
                range.cpu_base >= shared.base && cpu_end <= shared.base + shared.size
            });
            if (!in_own && !in_shared)
                || self.dma_ranges[..index].iter().any(|old| {
                    overlaps(
                        range.cpu_base,
                        cpu_end,
                        old.cpu_base,
                        old.cpu_base + old.size,
                    ) || overlaps(
                        range.dma_base,
                        dma_end,
                        old.dma_base,
                        old.dma_base + old.size,
                    )
                })
            {
                return Err(Error::InvalidInput);
            }
            covered = covered
                .checked_add(range.size)
                .ok_or(Error::AddressOverflow)?;
        }
        let expected = boot
            .memory_size
            .checked_add(self.shared_memory.map_or(0, |shared| shared.size))
            .ok_or(Error::AddressOverflow)?;
        if covered != expected {
            return Err(Error::InvalidInput);
        }
        Ok(())
    }

    fn physical_node(self) -> Option<MmioDevice> {
        self.virtio.or(self.sdhci.map(|device| device.host))
    }

    fn validate_clients(self, boot: Aarch64LinuxBoot<'_>) -> Result<(), Error> {
        if self.clients.len() > hyper_abi::HYPER_NATIVE_IO_MAX_CLIENTS as usize
            || self.shared_memory.is_some()
            || self.mailbox.is_some()
            || self.notification.is_some()
            || self.dma_ranges.len() > MAX_DMA_RANGES
        {
            return Err(Error::InvalidInput);
        }
        let own_end = end(boot.memory_base, boot.memory_size)?;
        let mut devices = [None; 1 + 2 * hyper_abi::HYPER_NATIVE_IO_MAX_CLIENTS as usize];
        devices[0] = self.physical_node();
        for (index, client) in self.clients.iter().enumerate() {
            let shared = client.shared_memory;
            let shared_end = end(shared.base, shared.size)?;
            end(shared.guest_base, shared.size)?;
            if client.id > 127
                || overlaps(shared.base, shared_end, boot.memory_base, own_end)
                || boot
                    .initramfs
                    .is_some_and(|(base, limit)| overlaps(shared.base, shared_end, base, limit))
                || self.clients[..index].iter().any(|old| {
                    old.id == client.id
                        || (!(old.dynamic && client.dynamic)
                            && overlaps(
                                shared.base,
                                shared_end,
                                old.shared_memory.base,
                                old.shared_memory.base + old.shared_memory.size,
                            ))
                })
            {
                return Err(Error::InvalidInput);
            }
            devices[1 + index * 2] = Some(client.mailbox);
            devices[2 + index * 2] = Some(client.notification);
        }
        for (index, device) in devices.iter().enumerate() {
            let Some(device) = device else {
                continue;
            };
            let physical = index == 0 && !self.dma_ranges.is_empty();
            let aperture = if physical {
                0x0b00_0000..0x0c00_0000
            } else {
                0x0a00_0000..0x0b00_0000
            };
            if device.size == 0
                || device.size > PAGE_SIZE
                || (index != 0 && device.size != PAGE_SIZE)
                || !device.base.is_multiple_of(PAGE_SIZE)
                || !(40..64).contains(&device.irq)
                || !aperture.contains(&device.base)
                || overlaps(
                    device.base,
                    device.base + PAGE_SIZE,
                    boot.memory_base,
                    own_end,
                )
                || self.clients.iter().any(|client| {
                    overlaps(
                        device.base,
                        device.base + PAGE_SIZE,
                        client.shared_memory.base,
                        client.shared_memory.base + client.shared_memory.size,
                    )
                })
                || devices[..index]
                    .iter()
                    .flatten()
                    .any(|old| old.base == device.base || old.irq == device.irq)
            {
                return Err(Error::InvalidInput);
            }
        }
        if self.dma_ranges.is_empty() {
            return Ok(());
        }
        if self.physical_node().is_none() {
            return Err(Error::InvalidInput);
        }
        let mut admitted_coverage = 0u64;
        for (index, range) in self.dma_ranges.iter().enumerate() {
            let cpu_end = end(range.cpu_base, range.size)?;
            let dma_end = end(range.dma_base, range.size)?;
            let in_own = range.cpu_base >= boot.memory_base && cpu_end <= own_end;
            let in_static = self.clients.iter().any(|client| {
                !client.dynamic
                    && range.cpu_base >= client.shared_memory.base
                    && cpu_end <= client.shared_memory.base + client.shared_memory.size
            });
            let in_dynamic = range.cpu_base.checked_sub(DYNAMIC_ALIAS_OFFSET)
                == Some(range.dma_base)
                && self.clients.iter().any(|client| {
                    client.dynamic
                        && range.cpu_base >= client.shared_memory.base
                        && cpu_end <= client.shared_memory.base + client.shared_memory.size
                });
            if (!in_own && !in_static && !in_dynamic)
                || self.dma_ranges[..index].iter().any(|old| {
                    overlaps(
                        range.cpu_base,
                        cpu_end,
                        old.cpu_base,
                        old.cpu_base + old.size,
                    ) || overlaps(
                        range.dma_base,
                        dma_end,
                        old.dma_base,
                        old.dma_base + old.size,
                    )
                })
            {
                return Err(Error::InvalidInput);
            }
            if in_own || in_static {
                admitted_coverage = admitted_coverage
                    .checked_add(range.size)
                    .ok_or(Error::AddressOverflow)?;
            }
        }
        let expected = self
            .clients
            .iter()
            .filter(|client| !client.dynamic)
            .try_fold(boot.memory_size, |size, client| {
                size.checked_add(client.shared_memory.size)
                    .ok_or(Error::AddressOverflow)
            })?;
        if admitted_coverage != expected {
            return Err(Error::InvalidInput);
        }
        Ok(())
    }

    fn append_clients(self, builder: &mut Builder<'_>) -> Result<(), Error> {
        let mut name = [0u8; 64];
        for client in self.clients.iter().filter(|client| !client.dynamic) {
            let memory = client.shared_memory;
            builder.begin_node(hex_node_name("memory@", memory.base, &mut name)?)?;
            builder.property_string("device_type", "memory")?;
            builder.property_u64_pair("reg", memory.base, memory.size)?;
            builder.end_node()?;
        }
        if self.clients.iter().any(|client| !client.dynamic) {
            builder.begin_node("reserved-memory")?;
            builder.property_u32("#address-cells", 2)?;
            builder.property_u32("#size-cells", 2)?;
            builder.property_empty("ranges")?;
            for client in self.clients.iter().filter(|client| !client.dynamic) {
                let memory = client.shared_memory;
                builder.begin_node(hex_node_name("guest-memory@", memory.base, &mut name)?)?;
                builder.property_u64_pair("reg", memory.base, memory.size)?;
                builder.property_u32("phandle", SHARED_PHANDLE + client.id)?;
                builder.end_node()?;
            }
            builder.end_node()?;
        }
        for client in self.clients {
            let memory = client.shared_memory;
            if client.dynamic {
                // Linux derives platform names from translated reg + node
                // basename, ignoring the unit address. Shared apertures need
                // the client identity in the basename, not only after '@'.
                let mut prefix = [0u8; 48];
                let length =
                    hex_node_name("hyper-guest-memory-", u64::from(client.id), &mut prefix)?.len();
                *prefix.get_mut(length).ok_or(Error::OutputTooSmall)? = b'@';
                let prefix =
                    core::str::from_utf8(&prefix[..length + 1]).map_err(|_| Error::InvalidInput)?;
                builder.begin_node(hex_node_name(prefix, memory.base, &mut name)?)?;
            } else {
                builder.begin_node(hex_node_name(
                    "hyper-guest-memory@",
                    u64::from(client.id),
                    &mut name,
                )?)?;
            }
            builder.property_string("compatible", "hyper,guest-memory-v1")?;
            builder.property_u32("hyper,client-id", client.id)?;
            if client.dynamic {
                builder.property_empty("hyper,dynamic-memory")?;
                builder.property_u64_pair("reg", memory.base, memory.size)?;
            } else {
                builder.property_u32("memory-region", SHARED_PHANDLE + client.id)?;
                builder.property_u64("hyper,guest-base", memory.guest_base)?;
            }
            builder.end_node()?;
            client_node(
                builder,
                "guest-mailbox@",
                "hyper,guest-mailbox-v1",
                client.mailbox,
                client.id,
            )?;
            client_node(
                builder,
                "guest-notification@",
                "hyper,guest-notification-v1",
                client.notification,
                client.id,
            )?;
        }
        Ok(())
    }

    pub(super) fn append(
        self,
        builder: &mut Builder<'_>,
        _boot: Aarch64LinuxBoot<'_>,
    ) -> Result<(), Error> {
        let mut name = [0u8; 64];
        self.append_clients(builder)?;
        if let Some(shared) = self.shared_memory {
            // Reserved memory alone is insufficient: Linux needs struct pages
            // for vm_insert_page/GUP/scatterlists. Advertise this second RAM
            // extent, then reserve it from Linux allocation without no-map.
            builder.begin_node(hex_node_name("memory@", shared.base, &mut name)?)?;
            builder.property_string("device_type", "memory")?;
            builder.property_u64_pair("reg", shared.base, shared.size)?;
            builder.end_node()?;
            builder.begin_node("reserved-memory")?;
            builder.property_u32("#address-cells", 2)?;
            builder.property_u32("#size-cells", 2)?;
            builder.property_empty("ranges")?;
            builder.begin_node(hex_node_name("guest-memory@", shared.base, &mut name)?)?;
            builder.property_u64_pair("reg", shared.base, shared.size)?;
            builder.property_u32("phandle", SHARED_PHANDLE)?;
            builder.end_node()?;
            builder.end_node()?;
            builder.begin_node("hyper-guest-memory")?;
            builder.property_string("compatible", "hyper,guest-memory-v1")?;
            builder.property_u32("memory-region", SHARED_PHANDLE)?;
            builder.property_u64("hyper,guest-base", shared.guest_base)?;
            builder.end_node()?;
        }
        if let Some(device) = self.physical_node() {
            let physical = !self.dma_ranges.is_empty();
            if physical {
                builder.begin_node("io-bus")?;
                builder.property_string("compatible", "simple-bus")?;
                builder.property_u32("#address-cells", 2)?;
                builder.property_u32("#size-cells", 2)?;
                builder.property_empty("ranges")?;
                if self.sdhci.is_none() {
                    builder.property_empty("dma-coherent")?;
                }
                let mut cells = [0u32; MAX_DMA_RANGES * 6];
                for (range, cells) in self.dma_ranges.iter().zip(cells.chunks_exact_mut(6)) {
                    // DT uses child bus (DMA/HPA), parent CPU (Linux GPA), size.
                    cells.copy_from_slice(&[
                        (range.dma_base >> 32) as u32,
                        range.dma_base as u32,
                        (range.cpu_base >> 32) as u32,
                        range.cpu_base as u32,
                        (range.size >> 32) as u32,
                        range.size as u32,
                    ]);
                }
                builder.property_cells("dma-ranges", &cells[..self.dma_ranges.len() * 6])?;
            }
            if let Some(sdhci) = self.sdhci {
                sdhci.append(builder)?;
            } else {
                node(builder, "virtio_mmio@", "virtio,mmio", device)?;
            }
            if physical {
                builder.end_node()?;
            }
        }
        if let Some(mailbox) = self.mailbox {
            node(builder, "guest-mailbox@", "hyper,guest-mailbox-v1", mailbox)?;
        }
        if let Some(notification) = self.notification {
            node(
                builder,
                "guest-notification@",
                "hyper,guest-notification-v1",
                notification,
            )?;
        }
        Ok(())
    }
}

fn client_node(
    builder: &mut Builder<'_>,
    prefix: &str,
    compatible: &str,
    device: MmioDevice,
    client: u32,
) -> Result<(), Error> {
    let mut name = [0u8; 64];
    builder.begin_node(hex_node_name(prefix, device.base, &mut name)?)?;
    builder.property_string("compatible", compatible)?;
    builder.property_u64_pair("reg", device.base, device.size)?;
    builder.property_u32("hyper,client-id", client)?;
    builder.property_cells("interrupts", &[0, device.irq - 32, 4])?;
    builder.end_node()
}

fn node(
    builder: &mut Builder<'_>,
    prefix: &str,
    compatible: &str,
    device: MmioDevice,
) -> Result<(), Error> {
    let mut name = [0u8; 64];
    builder.begin_node(hex_node_name(prefix, device.base, &mut name)?)?;
    builder.property_string("compatible", compatible)?;
    builder.property_u64_pair("reg", device.base, device.size)?;
    builder.property_cells("interrupts", &[0, device.irq - 32, 4])?;
    builder.end_node()
}

fn end(base: u64, size: u64) -> Result<u64, Error> {
    if size == 0 || !base.is_multiple_of(PAGE_SIZE) || !size.is_multiple_of(PAGE_SIZE) {
        return Err(Error::InvalidInput);
    }
    base.checked_add(size).ok_or(Error::AddressOverflow)
}
const fn overlaps(base: u64, end: u64, other: u64, other_end: u64) -> bool {
    base < other_end && other < end
}

#[cfg(test)]
#[path = "../../tests/io_fdt.rs"]
mod tests;
