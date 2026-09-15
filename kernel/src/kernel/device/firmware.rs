// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Immutable firmware facts. Driver binding and dependency policy are userspace.

use super::assigned::service::MatchError;
use crate::kernel::vm::service::Error;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};
use hyper::drivers::platform::PlatformDevice;
use hyper::mm::FallibleArc;

pub(super) struct Catalogue {
    pub(super) nodes: Vec<PlatformDevice>,
    reserved: Vec<AtomicBool>,
}

impl Catalogue {
    pub(super) fn new(
        nodes: Vec<PlatformDevice>,
        owned: impl Fn(&PlatformDevice) -> bool,
    ) -> Result<FallibleArc<Self>, ()> {
        let mut reserved = Vec::new();
        reserved.try_reserve_exact(nodes.len()).map_err(|_| ())?;
        reserved.extend((0..nodes.len()).map(|_| AtomicBool::new(false)));
        let catalogue = FallibleArc::try_new(Self { nodes, reserved }).map_err(|_| ())?;
        catalogue.reserve(owned);
        Ok(catalogue)
    }

    // Boot-only mutation, completed before the registry publishes this owner.
    pub(super) fn reserve(&self, owned: impl Fn(&PlatformDevice) -> bool) {
        for (index, node) in self.nodes.iter().enumerate() {
            // Protect byte resources and their device pages against firmware
            // aliases. A second node name never grants access to a kernel owner.
            let conflict = self.nodes.iter().filter(|node| owned(node)).any(|owner| {
                let irq_conflict = irq(owner)
                    .zip(irq(node))
                    .is_some_and(|(a, b)| a.0 >= 32 && a.0 == b.0);
                irq_conflict
                    || owner.registers().iter().any(|left| {
                        node.registers().iter().any(|right| {
                            let end = |range: &hyper::drivers::platform::MmioResource| {
                                range
                                    .start()
                                    .checked_add(range.size())
                                    .and_then(|v| v.checked_add(4095))
                                    .map(|v| v & !4095)
                            };
                            match (end(left), end(right)) {
                                (Some(a), Some(b)) => {
                                    (left.start() & !4095) < b && (right.start() & !4095) < a
                                }
                                _ => true,
                            }
                        })
                    })
            });
            self.reserved[index].store(owned(node) || conflict, Ordering::Relaxed);
        }
    }
    pub(super) fn reserved(&self, index: usize) -> bool {
        self.reserved[index].load(Ordering::Relaxed)
    }

    pub(super) fn read(&self, index: u32, field: u32, name: &str) -> Result<Vec<u8>, MatchError> {
        let node = self.nodes.get(index as usize).ok_or(MatchError::Missing)?;
        let mut out = Vec::new();
        let mut append = |bytes: &[u8]| -> Result<(), MatchError> {
            if out
                .len()
                .checked_add(bytes.len())
                .is_none_or(|size| size > 65536)
            {
                return Err(Error::InvalidArgument.into());
            }
            out.try_reserve(bytes.len()).map_err(|_| Error::NoMemory)?;
            out.extend_from_slice(bytes);
            Ok(())
        };
        match field {
            0 => {
                let (interrupt, trigger) = irq(node).unwrap_or((0, 0));
                let mut bytes = [0; 32];
                bytes[..4].copy_from_slice(&u32::from(self.reserved(index as usize)).to_le_bytes());
                bytes[4..8].copy_from_slice(&(node.registers().len() as u32).to_le_bytes());
                bytes[8..12].copy_from_slice(&interrupt.to_le_bytes());
                bytes[12..16].copy_from_slice(&trigger.to_le_bytes());
                append(&bytes)?;
            }
            1 => append(node.path().as_bytes())?,
            2 => {
                for value in node.compatibles() {
                    append(value.as_bytes())?;
                    append(&[0])?;
                }
            }
            3 => {
                for range in node.registers() {
                    append(&range.start().to_le_bytes())?;
                    append(&range.size().to_le_bytes())?;
                }
            }
            4 if name == "interrupts" && !node.interrupt_cells().is_empty() => {
                for cell in node.interrupt_cells() {
                    append(&cell.to_be_bytes())?;
                }
            }
            4 => append(node.property(name).ok_or(MatchError::Missing)?)?,
            _ => return Err(Error::InvalidArgument.into()),
        }
        Ok(out)
    }
}

fn irq(node: &PlatformDevice) -> Option<(u32, u32)> {
    let irq = crate::hal::irq::decode_platform(node.interrupt_cells()).ok()?;
    Some((
        irq.interrupt,
        match irq.trigger {
            hyper::platform::PlatformInterruptTrigger::Level => 1,
            hyper::platform::PlatformInterruptTrigger::Edge => 2,
        },
    ))
}
