// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Indexed virtual-GIC state and list-register scheduling.

use alloc::vec::Vec;
use core::num::NonZeroU32;

use crate::vm::interrupt::{VirtualCpuId, VirtualInterruptId};

use super::ready::{BoundedVec, EntryIndex, ReadyEntries, ReadyError, ReadyQueue, ReadyRank};

const PRIVATE_INTERRUPT_COUNT: usize = 32;
const MAX_GIC_INTERRUPT_ID: u32 = 1_019;
const SHARED_INTERRUPT_COUNT: usize = MAX_GIC_INTERRUPT_ID as usize + 1 - PRIVATE_INTERRUPT_COUNT;
const MAX_LIST_REGISTERS: usize = 16;

/// Guest-visible interrupt identifier in the modeled GIC INTID range.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct GicInterruptId(VirtualInterruptId);

impl GicInterruptId {
    pub const fn constant<const ID: u32>() -> Self {
        const { assert!(ID <= MAX_GIC_INTERRUPT_ID) };
        Self(VirtualInterruptId::constant::<ID>())
    }

    pub const fn new(value: u32) -> Option<Self> {
        if value <= MAX_GIC_INTERRUPT_ID {
            Some(Self(VirtualInterruptId::new(value)))
        } else {
            None
        }
    }

    pub const fn get(self) -> u32 {
        self.0.get()
    }

