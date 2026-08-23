// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Static scheduling classes and thread placement vocabulary.
//!
//! This module describes policy inputs without owning runnable queues or
//! performing context switches. The initial closed policy set contains a
//! cooperative fixed-priority FIFO class plus a non-runnable idle class. Placement
//! records current assignment separately from affinity and placement policy so
//! a future stopped-thread handoff can move a Thread without changing identity.

use hyper::cpu::{CpuIndex, MAX_CPUS};

const CPUS_PER_MASK_WORD: usize = u64::BITS as usize;
const CPU_MASK_WORDS: usize = MAX_CPUS.div_ceil(CPUS_PER_MASK_WORD);

/// The scheduler accepts the complete `u8` priority namespace.
pub const PRIORITY_LEVELS: usize = u8::MAX as usize + 1;

/// Fixed scheduler priority. Lower numeric values run first.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ThreadPriority(u8);

impl ThreadPriority {
    pub const HIGHEST: Self = Self(0);
    pub const NORMAL: Self = Self(128);
    pub const LOWEST: Self = Self(u8::MAX);

    pub const fn new(value: u8) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

/// Closed set of scheduling classes supported by this kernel image.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchedulingClass {
    /// Fixed-priority runnable policies.
    FixedPriority,
    /// Per-CPU fallback execution which is never inserted into a run queue.
    Idle,
}

/// Class-specific scheduling parameters.
///
/// Encoding Idle as a separate variant prevents assigning a meaningless
/// fixed priority to an idle thread.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchedulingPolicy {
    /// Cooperative fixed-priority FIFO scheduling.
    ///
    /// A higher-priority ready thread becomes eligible immediately, but the
    /// switch occurs only at an explicit safe point. Equal-priority wakeups do
    /// not request a switch.
    /// A priority change preserves position when unchanged, enters the tail of
    /// a raised-priority queue, and enters the head of a lowered-priority queue
    /// at the next scheduling decision.
    Fifo {
        priority: ThreadPriority,
    },
    Idle,
}

impl SchedulingPolicy {
    pub const fn fifo(priority: ThreadPriority) -> Self {
        Self::Fifo { priority }
    }

    pub const fn class(self) -> SchedulingClass {
        match self {
            Self::Fifo { .. } => SchedulingClass::FixedPriority,
            Self::Idle => SchedulingClass::Idle,
        }
    }

    pub const fn priority(self) -> Option<ThreadPriority> {
        match self {
            Self::Fifo { priority } => Some(priority),
            Self::Idle => None,
        }
    }

    /// Decides whether a newly ready FIFO thread outranks `self`.
    pub const fn is_preempted_by(self, candidate: Self) -> bool {
        match (self, candidate) {
            (Self::Idle, Self::Fifo { .. }) => true,
            (
                Self::Fifo { priority: current },
                Self::Fifo {
                    priority: candidate,
                },
            ) => candidate.0 < current.0,
            (_, Self::Idle) => false,
        }
    }
}

/// CPUs on which a stopped thread is permitted to run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CpuMask {
    words: [u64; CPU_MASK_WORDS],
}

impl CpuMask {
    pub const EMPTY: Self = Self {
        words: [0; CPU_MASK_WORDS],
    };

    pub const ALL: Self = Self::all();

    pub const fn single(cpu: CpuIndex) -> Self {
        Self::EMPTY.with_cpu(cpu)
    }

    pub const fn contains(self, cpu: CpuIndex) -> bool {
        let index = cpu.get();
        self.words[index / CPUS_PER_MASK_WORD] & (1u64 << (index % CPUS_PER_MASK_WORD)) != 0
    }

    pub const fn is_empty(self) -> bool {
        let mut word = 0;
        while word < CPU_MASK_WORDS {
            if self.words[word] != 0 {
                return false;
            }
            word += 1;
        }
        true
    }

    /// Adds one allowed CPU and returns the enlarged immutable value.
    pub const fn with_cpu(mut self, cpu: CpuIndex) -> Self {
        let index = cpu.get();
        self.words[index / CPUS_PER_MASK_WORD] |= 1u64 << (index % CPUS_PER_MASK_WORD);
        self
    }

