use hyper::platform::{
    CpuPowerInfo, InterruptControllerInfo, MAX_PLIC_CONTEXTS, PhysicalRange, PlatformInterrupt,
    PlatformInterruptTrigger, PlicInfo, SbiInfo, TimerInfo, TimerKind,
    fdt::{NodeId, NodeResources, NodeVisitor, Property},
};

const MAX_DEPTH: usize = 32;
const MAX_CLAIMS: usize = 8;
const SUPERVISOR_TIMER_INTERRUPT: u32 = 0;

#[derive(Clone, Copy, Debug)]
pub struct EssentialPlatformInfo {
    pub cpu_power: Option<CpuPowerInfo>,
    pub interrupt_controller: Option<InterruptControllerInfo>,
    pub timer: Option<TimerInfo>,
    pub timebase_frequency: u64,
    pub cache_block_size: usize,
    claims: [Option<NodeId>; MAX_CLAIMS],
    claim_count: usize,
}

impl EssentialPlatformInfo {
    pub fn claims(&self) -> &[Option<NodeId>] {
        &self.claims[..self.claim_count]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidDepth,
    InvalidPlic,
    InvalidProperty,
    MissingRequiredIsa,
    MissingPlic,
    MissingSstc,
    MissingTimebase,
    MissingZicbom,
    InvalidCacheBlockSize,
    InconsistentCacheBlockSize,
}

#[derive(Clone, Copy)]
struct Candidate {
    plic: bool,
    cpu: bool,
    source_count: Option<u32>,
    supervisor_timer_compare: bool,
    hypervisor_extension: bool,
    single_precision: bool,
    double_precision: bool,
    cache_block_management: bool,
    cache_block_size: Option<u32>,
}

impl Candidate {
    const EMPTY: Self = Self {
        plic: false,
        cpu: false,
        source_count: None,
        supervisor_timer_compare: false,
        hypervisor_extension: false,
        single_precision: false,
        double_precision: false,
        cache_block_management: false,
        cache_block_size: None,
    };
}

pub struct EssentialDeviceDiscovery {
    nodes: [Candidate; MAX_DEPTH],
    depth: usize,
    plic: Option<(NodeId, PhysicalRange, u32)>,
    timebase_frequency: Option<u64>,
    enabled_cpu_count: usize,
    cache_block_size: Option<u32>,
}

impl EssentialDeviceDiscovery {
    pub const fn new() -> Self {
        Self {
            nodes: [Candidate::EMPTY; MAX_DEPTH],
            depth: 0,
            plic: None,
            timebase_frequency: None,
            enabled_cpu_count: 0,
            cache_block_size: None,
        }
    }

    pub fn finish(self) -> Result<EssentialPlatformInfo, Error> {
        if self.depth != 0 {
            return Err(Error::InvalidDepth);
        }
        let (plic_node, registers, source_count) = self.plic.ok_or(Error::MissingPlic)?;
        let frequency = self.timebase_frequency.ok_or(Error::MissingTimebase)?;
        if self.enabled_cpu_count == 0 {
            return Err(Error::MissingRequiredIsa);
        }
        let mut contexts = [0; MAX_PLIC_CONTEXTS];
        for (cpu, context) in contexts.iter_mut().enumerate() {
            // QEMU virt and the legacy SiFive PLIC binding enumerate an
            // M-mode context followed by an S-mode context for every hart.
            *context = (cpu as u32).saturating_mul(2).saturating_add(1);
        }
        Ok(EssentialPlatformInfo {
            cpu_power: Some(CpuPowerInfo::Sbi(SbiInfo)),
            interrupt_controller: Some(InterruptControllerInfo::Plic(PlicInfo {
                registers,
                source_count,
                supervisor_contexts: contexts,
                context_count: MAX_PLIC_CONTEXTS,
            })),
            timer: Some(TimerInfo {
                kind: TimerKind::RiscvSupervisor,
                virtual_timer: PlatformInterrupt {
                    interrupt: SUPERVISOR_TIMER_INTERRUPT,
                    trigger: PlatformInterruptTrigger::Level,
                },
                hypervisor_physical: PlatformInterrupt {
                    interrupt: SUPERVISOR_TIMER_INTERRUPT,
                    trigger: PlatformInterruptTrigger::Level,
                },
            }),
            timebase_frequency: frequency,
            cache_block_size: usize::try_from(
                self.cache_block_size.ok_or(Error::InvalidCacheBlockSize)?,
            )
            .map_err(|_| Error::InvalidCacheBlockSize)?,
            claims: [Some(plic_node), None, None, None, None, None, None, None],
            claim_count: 1,
        })
    }
}

impl Default for EssentialDeviceDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeVisitor for EssentialDeviceDiscovery {
    type Error = Error;

