// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Installed userspace-device routes and hardware-detached MMIO requests.

use super::{Error, InstalledMachine, RuntimeState};
use hyper::vm::device::mmio::Request;
use hyper::vm::exit::{MmioAccess, MmioAction};

#[derive(Clone)]
pub(super) struct Region {
    base: u64,
    end: u64,
    device: u64,
    #[cfg(CONFIG_ARCH_RISCV64)]
    firmware_console: bool,
    event: Option<
        crate::kernel::object::KernelRef<
            crate::kernel::object::Event,
            crate::kernel::object::VmDeviceBinding,
        >,
    >,
}

impl InstalledMachine {
    pub(in crate::kernel::vm) fn register_mmio(
        &self,
        base: u64,
        length: u64,
        device: u64,
        event: Option<
            crate::kernel::object::KernelRef<
                crate::kernel::object::Event,
                crate::kernel::object::VmDeviceBinding,
            >,
        >,
    ) -> Result<(), Error> {
        if device == 0 || length == 0 {
            return Err(Error::BadState);
        }
        let end = base.checked_add(length).ok_or(Error::BadState)?;
        self.state.with(|state| {
            if !matches!(state, RuntimeState::Installed { .. }) {
                return Err(Error::BadState);
            }
            let id = match state {
                RuntimeState::Installed { id, .. } => *id,
                _ => return Err(Error::BadState),
            };
            // Keep the generation-qualified lease through route publication.
            // The lifecycle lock serializes this with other route installation
            // and first start; a physical window is never an ambient aperture.
            let binding =
                crate::kernel::vm::registry::acquire_binding(id).map_err(|_| Error::BadState)?;
            let permitted = crate::kernel::vm::device::supports_userspace_mmio(
                self.configuration.platform_profile,
                base,
                length,
            ) || binding.owns_userspace_assignment_aperture(base, length);
            if !permitted || binding.io_range_conflicts(base, length) {
                return Err(Error::BadState);
            }
            self.mmio_regions.with(|regions| {
                if regions.iter().flatten().any(|region| {
                    region.device == device
                        || (base < region.end && region.base < end)
                        || event
                            .as_ref()
                            .zip(region.event.as_ref())
                            .is_some_and(|(a, b)| a.koid() == b.koid())
                }) {
                    return Err(Error::BadState);
                }
                let slot = regions
                    .iter_mut()
                    .find(|slot| slot.is_none())
                    .ok_or(Error::BadState)?;
                *slot = Some(Region {
                    base,
                    end,
                    device,
                    event,
                    #[cfg(CONFIG_ARCH_RISCV64)]
                    firmware_console: false,
                });
                Ok(())
            })
        })
    }

    #[allow(
        dead_code,
        reason = "selected guest backends opt into deferred userspace MMIO"
    )]
    pub(in crate::kernel) fn route_mmio(
        &self,
        vcpu: u32,
        access: MmioAccess,
    ) -> Option<MmioAction> {
        let base = access.address().get();
        let end = base.checked_add(access.size() as u64)?;
        let device = self.mmio_regions.with(|regions| {
            regions
                .iter()
                .flatten()
                .find(|region| base >= region.base && end <= region.end)
                .map(|region| region.device)
        })?;
        match self.endpoint(vcpu).and_then(|endpoint| {
            endpoint
                .stage_mmio(device, access)
                .map_err(|_| Error::BadState)
        }) {
            Ok(()) => Some(MmioAction::Deferred),
            Err(_) => Some(MmioAction::Stop),
        }
    }

    pub(in crate::kernel::vm) fn publish_mmio(&self, vcpu: u32) -> Result<(), Error> {
        let endpoint = self.endpoint(vcpu)?;
        let routed = endpoint.staged_mmio().ok_or(Error::BadState)?;
        let event_route = self.mmio_regions.with(|regions| {
            regions
                .iter()
                .flatten()
                .any(|region| region.device == routed.device && region.event.is_some())
        });
        endpoint
            .publish_mmio(!event_route)
            .map_err(|_| Error::BadState)?;
        // The pending slot is durable before the prompt. A consumer clears its
        // private event before scanning every vCPU; completion never clears it.
        if let Some(request) = endpoint.pending_mmio() {
            self.mmio_regions.with(|regions| {
                if let Some(event) = regions
                    .iter()
                    .flatten()
                    .find(|region| region.device == request.device)
                    .and_then(|region| region.event.as_ref())
                    && event
                        .object()
                        .signal(0, hyper::abi::native::HYPER_NATIVE_SIGNAL_EVENT_SIGNALED)
                        .is_err()
                {
                    hyper::debug::invariant_failure("MMIO route event publication");
                }
            });
        }
        Ok(())
    }

    pub(in crate::kernel::vm) fn pending_mmio(
        &self,
        vcpu: u32,
        device: u64,
    ) -> Result<Option<Request>, Error> {
        self.state.with(|state| {
            if matches!(state, RuntimeState::Installed { .. }) {
                return Ok(None);
            }
            if !matches!(state, RuntimeState::Running { .. }) {
                return Err(Error::BadState);
            }
            Ok(self.endpoint(vcpu)?.pending_mmio().filter(|request| {
                self.mmio_regions.with(|regions| {
                    regions.iter().flatten().any(|region| {
                        region.device == request.device
                            && if device == 0 {
                                region.event.is_none()
                            } else {
                                region.device == device
                            }
                    })
                })
            }))
        })
    }

    pub(in crate::kernel::vm) fn complete_mmio(
        &self,
        vcpu: u32,
        id: u64,
        action: MmioAction,
    ) -> Result<(), Error> {
        // Completion and stop admission have one lifecycle lock. The runner
        // alone updates architectural state after consuming this result.
        self.state.with(|state| {
            if !matches!(state, RuntimeState::Running { .. }) {
                return Err(Error::BadState);
            }
            self.endpoint(vcpu)?
                .complete_mmio(id, action)
                .map_err(|_| Error::BadState)
        })
    }

    pub(in crate::kernel::vm) fn take_mmio_completion(
        &self,
        vcpu: u32,
    ) -> Result<Option<MmioAction>, Error> {
        self.endpoint(vcpu)?
            .take_mmio_completion()
            .map_err(|_| Error::BadState)
    }
}