    /// Removes one allowed CPU and returns the reduced immutable value.
    pub const fn without_cpu(mut self, cpu: CpuIndex) -> Self {
        let index = cpu.get();
        self.words[index / CPUS_PER_MASK_WORD] &= !(1u64 << (index % CPUS_PER_MASK_WORD));
        self
    }

    const fn all() -> Self {
        let mut mask = Self {
            words: [u64::MAX; CPU_MASK_WORDS],
        };
        let remainder = MAX_CPUS % CPUS_PER_MASK_WORD;
        if remainder != 0 {
            mask.words[CPU_MASK_WORDS - 1] = (1u64 << remainder) - 1;
        }
        mask
    }
}

/// CPU-selection constraints retained independently of current assignment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlacementPolicy {
    Movable,
    Prefer(CpuIndex),
    Pinned(CpuIndex),
}

/// Scheduler assignment of a thread within its permitted CPU affinity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ThreadPlacement {
    assigned_cpu: CpuIndex,
    affinity: CpuMask,
    policy: PlacementPolicy,
    last_cpu: Option<CpuIndex>,
}

impl ThreadPlacement {
    /// Assigns a thread to one CPU admitted by `affinity`.
    ///
    /// Constructing placement does not migrate a live Thread. Scheduler code
    /// may install a different value only under a future stopped-thread
    /// handoff protocol.
    #[cfg(test)]
    pub const fn new(
        assigned_cpu: CpuIndex,
        affinity: CpuMask,
        policy: PlacementPolicy,
    ) -> Option<Self> {
        let policy_valid = match policy {
            PlacementPolicy::Movable => true,
            PlacementPolicy::Prefer(cpu) => affinity.contains(cpu),
            PlacementPolicy::Pinned(cpu) => {
                affinity.contains(cpu) && assigned_cpu.get() == cpu.get()
            }
        };
        if affinity.contains(assigned_cpu) && policy_valid {
            Some(Self {
                assigned_cpu,
                affinity,
                policy,
                last_cpu: None,
            })
        } else {
            None
        }
    }

    /// Creates the non-migrating placement used by the current scheduler.
    pub const fn pinned(cpu: CpuIndex) -> Self {
        Self {
            assigned_cpu: cpu,
            affinity: CpuMask::single(cpu),
            policy: PlacementPolicy::Pinned(cpu),
            last_cpu: Some(cpu),
        }
    }

    /// Creates an initially assigned but migration-capable placement.
    #[cfg(test)]
    pub const fn movable(cpu: CpuIndex) -> Self {
        Self {
            assigned_cpu: cpu,
            affinity: CpuMask::ALL,
            policy: PlacementPolicy::Movable,
            last_cpu: None,
        }
    }

    /// Creates a migration-capable placement constrained by `affinity`.
    pub const fn movable_with_affinity(cpu: CpuIndex, affinity: CpuMask) -> Option<Self> {
        if affinity.contains(cpu) {
            Some(Self {
                assigned_cpu: cpu,
                affinity,
                policy: PlacementPolicy::Movable,
                last_cpu: None,
            })
        } else {
            None
        }
    }

    /// Creates an initially assigned placement which prefers its creating CPU.
    pub const fn prefer(cpu: CpuIndex) -> Self {
        Self {
            assigned_cpu: cpu,
            affinity: CpuMask::ALL,
            policy: PlacementPolicy::Prefer(cpu),
            last_cpu: None,
        }
    }

    pub const fn assigned_cpu(self) -> CpuIndex {
        self.assigned_cpu
    }

    pub const fn affinity(self) -> CpuMask {
        self.affinity
    }

    #[cfg(test)]
    pub const fn policy(self) -> PlacementPolicy {
        self.policy
    }

    #[cfg(test)]
    pub const fn last_cpu(self) -> Option<CpuIndex> {
        self.last_cpu
    }

    pub(crate) const fn mark_running(mut self, cpu: CpuIndex) -> Option<Self> {
        if self.assigned_cpu.get() != cpu.get() || !self.affinity.contains(cpu) {
            return None;
        }
        self.last_cpu = Some(cpu);
        Some(self)
    }
}