    const fn is_private(self) -> bool {
        self.get() < PRIVATE_INTERRUPT_COUNT as u32
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

impl ListState {
    const fn pending(self) -> bool {
        matches!(self, Self::Pending | Self::PendingActive)
    }

    const fn active(self) -> bool {
        matches!(self, Self::Active | Self::PendingActive)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ListEntry {
    pub interrupt: GicInterruptId,
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
pub enum BuildError {
    Allocation,
    AlreadyConfigured,
    InvalidCpu,
    InvalidListRegisterCount,
    TooManyInterrupts,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeError {
    Busy,
    CorruptState,
    InvalidCpu,
    InvalidRoute,
    InvalidSlotCount,
    NotConfigured,
    ResidencyMismatch,
    SnapshotContainsDuplicate,
}

#[derive(Clone, Copy)]
struct DirectorySlot(NonZeroU32);

impl DirectorySlot {
    fn new(index: usize) -> Result<Self, BuildError> {
        let encoded = u32::try_from(index)
            .ok()
            .and_then(|value| value.checked_add(1))
            .and_then(NonZeroU32::new)
            .ok_or(BuildError::TooManyInterrupts)?;
        Ok(Self(encoded))
    }

    const fn index(self) -> EntryIndex {
        EntryIndex(self.0.get() - 1)
    }
}

struct Interrupt {
    id: GicInterruptId,
    target: VirtualCpuId,
    priority: u8,
    group: InterruptGroup,
    trigger: InterruptTrigger,
    enabled: bool,
    pending_command: PendingCommand,
    list_state: Option<ListState>,
    ready_position: Option<usize>,
    listed_position: Option<usize>,
    maintenance_on_eoi: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PendingCommand {
    None,
    Assert,
    Clear,
}

struct EntryStore<'a>(&'a mut [Interrupt]);

impl ReadyEntries for EntryStore<'_> {
    fn rank(&self, index: EntryIndex) -> ReadyRank {
        let entry = &self.0[index.0 as usize];
        ReadyRank {
            priority: entry.priority,
            interrupt: entry.id.get(),
        }
    }

    fn position(&self, index: EntryIndex) -> Option<usize> {
        self.0[index.0 as usize].ready_position
    }

    fn set_position(&mut self, index: EntryIndex, position: Option<usize>) {
        self.0[index.0 as usize].ready_position = position;
    }
}

struct VcpuDelivery {
    ready: ReadyQueue,
    listed: BoundedVec<EntryIndex>,
}

struct ValidatedSlots {
    indices: [Option<EntryIndex>; MAX_LIST_REGISTERS],
}

/// Fallible construction phase for a virtual GIC.
///
/// Directory, entry, ready-queue, and LR-residency storage is allocated only
/// here. Configure every interrupt before calling [`Self::finish`]; the
/// returned [`VirtualGic`] has no configuration or capacity-growth API.
pub struct VirtualGicBuilder {
    entries: Vec<Interrupt>,
    private: Vec<Option<DirectorySlot>>,
    shared: Vec<Option<DirectorySlot>>,
    vcpu_count: u32,
}

impl VirtualGicBuilder {
    pub fn new(vcpu_count: u32) -> Result<Self, BuildError> {
        if vcpu_count == 0 {
            return Err(BuildError::InvalidCpu);
        }
        let private_count = usize::try_from(vcpu_count)
            .ok()
            .and_then(|count| count.checked_mul(PRIVATE_INTERRUPT_COUNT))
            .ok_or(BuildError::Allocation)?;
        Ok(Self {
            entries: Vec::new(),
            private: empty_slots(private_count)?,
            shared: empty_slots(SHARED_INTERRUPT_COUNT)?,
            vcpu_count,
        })
    }

    pub fn configure(
        &mut self,
        interrupt: GicInterruptId,
        target: VirtualCpuId,
        priority: u8,
        group: InterruptGroup,
        trigger: InterruptTrigger,
    ) -> Result<(), BuildError> {
        self.validate_cpu(target)?;
        let directory_offset = if interrupt.is_private() {
            private_offset(target, interrupt).ok_or(BuildError::InvalidCpu)?
        } else {
            shared_offset(interrupt)
        };
        let configured = if interrupt.is_private() {
            self.private.get(directory_offset)
        } else {
            self.shared.get(directory_offset)
        }
        .ok_or(BuildError::InvalidCpu)?
        .is_some();
        if configured {
            return Err(BuildError::AlreadyConfigured);
        }
        self.entries
            .try_reserve(1)
            .map_err(|_| BuildError::Allocation)?;
        let index = DirectorySlot::new(self.entries.len())?;
        self.entries.push(Interrupt {
            id: interrupt,
            target,
            priority,
            group,
            trigger,
            enabled: false,
            pending_command: PendingCommand::None,
            list_state: None,
            ready_position: None,
            listed_position: None,
            maintenance_on_eoi: false,
        });
        if interrupt.is_private() {
            self.private[directory_offset] = Some(index);
        } else {
            self.shared[directory_offset] = Some(index);
        }
        Ok(())
    }

    /// Allocates the final fixed-capacity runtime indexes and seals the model.
    pub fn finish(self, list_register_count: usize) -> Result<VirtualGic, BuildError> {
        if list_register_count == 0 || list_register_count > MAX_LIST_REGISTERS {
            return Err(BuildError::InvalidListRegisterCount);
        }
        let cpu_count = usize::try_from(self.vcpu_count).map_err(|_| BuildError::Allocation)?;
        let shared_count = self.shared.iter().flatten().count();
        let mut private_counts = empty_usizes(cpu_count)?;
        for entry in &self.entries {
            if entry.id.is_private() {
                private_counts[entry.target.get() as usize] += 1;
            }
        }
        let mut deliveries = Vec::new();
        deliveries
            .try_reserve_exact(cpu_count)
            .map_err(|_| BuildError::Allocation)?;
        for private_count in private_counts {
            let ready_capacity = private_count
                .checked_add(shared_count)
                .ok_or(BuildError::Allocation)?;
            let ready = ReadyQueue::try_with_capacity(ready_capacity).map_err(map_ready_build)?;
            let listed = BoundedVec::try_new(list_register_count).map_err(map_ready_build)?;
            deliveries.push(VcpuDelivery { ready, listed });
        }
        Ok(VirtualGic {
            entries: self.entries,
            private: self.private,
            shared: self.shared,
            deliveries,
            vcpu_count: self.vcpu_count,
            list_register_count,
        })
    }

    fn validate_cpu(&self, cpu: VirtualCpuId) -> Result<(), BuildError> {
        if cpu.get() < self.vcpu_count {
            Ok(())
        } else {
            Err(BuildError::InvalidCpu)
        }
    }
}

/// Finished virtual-GIC state. Every runtime operation is allocation-free.
pub struct VirtualGic {
    entries: Vec<Interrupt>,
    private: Vec<Option<DirectorySlot>>,
    shared: Vec<Option<DirectorySlot>>,
    deliveries: Vec<VcpuDelivery>,
    vcpu_count: u32,
    list_register_count: usize,
}

impl VirtualGic {
    /// Conservatively reports whether saved interrupt state may wake `WFI`.
    ///
    /// CPU-interface priority and group masks are deliberately ignored. That
    /// can cause a harmless resume for a masked interrupt, but never strands a
    /// vCPU because software underestimated architectural deliverability.
    pub fn may_wake_wfi(&self, vcpu: VirtualCpuId) -> Result<bool, RuntimeError> {
        self.validate_cpu(vcpu)?;
        let cpu = vcpu.get() as usize;
        self.validate_listed(cpu)?;
        // This is intentionally conservative: priority masks, group enables,
        // VMCR, and APR state are not authoritative in the saved software
        // model. Ignoring them can only cause a harmless early wake.
        if !self.deliveries[cpu].ready.is_empty() {
            return Ok(true);
        }
        Ok(self.deliveries[cpu].listed.iter().copied().any(|index| {
            let entry = &self.entries[index.0 as usize];
            entry.enabled
                && match entry.pending_command {
                    PendingCommand::Assert => true,
                    PendingCommand::Clear => false,
                    PendingCommand::None => entry.list_state.is_some_and(ListState::pending),
                }
        }))
    }

    pub fn set_enabled(
        &mut self,
        interrupt: GicInterruptId,
        target: VirtualCpuId,
        enabled: bool,
    ) -> Result<(), RuntimeError> {
        let index = self.lookup(interrupt, target)?;
        if enabled && self.entries[index.0 as usize].pending_command == PendingCommand::Assert {
            self.preflight_ready_insert(index)?;
        }
        self.entries[index.0 as usize].enabled = enabled;
        self.reconcile_ready(index)?;
        Ok(())
    }

    pub fn set_priority(
        &mut self,
        interrupt: GicInterruptId,
        target: VirtualCpuId,
        priority: u8,
    ) -> Result<(), RuntimeError> {
        let index = self.lookup(interrupt, target)?;
        self.validate_ready_position(index)?;
        self.entries[index.0 as usize].priority = priority;
        let cpu = self.entries[index.0 as usize].target.get() as usize;
        let (entries, deliveries) = (&mut self.entries, &mut self.deliveries);
        deliveries[cpu]
            .ready
            .reprioritize(index, &mut EntryStore(entries));
        Ok(())
    }

    pub fn set_group(
        &mut self,
        interrupt: GicInterruptId,
        target: VirtualCpuId,
        group: InterruptGroup,
    ) -> Result<(), RuntimeError> {
        self.entry_mut(interrupt, target)?.group = group;
        Ok(())
    }

    pub fn set_trigger(
        &mut self,
        interrupt: GicInterruptId,
        target: VirtualCpuId,
        trigger: InterruptTrigger,
    ) -> Result<(), RuntimeError> {
        self.entry_mut(interrupt, target)?.trigger = trigger;
        Ok(())
    }

    pub fn set_maintenance_on_eoi(
        &mut self,
        interrupt: GicInterruptId,
        target: VirtualCpuId,
        enabled: bool,
    ) -> Result<(), RuntimeError> {
        self.entry_mut(interrupt, target)?.maintenance_on_eoi = enabled;
        Ok(())
    }

    pub fn route(
        &mut self,
        interrupt: GicInterruptId,
        target: VirtualCpuId,
    ) -> Result<(), RuntimeError> {
        if interrupt.is_private() {
            return Err(RuntimeError::InvalidRoute);
        }
        self.validate_cpu(target)?;
        let index = self.lookup_shared(interrupt)?;
        let entry = &self.entries[index.0 as usize];
        if entry.list_state.is_some() {
            return Err(RuntimeError::Busy);
        }
        self.validate_ready_position(index)?;
        let was_ready = entry.ready_position.is_some();
        let old_cpu = entry.target.get() as usize;
        let new_cpu = target.get() as usize;
        if old_cpu == new_cpu {
            return Ok(());
        }
        if was_ready {
            self.deliveries[new_cpu]
                .ready
                .can_insert()
                .map_err(map_ready_runtime)?;
            let (entries, deliveries) = (&mut self.entries, &mut self.deliveries);
            deliveries[old_cpu]
                .ready
                .remove(index, &mut EntryStore(entries));
        }
        self.entries[index.0 as usize].target = target;
        if was_ready {
            let (entries, deliveries) = (&mut self.entries, &mut self.deliveries);
            deliveries[new_cpu]
                .ready
                .insert(index, &mut EntryStore(entries))
                .map_err(map_ready_runtime)?;
        }
        Ok(())
    }

    pub fn inject(
        &mut self,
        interrupt: GicInterruptId,
        target: VirtualCpuId,
    ) -> Result<(), RuntimeError> {
        let index = self.lookup(interrupt, target)?;
        if self.entries[index.0 as usize].enabled {
            self.preflight_ready_insert(index)?;
        }
        let entry = &mut self.entries[index.0 as usize];
        entry.pending_command = PendingCommand::Assert;
        self.reconcile_ready(index)?;
        Ok(())
    }

    pub fn clear_pending(
        &mut self,
        interrupt: GicInterruptId,
        target: VirtualCpuId,
    ) -> Result<(), RuntimeError> {
        let index = self.lookup(interrupt, target)?;
        let entry = &mut self.entries[index.0 as usize];
        entry.pending_command = if entry.list_state.is_some() {
            PendingCommand::Clear
        } else {
            PendingCommand::None
        };
        self.reconcile_ready(index)?;
        Ok(())
    }

    pub fn snapshot(
        &self,
        interrupt: GicInterruptId,
        target: VirtualCpuId,
    ) -> Result<InterruptSnapshot, RuntimeError> {
        let entry = self.entry(interrupt, target)?;
        let listed_pending = entry.list_state.is_some_and(ListState::pending);
        let pending = match entry.pending_command {
            PendingCommand::Assert => true,
            PendingCommand::Clear => false,
            PendingCommand::None => listed_pending,
        };
        Ok(InterruptSnapshot {
            enabled: entry.enabled,
            pending,
            active: entry.list_state.is_some_and(ListState::active),
            listed: entry.list_state.is_some(),
            priority: entry.priority,
            group: entry.group,
            trigger: entry.trigger,
            target: entry.target,
        })
    }

    /// Replaces the saved hardware LR residency after a complete bank read.
    ///
    /// `slots` must contain exactly the LR count supplied to
    /// [`VirtualGicBuilder::finish`]. The entire snapshot is validated before
    /// state changes, and unpublished Assert/Clear commands are retained for
    /// the following [`Self::refill`] transaction.
    pub fn synchronize(
        &mut self,
        vcpu: VirtualCpuId,
        slots: &[Option<ListEntry>],
    ) -> Result<(), RuntimeError> {
        let validated = self.validate_snapshot_slots(vcpu, slots)?;
        let cpu = vcpu.get() as usize;
        let additions = self.deliveries[cpu]
            .listed
            .iter()
            .filter(|index| {
                let entry = &self.entries[index.0 as usize];
                entry.enabled
                    && entry.pending_command == PendingCommand::Assert
                    && entry.ready_position.is_none()
            })
            .count();
        if additions > self.deliveries[cpu].ready.remaining() {
            return Err(RuntimeError::CorruptState);
        }
        while let Some(index) = self.deliveries[cpu].listed.pop() {
            let entry = &mut self.entries[index.0 as usize];
            entry.list_state = None;
            entry.listed_position = None;
            self.reconcile_ready(index)?;
        }
        for (listed, index) in slots
            .iter()
            .zip(validated.indices)
            .filter_map(|(slot, index)| slot.as_ref().zip(index))
        {
            let entry = &mut self.entries[index.0 as usize];
            entry.list_state = Some(listed.state);
            if entry.list_state.is_some() {
                entry.listed_position = Some(self.deliveries[cpu].listed.len());
                self.deliveries[cpu]
                    .listed
                    .push(index)
                    .map_err(map_ready_runtime)?;
            }
            self.reconcile_ready(index)?;
        }
        Ok(())
    }

    /// Applies pending commands and fills a previously synchronized LR bank.
    ///
    /// The caller must pass the exact identity and `ListState` residency most
    /// recently synchronized with this controller. Metadata fields are
    /// refreshed by this method. A mismatched bank is rejected before either
    /// the caller's slice or controller state changes.
    pub fn refill(
        &mut self,
        vcpu: VirtualCpuId,
        slots: &mut [Option<ListEntry>],
    ) -> Result<usize, RuntimeError> {
        let validated = self.validate_refill_slots(vcpu, slots)?;
        let cpu = vcpu.get() as usize;
        for (slot, index) in slots.iter_mut().zip(validated.indices) {
            let Some(mut listed) = *slot else {
                continue;
            };
            let Some(index) = index else {
                continue;
            };
            let entry = &mut self.entries[index.0 as usize];
            let mut state = Some(listed.state);
            state = apply_pending_command(entry, state);
            state = apply_disabled_policy(entry, state);
            let Some(state) = state else {
                entry.list_state = None;
                entry.listed_position = None;
                *slot = None;
                self.reconcile_ready(index)?;
                continue;
            };
            entry.list_state = Some(state);
            listed.priority = entry.priority;
            listed.group = entry.group;
            listed.state = state;
            listed.request_eoi_maintenance = entry.maintenance_on_eoi || !entry.id.is_private();
            *slot = Some(listed);
            self.reconcile_ready(index)?;
        }
        self.rebuild_listed(cpu, &validated)?;
        let mut filled = 0;
        for slot in slots.iter_mut().filter(|slot| slot.is_none()) {
            let index = {
                let (entries, deliveries) = (&mut self.entries, &mut self.deliveries);
                deliveries[cpu].ready.pop(&mut EntryStore(entries))
            };
            let Some(index) = index else {
                break;
            };
            let entry = &mut self.entries[index.0 as usize];
            entry.pending_command = PendingCommand::None;
            let state = ListState::Pending;
            entry.list_state = Some(state);
            entry.listed_position = Some(self.deliveries[cpu].listed.len());
            self.deliveries[cpu]
                .listed
                .push(index)
                .map_err(map_ready_runtime)?;
            *slot = Some(ListEntry {
                interrupt: entry.id,
                priority: entry.priority,
                group: entry.group,
                state,
                request_eoi_maintenance: entry.maintenance_on_eoi || !entry.id.is_private(),
            });
            filled += 1;
        }
        Ok(filled)
    }

    fn rebuild_listed(&mut self, cpu: usize, slots: &ValidatedSlots) -> Result<(), RuntimeError> {
        for index in self.deliveries[cpu].listed.iter().copied() {
            self.entries[index.0 as usize].listed_position = None;
        }
        self.deliveries[cpu].listed.clear();
        for index in slots.indices.iter().flatten().copied() {
            if self.entries[index.0 as usize].list_state.is_none() {
                continue;
            }
            self.entries[index.0 as usize].listed_position =
                Some(self.deliveries[cpu].listed.len());
            self.deliveries[cpu]
                .listed
                .push(index)
                .map_err(map_ready_runtime)?;
        }
        Ok(())
    }

    fn reconcile_ready(&mut self, index: EntryIndex) -> Result<(), RuntimeError> {
        let entry = &self.entries[index.0 as usize];
        let should = entry.enabled
            && entry.pending_command == PendingCommand::Assert
            && entry.list_state.is_none();
        let present = entry.ready_position.is_some();
        let cpu = entry.target.get() as usize;
        match (present, should) {
            (false, true) => {
                let (entries, deliveries) = (&mut self.entries, &mut self.deliveries);
                deliveries[cpu]
                    .ready
                    .insert(index, &mut EntryStore(entries))
                    .map_err(map_ready_runtime)?;
            }
            (true, false) => {
                let (entries, deliveries) = (&mut self.entries, &mut self.deliveries);
                deliveries[cpu]
                    .ready
                    .remove(index, &mut EntryStore(entries));
            }
            _ => {}
        }
        Ok(())
    }

    fn preflight_ready_insert(&self, index: EntryIndex) -> Result<(), RuntimeError> {
        let entry = &self.entries[index.0 as usize];
        if entry.list_state.is_none() && entry.ready_position.is_none() {
            let cpu = entry.target.get() as usize;
            self.deliveries[cpu]
                .ready
                .can_insert()
                .map_err(map_ready_runtime)?;
        }
        Ok(())
    }

    fn validate_ready_position(&self, index: EntryIndex) -> Result<(), RuntimeError> {
        let cpu = self.entries[index.0 as usize].target.get() as usize;
        self.deliveries[cpu]
            .ready
            .contains(index, &EntryView(&self.entries))
            .map(|_| ())
            .map_err(map_ready_runtime)
    }

    fn validate_snapshot_slots(
        &self,
        vcpu: VirtualCpuId,
        slots: &[Option<ListEntry>],
    ) -> Result<ValidatedSlots, RuntimeError> {
        self.validate_cpu(vcpu)?;
        if slots.len() != self.list_register_count {
            return Err(RuntimeError::InvalidSlotCount);
        }
        self.validate_listed(vcpu.get() as usize)?;
        let mut seen = [0u64; 16];
        let mut indices = [None; MAX_LIST_REGISTERS];
        for (position, listed) in slots
            .iter()
            .enumerate()
            .filter_map(|(position, slot)| slot.as_ref().map(|listed| (position, listed)))
        {
            let id = listed.interrupt.get() as usize;
            let word = id / 64;
            let bit = 1u64 << (id % 64);
            if seen[word] & bit != 0 {
                return Err(RuntimeError::SnapshotContainsDuplicate);
            }
            seen[word] |= bit;
            indices[position] = Some(self.lookup(listed.interrupt, vcpu)?);
        }
        Ok(ValidatedSlots { indices })
    }

    fn validate_refill_slots(
        &self,
        vcpu: VirtualCpuId,
        slots: &[Option<ListEntry>],
    ) -> Result<ValidatedSlots, RuntimeError> {
        let validated = self.validate_snapshot_slots(vcpu, slots)?;
        let cpu = vcpu.get() as usize;
        let mut resident_count = 0;
        for (slot, index) in slots.iter().zip(validated.indices) {
            let Some(listed) = slot else {
                continue;
            };
            let Some(index) = index else {
                return Err(RuntimeError::ResidencyMismatch);
            };
            let entry = &self.entries[index.0 as usize];
            let Some(position) = entry.listed_position else {
                return Err(RuntimeError::ResidencyMismatch);
            };
            if self.deliveries[cpu].listed.get(position).copied() != Some(index)
                || entry.list_state != Some(listed.state)
            {
                return Err(RuntimeError::ResidencyMismatch);
            }
            resident_count += 1;
        }
        if resident_count != self.deliveries[cpu].listed.len() {
            return Err(RuntimeError::ResidencyMismatch);
        }
        Ok(validated)
    }

    fn validate_listed(&self, cpu: usize) -> Result<(), RuntimeError> {
        for (position, index) in self.deliveries[cpu].listed.iter().copied().enumerate() {
            let entry = self
                .entries
                .get(index.0 as usize)
                .ok_or(RuntimeError::CorruptState)?;
            if entry.target.get() as usize != cpu
                || entry.list_state.is_none()
                || entry.listed_position != Some(position)
                || self.deliveries[cpu]
                    .ready
                    .contains(index, &EntryView(&self.entries))
                    .map_err(map_ready_runtime)?
            {
                return Err(RuntimeError::CorruptState);
            }
        }
        Ok(())
    }

    fn lookup(
        &self,
        interrupt: GicInterruptId,
        target: VirtualCpuId,
    ) -> Result<EntryIndex, RuntimeError> {
        self.validate_cpu(target)?;
        let slot = if interrupt.is_private() {
            self.private
                .get(private_offset(target, interrupt).ok_or(RuntimeError::InvalidCpu)?)
        } else {
            self.shared.get(shared_offset(interrupt))
        }
        .and_then(|slot| *slot)
        .ok_or(RuntimeError::NotConfigured)?;
        let index = slot.index();
        let entry = self
            .entries
            .get(index.0 as usize)
            .ok_or(RuntimeError::CorruptState)?;
        if entry.target != target {
            return Err(RuntimeError::NotConfigured);
        }
        Ok(index)
    }

    fn lookup_shared(&self, interrupt: GicInterruptId) -> Result<EntryIndex, RuntimeError> {
        let slot = self
            .shared
            .get(shared_offset(interrupt))
            .and_then(|slot| *slot)
            .ok_or(RuntimeError::NotConfigured)?;
        let index = slot.index();
        if self.entries.get(index.0 as usize).is_some() {
            Ok(index)
        } else {
            Err(RuntimeError::CorruptState)
        }
    }

    fn entry(
        &self,
        interrupt: GicInterruptId,
        target: VirtualCpuId,
    ) -> Result<&Interrupt, RuntimeError> {
        let index = self.lookup(interrupt, target)?;
        self.entries
            .get(index.0 as usize)
            .ok_or(RuntimeError::CorruptState)
    }

    fn entry_mut(
        &mut self,
        interrupt: GicInterruptId,
        target: VirtualCpuId,
    ) -> Result<&mut Interrupt, RuntimeError> {
        let index = self.lookup(interrupt, target)?;
        self.entries
            .get_mut(index.0 as usize)
            .ok_or(RuntimeError::CorruptState)
    }

    fn validate_cpu(&self, cpu: VirtualCpuId) -> Result<(), RuntimeError> {
        if cpu.get() < self.vcpu_count {
            Ok(())
        } else {
            Err(RuntimeError::InvalidCpu)
        }
    }
}

struct EntryView<'a>(&'a [Interrupt]);

impl ReadyEntries for EntryView<'_> {
    fn rank(&self, index: EntryIndex) -> ReadyRank {
        let entry = &self.0[index.0 as usize];
        ReadyRank {
            priority: entry.priority,
            interrupt: entry.id.get(),
        }
    }

    fn position(&self, index: EntryIndex) -> Option<usize> {
        self.0[index.0 as usize].ready_position
    }

    fn set_position(&mut self, _index: EntryIndex, _position: Option<usize>) {}
}

fn empty_slots(length: usize) -> Result<Vec<Option<DirectorySlot>>, BuildError> {
    let mut slots = Vec::new();
    slots
        .try_reserve_exact(length)
        .map_err(|_| BuildError::Allocation)?;
    slots.resize(length, None);
    Ok(slots)
}

fn empty_usizes(length: usize) -> Result<Vec<usize>, BuildError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| BuildError::Allocation)?;
    values.resize(length, 0);
    Ok(values)
}

fn private_offset(cpu: VirtualCpuId, interrupt: GicInterruptId) -> Option<usize> {
    usize::try_from(cpu.get())
        .ok()?
        .checked_mul(PRIVATE_INTERRUPT_COUNT)?
        .checked_add(interrupt.get() as usize)
}

const fn shared_offset(interrupt: GicInterruptId) -> usize {
    interrupt.get() as usize - PRIVATE_INTERRUPT_COUNT
}

fn apply_pending_command(entry: &mut Interrupt, state: Option<ListState>) -> Option<ListState> {
    match entry.pending_command {
        PendingCommand::None => state,
        PendingCommand::Assert if entry.enabled => {
            let state = match state {
                None => None,
                Some(ListState::Active) => Some(ListState::PendingActive),
                pending @ Some(ListState::Pending | ListState::PendingActive) => pending,
            };
            if state.is_some_and(ListState::pending) {
                entry.pending_command = PendingCommand::None;
            }
            state
        }
        PendingCommand::Assert => state,
        PendingCommand::Clear => {
            entry.pending_command = PendingCommand::None;
            match state {
                Some(ListState::Pending) => None,
                Some(ListState::PendingActive) => Some(ListState::Active),
                other => other,
            }
        }
    }
}

fn apply_disabled_policy(entry: &mut Interrupt, state: Option<ListState>) -> Option<ListState> {
    if entry.enabled {
        return state;
    }
    match state {
        Some(ListState::Pending) => {
            entry.pending_command = PendingCommand::Assert;
            None
        }
        Some(ListState::PendingActive) => {
            entry.pending_command = PendingCommand::Assert;
            Some(ListState::Active)
        }
        other => other,
    }
}

const fn map_ready_build(error: ReadyError) -> BuildError {
    match error {
        ReadyError::Allocation => BuildError::Allocation,
        ReadyError::Capacity | ReadyError::CorruptPosition => BuildError::TooManyInterrupts,
    }
}

const fn map_ready_runtime(_error: ReadyError) -> RuntimeError {
    RuntimeError::CorruptState
}
