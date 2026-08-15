pub mod chosen;
pub mod fdt;

pub const MAX_MEMORY_REGIONS: usize = 8;
pub const MAX_RESERVED_REGIONS: usize = 32;
pub const MAX_MMIO_REGIONS: usize = 64;
pub const MAX_NO_MAP_REGIONS: usize = 32;
pub const MAX_GIC_REDISTRIBUTOR_REGIONS: usize = 4;
pub const MAX_CPUS: usize = crate::config::MAX_CPUS as usize;

/// A half-open physical address interval `[start, start + size)`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalRange {
    start: u64,
    size: u64,
}

impl PhysicalRange {
    pub const EMPTY: Self = Self { start: 0, size: 0 };

    pub const fn new(start: u64, size: u64) -> Option<Self> {
        if size == 0 || start.checked_add(size).is_none() {
            None
        } else {
            Some(Self { start, size })
        }
    }

    pub const fn end(self) -> u64 {
        // Construction guarantees that this addition cannot overflow.
        self.start + self.size
    }

    pub const fn start(self) -> u64 {
        self.start
    }

    pub const fn size(self) -> u64 {
        self.size
    }

    pub const fn overlaps(self, other: Self) -> bool {
        self.start < other.end() && other.start < self.end()
    }
}

/// Allocation-free region storage used before the heap exists.
#[derive(Clone, Copy, Debug)]
pub struct RegionList<const CAPACITY: usize> {
    entries: [PhysicalRange; CAPACITY],
    length: usize,
}

impl<const CAPACITY: usize> RegionList<CAPACITY> {
    pub const fn new() -> Self {
        Self {
            entries: [PhysicalRange::EMPTY; CAPACITY],
            length: 0,
        }
    }

    pub fn as_slice(&self) -> &[PhysicalRange] {
        &self.entries[..self.length]
    }

    pub const fn is_empty(&self) -> bool {
        self.length == 0
    }

    pub const fn len(&self) -> usize {
        self.length
    }

    /// Inserts a range and coalesces overlapping or adjacent entries.
    pub fn insert(&mut self, mut range: PhysicalRange) -> Result<(), CapacityError> {
        if range.size == 0 || range.start.checked_add(range.size).is_none() {
            return Err(CapacityError);
        }
        let mut index = 0;
        while index < self.length {
            let current = self.entries[index];
            if range.start <= current.end() && current.start <= range.end() {
                let start = range.start.min(current.start);
                let end = range.end().max(current.end());
                range = PhysicalRange::new(start, end - start).ok_or(CapacityError)?;
                self.remove(index);
            } else {
                index += 1;
            }
        }

        if self.length == CAPACITY {
            return Err(CapacityError);
        }
        self.entries[self.length] = range;
        self.length += 1;
        self.entries[..self.length].sort_unstable_by_key(|entry| entry.start);
        Ok(())
    }

    fn remove(&mut self, index: usize) {
        self.entries.copy_within(index + 1..self.length, index);
        self.length -= 1;
        self.entries[self.length] = PhysicalRange::EMPTY;
    }
}

impl<const CAPACITY: usize> Default for RegionList<CAPACITY> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapacityError;

/// Hardware facts discovered from firmware-provided platform data.
#[derive(Clone, Copy, Debug)]
pub struct PlatformInfo {
    pub cpus: CpuList,
    pub memory: RegionList<MAX_MEMORY_REGIONS>,
    pub reserved: RegionList<MAX_RESERVED_REGIONS>,
    pub no_map: RegionList<MAX_NO_MAP_REGIONS>,
    pub mmio: RegionList<MAX_MMIO_REGIONS>,
    pub dtb_size: u64,
}

/// Firmware-discovered processing element.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CpuInfo {
    pub hardware_id: u64,
}

/// Fixed-capacity CPU topology available before the heap is initialized.
#[derive(Clone, Copy, Debug)]
pub struct CpuList {
    entries: [CpuInfo; MAX_CPUS],
    length: usize,
}

impl CpuList {
    pub const fn new() -> Self {
        Self {
            entries: [CpuInfo { hardware_id: 0 }; MAX_CPUS],
            length: 0,
        }
    }

    pub fn as_slice(&self) -> &[CpuInfo] {
        &self.entries[..self.length]
    }

    pub const fn len(&self) -> usize {
        self.length
    }

    pub const fn is_empty(&self) -> bool {
        self.length == 0
    }

    pub fn push(&mut self, cpu: CpuInfo) -> Result<(), CpuListError> {
        if self.entries[..self.length]
            .iter()
            .any(|entry| entry.hardware_id == cpu.hardware_id)
        {
            return Err(CpuListError::Duplicate);
        }
        if self.length == MAX_CPUS {
            return Err(CpuListError::Capacity);
        }
        self.entries[self.length] = cpu;
        self.length += 1;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CpuListError {
    Capacity,
    Duplicate,
}

impl Default for CpuList {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CpuPowerInfo {
    Psci(PsciInfo),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PsciInfo {
    pub conduit: PsciConduit,
    pub compatible_version: PsciCompatibleVersion,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PsciConduit {
    Smc,
    Hvc,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PsciCompatibleVersion {
    V0_2,
    V1_0,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimerInfo {
    pub kind: TimerKind,
    pub virtual_timer: PlatformInterrupt,
    pub hypervisor_physical: PlatformInterrupt,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimerKind {
    ArmGeneric,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlatformInterruptTrigger {
    Level,
    Edge,
}

#[derive(Clone, Copy, Debug)]
pub enum InterruptControllerInfo {
    GicV3(GicV3Info),
}

#[derive(Clone, Copy, Debug)]
pub struct GicV3Info {
    pub distributor: PhysicalRange,
    pub redistributors: RegionList<MAX_GIC_REDISTRIBUTOR_REGIONS>,
    pub redistributor_stride: Option<u64>,
    pub maintenance_interrupt: Option<PlatformInterrupt>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlatformInterrupt {
    pub interrupt: u32,
    pub trigger: PlatformInterruptTrigger,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConsoleInfo {
    pub kind: ConsoleKind,
    pub base: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConsoleKind {
    Pl011,
}