    fn begin_node(&mut self, _id: NodeId, _name: &str) -> Result<(), Self::Error> {
        if self.depth == MAX_DEPTH {
            return Err(Error::InvalidDepth);
        }
        self.nodes[self.depth] = Candidate::EMPTY;
        self.depth += 1;
        Ok(())
    }

    fn property(&mut self, _id: NodeId, property: Property<'_>) -> Result<(), Self::Error> {
        let candidate = self
            .depth
            .checked_sub(1)
            .and_then(|i| self.nodes.get_mut(i))
            .ok_or(Error::InvalidDepth)?;
        match property.name() {
            "compatible" => {
                candidate.plic = property
                    .contains_string("sifive,plic-1.0.0")
                    .map_err(|_| Error::InvalidProperty)?
                    || property
                        .contains_string("riscv,plic0")
                        .map_err(|_| Error::InvalidProperty)?;
            }
            "riscv,ndev" => {
                candidate.source_count = Some(property.u32().map_err(|_| Error::InvalidProperty)?)
            }
            "device_type" => {
                candidate.cpu = property.string().map_err(|_| Error::InvalidProperty)? == "cpu"
            }
            "timebase-frequency" => {
                self.timebase_frequency =
                    Some(property.integer().map_err(|_| Error::InvalidProperty)?)
            }
            "riscv,isa" => {
                let value = property.string().map_err(|_| Error::InvalidProperty)?;
                let base = value
                    .as_bytes()
                    .split(|byte| *byte == b'_' || *byte == 0)
                    .next()
                    .unwrap_or(&[]);
                candidate.hypervisor_extension |= base.contains(&b'h');
                candidate.single_precision |= base.contains(&b'f');
                candidate.double_precision |= base.contains(&b'd');
                candidate.supervisor_timer_compare |= value
                    .as_bytes()
                    .split(|byte| *byte == b'_' || *byte == 0)
                    .any(|extension| extension.starts_with(b"sstc"));
                candidate.cache_block_management |= value
                    .as_bytes()
                    .split(|byte| *byte == b'_' || *byte == 0)
                    .any(|extension| extension.starts_with(b"zicbom"));
            }
            "riscv,isa-extensions" => {
                candidate.supervisor_timer_compare |= property
                    .contains_string("sstc")
                    .map_err(|_| Error::InvalidProperty)?;
                candidate.hypervisor_extension |= property
                    .contains_string("h")
                    .map_err(|_| Error::InvalidProperty)?;
                candidate.single_precision |= property
                    .contains_string("f")
                    .map_err(|_| Error::InvalidProperty)?;
                candidate.double_precision |= property
                    .contains_string("d")
                    .map_err(|_| Error::InvalidProperty)?;
                candidate.cache_block_management |= property
                    .contains_string("zicbom")
                    .map_err(|_| Error::InvalidProperty)?;
            }
            "riscv,cbom-block-size" => {
                candidate.cache_block_size =
                    Some(property.u32().map_err(|_| Error::InvalidProperty)?)
            }
            _ => {}
        }
        Ok(())
    }

    fn end_node(&mut self, node: NodeResources<'_>) -> Result<(), Self::Error> {
        let index = self.depth.checked_sub(1).ok_or(Error::InvalidDepth)?;
        let candidate = self.nodes[index];
        self.depth = index;
        if !node.enabled {
            return Ok(());
        }
        if candidate.cpu {
            if !candidate.supervisor_timer_compare {
                return Err(Error::MissingSstc);
            }
            if !candidate.hypervisor_extension
                || !candidate.single_precision
                || !candidate.double_precision
            {
                return Err(Error::MissingRequiredIsa);
            }
            if !candidate.cache_block_management {
                return Err(Error::MissingZicbom);
            }
            let block_size = candidate
                .cache_block_size
                .ok_or(Error::InvalidCacheBlockSize)?;
            if block_size == 0 || !block_size.is_power_of_two() {
                return Err(Error::InvalidCacheBlockSize);
            }
            match self.cache_block_size {
                Some(previous) if previous != block_size => {
                    return Err(Error::InconsistentCacheBlockSize);
                }
                Some(_) => {}
                None => self.cache_block_size = Some(block_size),
            }
            self.enabled_cpu_count = self.enabled_cpu_count.saturating_add(1);
        }
        if !candidate.plic || self.plic.is_some() {
            return Ok(());
        }
        let registers = node.registers.first().copied().ok_or(Error::InvalidPlic)?;
        let source_count = candidate.source_count.ok_or(Error::InvalidPlic)?;
        self.plic = Some((node.id, registers, source_count));
        Ok(())
    }
}

pub fn decode_platform_interrupt(descriptor: &[u32]) -> Result<PlatformInterrupt, Error> {
    let interrupt = *descriptor.first().ok_or(Error::InvalidProperty)?;
    Ok(PlatformInterrupt {
        interrupt,
        trigger: PlatformInterruptTrigger::Level,
    })
}
