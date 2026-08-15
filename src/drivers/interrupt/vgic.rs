//! Architecture-neutral virtual interrupt state and list-register scheduling.
//!
//! This module owns guest-visible interrupt lifecycle policy. Architecture
//! backends only translate [`ListEntry`] values to hardware list registers.

use alloc::vec::Vec;

pub const MAX_VIRTUAL_INTERRUPT_ID: u32 = 1_019;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct VirtualCpuId(u32);

impl VirtualCpuId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct VirtualInterruptId(u32);

impl VirtualInterruptId {
    pub const fn new(value: u32) -> Option<Self> {
        if value <= MAX_VIRTUAL_INTERRUPT_ID {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn get(self) -> u32 {
        self.0
    }

    const fn is_private(self) -> bool {
        self.0 < 32
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InterruptGroup {
    Group0,
    Group1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InterruptTrigger {
    Level,
    Edge,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ListState {
    Pending,
    Active,
    PendingActive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ListEntry {
    pub interrupt: VirtualInterruptId,
    pub priority: u8,
    pub group: InterruptGroup,
    pub state: ListState,
    pub request_eoi_maintenance: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InterruptSnapshot {
    pub enabled: bool,
    pub pending: bool,
    pub active: bool,
    pub listed: bool,
    pub priority: u8,
    pub group: InterruptGroup,
    pub trigger: InterruptTrigger,
    pub target: VirtualCpuId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Allocation,
    AlreadyConfigured,
    Busy,
    InvalidCpu,
    InvalidRoute,
    NotConfigured,
    SnapshotContainsDuplicate,
}

struct Interrupt {
    id: VirtualInterruptId,
    target: VirtualCpuId,
    priority: u8,
    group: InterruptGroup,
    trigger: InterruptTrigger,
    enabled: bool,
    pending: bool,
    active: bool,
    listed: bool,
    maintenance_on_eoi: bool,
}

/// Sparse virtual interrupt controller state owned by one virtual machine.
pub struct VirtualInterruptController {
    interrupts: Vec<Interrupt>,
    vcpu_count: u32,
}

impl VirtualInterruptController {
    pub const fn new(vcpu_count: u32) -> Result<Self, Error> {
        if vcpu_count == 0 {
            return Err(Error::InvalidCpu);
        }
        Ok(Self {
            interrupts: Vec::new(),
            vcpu_count,
        })
    }

    pub fn configure(
        &mut self,
        interrupt: VirtualInterruptId,
        target: VirtualCpuId,
        priority: u8,
        group: InterruptGroup,
        trigger: InterruptTrigger,
    ) -> Result<(), Error> {
        self.validate_cpu(target)?;
        if self.position(interrupt, target).is_some()
            || (!interrupt.is_private()
                && self.interrupts.iter().any(|entry| entry.id == interrupt))
        {
            return Err(Error::AlreadyConfigured);
        }
        self.interrupts
            .try_reserve(1)
            .map_err(|_| Error::Allocation)?;
        self.interrupts.push(Interrupt {
            id: interrupt,
            target,
            priority,
            group,
            trigger,
            enabled: false,
            pending: false,
            active: false,
            listed: false,
            maintenance_on_eoi: false,
        });
        Ok(())
    }

    pub fn set_enabled(
        &mut self,
        interrupt: VirtualInterruptId,
        target: VirtualCpuId,
        enabled: bool,
    ) -> Result<(), Error> {
        let entry = self.entry_mut(interrupt, target)?;
        entry.enabled = enabled;
        Ok(())
    }

    pub fn set_priority(
        &mut self,
        interrupt: VirtualInterruptId,
        target: VirtualCpuId,
        priority: u8,
    ) -> Result<(), Error> {
        self.entry_mut(interrupt, target)?.priority = priority;
        Ok(())
    }

    pub fn set_group(
        &mut self,
        interrupt: VirtualInterruptId,
        target: VirtualCpuId,
        group: InterruptGroup,
    ) -> Result<(), Error> {
        self.entry_mut(interrupt, target)?.group = group;
        Ok(())
    }

    pub fn set_trigger(
        &mut self,
        interrupt: VirtualInterruptId,
        target: VirtualCpuId,
        trigger: InterruptTrigger,
    ) -> Result<(), Error> {
        self.entry_mut(interrupt, target)?.trigger = trigger;
        Ok(())
    }

    /// Requests a maintenance interrupt when the guest EOIs this interrupt.
    pub fn set_maintenance_on_eoi(
        &mut self,
        interrupt: VirtualInterruptId,
        target: VirtualCpuId,
        enabled: bool,
    ) -> Result<(), Error> {
        self.entry_mut(interrupt, target)?.maintenance_on_eoi = enabled;
        Ok(())
    }

    /// Routes a shared interrupt while it is neither listed nor active.
    pub fn route(
        &mut self,
        interrupt: VirtualInterruptId,
        target: VirtualCpuId,
    ) -> Result<(), Error> {
        if interrupt.is_private() {
            return Err(Error::InvalidRoute);
        }
        self.validate_cpu(target)?;
        let entry = self
            .interrupts
            .iter_mut()
            .find(|entry| entry.id == interrupt)
            .ok_or(Error::NotConfigured)?;
        if entry.listed || entry.active {
            return Err(Error::Busy);
        }
        entry.target = target;
        Ok(())
    }

    pub fn inject(
        &mut self,
        interrupt: VirtualInterruptId,
        target: VirtualCpuId,
    ) -> Result<(), Error> {
        self.entry_mut(interrupt, target)?.pending = true;
        Ok(())
    }

    pub fn clear_pending(
        &mut self,
        interrupt: VirtualInterruptId,
        target: VirtualCpuId,
    ) -> Result<(), Error> {
        self.entry_mut(interrupt, target)?.pending = false;
        Ok(())
    }

    pub fn snapshot(
        &self,
        interrupt: VirtualInterruptId,
        target: VirtualCpuId,
    ) -> Result<InterruptSnapshot, Error> {
        let entry = self.entry(interrupt, target)?;
        Ok(InterruptSnapshot {
            enabled: entry.enabled,
            pending: entry.pending,
            active: entry.active,
            listed: entry.listed,
            priority: entry.priority,
            group: entry.group,
            trigger: entry.trigger,
            target: entry.target,
        })
    }

    /// Updates a complete hardware list-register snapshot for one vCPU.
    ///
    /// Entries that were previously listed but are absent from `slots` have
    /// completed in the guest and become inactive. The caller must therefore
    /// pass every implemented hardware slot, including empty slots.
    pub fn synchronize(
        &mut self,
        vcpu: VirtualCpuId,
        slots: &[Option<ListEntry>],
    ) -> Result<(), Error> {
        self.validate_cpu(vcpu)?;
        self.validate_slots(vcpu, slots)?;
        for entry in self
            .interrupts
            .iter_mut()
            .filter(|entry| entry.target == vcpu && entry.listed)
        {
            entry.listed = false;
            entry.pending = false;
            entry.active = false;
        }
        for slot in slots {
            let Some(listed) = slot else {
                continue;
            };
            let entry = self.entry_mut(listed.interrupt, vcpu)?;
            entry.listed = true;
            match listed.state {
                ListState::Pending => {
                    entry.pending = true;
                    entry.active = false;
                }
                ListState::Active => {
                    entry.pending = false;
                    entry.active = true;
                }
                ListState::PendingActive => {
                    entry.pending = true;
                    entry.active = true;
                }
            }
        }
        Ok(())
    }

    /// Fills empty list-register slots in guest priority order.
    ///
    /// Existing active entries are promoted to pending-active when a device
    /// reinjects the same level while the interrupt remains active.
    pub fn refill(
        &mut self,
        vcpu: VirtualCpuId,
        slots: &mut [Option<ListEntry>],
    ) -> Result<usize, Error> {
        self.validate_cpu(vcpu)?;
        self.validate_slots(vcpu, slots)?;
        for slot in slots.iter_mut() {
            let Some(mut listed) = *slot else {
                continue;
            };
            let entry = self.entry_mut(listed.interrupt, vcpu)?;
            listed.priority = entry.priority;
            listed.group = entry.group;
            listed.request_eoi_maintenance = entry.maintenance_on_eoi || !entry.id.is_private();
            listed.state = match (entry.enabled, entry.pending, listed.state) {
                (false, _, ListState::Pending) | (true, false, ListState::Pending) => {
                    entry.listed = false;
                    *slot = None;
                    continue;
                }
                (false, _, ListState::PendingActive) | (true, false, ListState::PendingActive) => {
                    ListState::Active
                }
                (true, true, ListState::Active) => ListState::PendingActive,
                (_, _, state) => state,
            };
            entry.listed = true;
            *slot = Some(listed);
        }

        let mut filled = 0;
        for slot in slots.iter_mut().filter(|slot| slot.is_none()) {
            let candidate = self
                .interrupts
                .iter()
                .enumerate()
                .filter(|(_, entry)| {
                    entry.target == vcpu && entry.enabled && entry.pending && !entry.listed
                })
                .min_by_key(|(_, entry)| (entry.priority, entry.id));
            let Some((index, _)) = candidate else {
                break;
            };
            let entry = &mut self.interrupts[index];
            entry.listed = true;
            *slot = Some(ListEntry {
                interrupt: entry.id,
                priority: entry.priority,
                group: entry.group,
                state: if entry.active {
                    ListState::PendingActive
                } else {
                    ListState::Pending
                },
                request_eoi_maintenance: entry.maintenance_on_eoi || !entry.id.is_private(),
            });
            filled += 1;
        }
        Ok(filled)
    }

    fn validate_cpu(&self, vcpu: VirtualCpuId) -> Result<(), Error> {
        if vcpu.get() < self.vcpu_count {
            Ok(())
        } else {
            Err(Error::InvalidCpu)
        }
    }

    fn validate_slots(&self, vcpu: VirtualCpuId, slots: &[Option<ListEntry>]) -> Result<(), Error> {
        for (index, listed) in slots
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| slot.as_ref().map(|entry| (index, entry)))
        {
            if slots[..index]
                .iter()
                .flatten()
                .any(|previous| previous.interrupt == listed.interrupt)
            {
                return Err(Error::SnapshotContainsDuplicate);
            }
            let _ = self.entry(listed.interrupt, vcpu)?;
        }
        Ok(())
    }

    fn position(&self, interrupt: VirtualInterruptId, target: VirtualCpuId) -> Option<usize> {
        self.interrupts
            .iter()
            .position(|entry| entry.id == interrupt && entry.target == target)
    }

    fn entry(
        &self,
        interrupt: VirtualInterruptId,
        target: VirtualCpuId,
    ) -> Result<&Interrupt, Error> {
        self.validate_cpu(target)?;
        self.position(interrupt, target)
            .and_then(|index| self.interrupts.get(index))
            .ok_or(Error::NotConfigured)
    }

    fn entry_mut(
        &mut self,
        interrupt: VirtualInterruptId,
        target: VirtualCpuId,
    ) -> Result<&mut Interrupt, Error> {
        self.validate_cpu(target)?;
        let index = self
            .position(interrupt, target)
            .ok_or(Error::NotConfigured)?;
        self.interrupts.get_mut(index).ok_or(Error::NotConfigured)
    }
}