impl InstalledMachine {
    pub(in crate::kernel::vm) fn validate_io_route(
        &self,
        route: &crate::kernel::vm::io::Route,
    ) -> Result<(), crate::kernel::vm::io::Error> {
        let base = route.base();
        self.mmio_regions.with(|regions| {
            if regions.iter().flatten().any(|region| {
                base < region.end
                    && region.base < base + 4096
                    && !(route.deferred_overlay()
                        && region.base == base
                        && region.end == base + 4096)
            }) {
                Err(crate::kernel::vm::io::Error::Busy)
            } else {
                Ok(())
            }
        })
    }
}

impl InstalledMachine {
    /// Management authority updates only shared device lines, never SGIs/PPIs.
    pub(in crate::kernel::vm) fn set_device_interrupt(
        &self,
        interrupt: u32,
        asserted: bool,
    ) -> Result<(), Error> {
        self.state.with(|state| {
            let id = match state { RuntimeState::Installed { id, .. } | RuntimeState::Running { id, .. } => *id, _ => return Err(Error::BadState) };
            let binding = crate::kernel::vm::registry::acquire_binding(id).map_err(|_| Error::BadState)?;
            #[cfg(CONFIG_ARCH_AARCH64)] {
                if !(32..hyper::abi::native::HYPER_NATIVE_VIRTUAL_PLATFORM_AARCH64_REFERENCE_INTERRUPT_COUNT as u32).contains(&interrupt) { return Err(Error::BadState); }
                crate::hal::vm::update_saved_device_line(binding.interrupts(), interrupt, asserted).map_err(|_| Error::BadState)?;
                binding.publish_changed_interrupts();
            }
            #[cfg(CONFIG_ARCH_RISCV64)] {
                crate::hal::vm::update_saved_guest_device_interrupt(binding.interrupts(), 0, hyper::vm::interrupt::VirtualInterruptId::new(interrupt), asserted).map_err(|_| Error::BadState)?;
                if let Some(thread) = binding.endpoint_owner(0).map_err(|_| Error::BadState)?.thread() {
                    binding.publish_interrupt_reconcile(0, thread).map_err(|_| Error::BadState)?;
                }
            }
            #[cfg(CONFIG_ARCH_X86_64)] { let _ = (binding, interrupt, asserted); Err(Error::BadState) }
            #[cfg(not(CONFIG_ARCH_X86_64))] Ok(())
        })
    }
}

impl InstalledMachine {
    pub(in crate::kernel::vm) fn bind_firmware_console(&self, device: u64) -> Result<(), Error> {
        #[cfg(not(CONFIG_ARCH_RISCV64))]
        {
            let _ = device;
            Err(Error::BadState)
        }
        #[cfg(CONFIG_ARCH_RISCV64)]
        self.state.with(|state| {
            if !matches!(state, RuntimeState::Installed { .. }) {
                return Err(Error::BadState);
            }
            self.mmio_regions.with(|regions| {
                if regions
                    .iter()
                    .flatten()
                    .any(|region| region.firmware_console)
                {
                    return Err(Error::BadState);
                }
                let region = regions
                    .iter_mut()
                    .flatten()
                    .find(|region| region.device == device && region.event.is_some())
                    .ok_or(Error::BadState)?;
                region.firmware_console = true;
                Ok(())
            })
        })
    }
    #[cfg(CONFIG_ARCH_RISCV64)]
    pub(in crate::kernel) fn route_firmware_console(&self, vcpu: u32, byte: u8) -> bool {
        let device = self.mmio_regions.with(|regions| {
            regions
                .iter()
                .flatten()
                .find(|region| region.firmware_console)
                .map(|region| region.device)
        });
        device.is_some_and(|device| {
            self.endpoint(vcpu)
                .is_ok_and(|endpoint| endpoint.stage_firmware_console(device, byte).is_ok())
        })
    }
}
