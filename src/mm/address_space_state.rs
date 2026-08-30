// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Lock-protected admission and residency state for immutable machine roots.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResidencyError {
    Busy,
    AlreadyActive,
    InvalidCpu,
    NotActive,
    StaleEpoch,
    SequenceExhausted,
}

pub struct AddressSpaceResidency<const CPUS: usize> {
    epoch: u64,
    updating: bool,
    active: [bool; CPUS],
    resident: [bool; CPUS],
    update_sequence: u64,
}

impl<const CPUS: usize> AddressSpaceResidency<CPUS> {
    pub const fn new(epoch: u64) -> Self {
        Self {
            epoch,
            updating: false,
            active: [false; CPUS],
            resident: [false; CPUS],
            update_sequence: 0,
        }
    }

    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn check_admission(&self, cpu: usize, epoch: u64) -> Result<(), ResidencyError> {
        if cpu >= CPUS {
            return Err(ResidencyError::InvalidCpu);
        }
        if self.updating {
            return Err(ResidencyError::Busy);
        }
        if epoch != self.epoch {
            return Err(ResidencyError::StaleEpoch);
        }
        if self.active[cpu] {
            return Err(ResidencyError::AlreadyActive);
        }
        Ok(())
    }

    /// Publishes an admission after the caller installed the root while still
    /// holding the same external lock used for [`Self::check_admission`].
    pub fn publish_admission(&mut self, cpu: usize) -> Result<(), ResidencyError> {
        if cpu >= CPUS {
            return Err(ResidencyError::InvalidCpu);
        }
        if self.updating {
            return Err(ResidencyError::Busy);
        }
        if self.active[cpu] {
            return Err(ResidencyError::AlreadyActive);
        }
        self.active[cpu] = true;
        self.resident[cpu] = true;
        Ok(())
    }

    /// Removes one active CPU only after the update gate is open and the
    /// caller's observed machine epoch is current.
    pub fn leave(&mut self, cpu: usize, expected_epoch: u64) -> Result<(), ResidencyError> {
        if self.updating {
            return Err(ResidencyError::Busy);
        }
        if expected_epoch != self.epoch {
            return Err(ResidencyError::StaleEpoch);
        }
        let active = self.active.get_mut(cpu).ok_or(ResidencyError::InvalidCpu)?;
        if !*active {
            return Err(ResidencyError::NotActive);
        }
        *active = false;
        Ok(())
    }

    pub fn begin_update(
        &mut self,
        expected_epoch: u64,
    ) -> Result<ResidencyCut<CPUS>, ResidencyError> {
        if self.updating {
            return Err(ResidencyError::Busy);
        }
        if self.epoch != expected_epoch {
            return Err(ResidencyError::StaleEpoch);
        }
        let sequence = self
            .update_sequence
            .checked_add(1)
            .ok_or(ResidencyError::SequenceExhausted)?;
        self.update_sequence = sequence;
        self.updating = true;
        Ok(ResidencyCut {
            base_epoch: self.epoch,
            sequence,
            active: self.active,
            targets: self.resident,
        })
    }

    /// Permanently closes admission while the owner retires every resident
    /// translation. The surrounding address-space owner is consumed, so no
    /// corresponding reopen transition exists.
    pub fn begin_retirement(
        &mut self,
        expected_epoch: u64,
    ) -> Result<ResidencyCut<CPUS>, ResidencyError> {
        if self.active.iter().copied().any(|active| active) {
            return Err(ResidencyError::Busy);
        }
        self.begin_update(expected_epoch)
    }

    pub fn abort_update(&mut self, cut: ResidencyCut<CPUS>) -> Result<(), ResidencyError> {
        self.validate_cut(&cut)?;
        self.updating = false;
        Ok(())
    }

    /// Opens admission for the new epoch after every cut target acknowledged.
    pub fn finish_update(
        &mut self,
        cut: ResidencyCut<CPUS>,
        epoch: u64,
    ) -> Result<(), ResidencyError> {
        self.validate_cut(&cut)?;
        if epoch <= self.epoch {
            return Err(ResidencyError::StaleEpoch);
        }
        self.epoch = epoch;
        // Inactive residents were invalidated by the cut. Active targets have
        // installed the new root and remain resident.
        self.resident = self.active;
        self.updating = false;
        Ok(())
    }

    fn validate_cut(&self, cut: &ResidencyCut<CPUS>) -> Result<(), ResidencyError> {
        if !self.updating || cut.base_epoch != self.epoch || cut.sequence != self.update_sequence {
            return Err(ResidencyError::StaleEpoch);
        }
        Ok(())
    }
}

pub struct ResidencyCut<const CPUS: usize> {
    base_epoch: u64,
    sequence: u64,
    active: [bool; CPUS],
    targets: [bool; CPUS],
}

impl<const CPUS: usize> ResidencyCut<CPUS> {
    pub const fn active(&self) -> &[bool; CPUS] {
        &self.active
    }

    pub const fn targets(&self) -> &[bool; CPUS] {
        &self.targets
    }

    pub const fn base_epoch(&self) -> u64 {
        self.base_epoch
    }
}
