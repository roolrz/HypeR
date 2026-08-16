use hyper::platform::{
    CpuPowerInfo, InterruptControllerInfo, MAX_PLIC_CONTEXTS, PhysicalRange, PlatformInterrupt,
    PlatformInterruptTrigger, PlicInfo, SbiInfo, TimerInfo, TimerKind,
    fdt::{NodeId, NodeResources, NodeVisitor},
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
    };
}

pub struct EssentialDeviceDiscovery {
    nodes: [Candidate; MAX_DEPTH],
    depth: usize,
    plic: Option<(NodeId, PhysicalRange, u32)>,
    timebase_frequency: Option<u64>,
    enabled_cpu_count: usize,
    error: Option<Error>,
}

impl EssentialDeviceDiscovery {
    pub const fn new() -> Self {
        Self {
            nodes: [Candidate::EMPTY; MAX_DEPTH],
            depth: 0,
            plic: None,
            timebase_frequency: None,
            enabled_cpu_count: 0,
            error: None,
        }
    }

    pub fn finish(self) -> Result<EssentialPlatformInfo, Error> {
        if let Some(error) = self.error {
            return Err(error);
        }
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
    fn begin_node(&mut self, _id: NodeId, _name: &str) {
        if self.depth == MAX_DEPTH {
            self.error = Some(Error::InvalidDepth);
            return;
        }
        self.nodes[self.depth] = Candidate::EMPTY;
        self.depth += 1;
    }

    fn property(&mut self, _id: NodeId, name: &str, value: &[u8]) {
        let Some(candidate) = self
            .depth
            .checked_sub(1)
            .and_then(|i| self.nodes.get_mut(i))
        else {
            self.error = Some(Error::InvalidDepth);
            return;
        };
        match name {
            "compatible" => {
                candidate.plic = string_list_contains(value, "sifive,plic-1.0.0")
                    || string_list_contains(value, "riscv,plic0");
            }
            "riscv,ndev" => match read_u32(value) {
                Ok(value) => candidate.source_count = Some(value),
                Err(error) => self.error = Some(error),
            },
            "device_type" => candidate.cpu = c_string_equals(value, "cpu"),
            "timebase-frequency" => match read_integer(value) {
                Ok(value) => self.timebase_frequency = Some(value),
                Err(error) => self.error = Some(error),
            },
            "riscv,isa" => {
                let base = value
                    .split(|byte| *byte == b'_' || *byte == 0)
                    .next()
                    .unwrap_or(&[]);
                candidate.hypervisor_extension |= base.contains(&b'h');
                candidate.single_precision |= base.contains(&b'f');
                candidate.double_precision |= base.contains(&b'd');
                candidate.supervisor_timer_compare |= value
                    .split(|byte| *byte == b'_')
                    .any(|extension| extension.starts_with(b"sstc"));
            }
            "riscv,isa-extensions" => {
                candidate.supervisor_timer_compare |= string_list_contains(value, "sstc");
                candidate.hypervisor_extension |= string_list_contains(value, "h");
                candidate.single_precision |= string_list_contains(value, "f");
                candidate.double_precision |= string_list_contains(value, "d");
            }
            _ => {}
        }
    }

    fn end_node(&mut self, node: NodeResources<'_>) {
        let Some(index) = self.depth.checked_sub(1) else {
            self.error = Some(Error::InvalidDepth);
            return;
        };
        let candidate = self.nodes[index];
        self.depth = index;
        if self.error.is_some() || !node.enabled {
            return;
        }
        if candidate.cpu {
            if !candidate.supervisor_timer_compare {
                self.error = Some(Error::MissingSstc);
                return;
            }
            if !candidate.hypervisor_extension
                || !candidate.single_precision
                || !candidate.double_precision
            {
                self.error = Some(Error::MissingRequiredIsa);
                return;
            }
            self.enabled_cpu_count = self.enabled_cpu_count.saturating_add(1);
        }
        if !candidate.plic || self.plic.is_some() {
            return;
        }
        let Some(registers) = node.registers.first().copied() else {
            self.error = Some(Error::InvalidPlic);
            return;
        };
        let Some(source_count) = candidate.source_count else {
            self.error = Some(Error::InvalidPlic);
            return;
        };
        self.plic = Some((node.id, registers, source_count));
    }
}

pub fn decode_platform_interrupt(descriptor: &[u32]) -> Result<PlatformInterrupt, Error> {
    let interrupt = *descriptor.first().ok_or(Error::InvalidProperty)?;
    Ok(PlatformInterrupt {
        interrupt,
        trigger: PlatformInterruptTrigger::Level,
    })
}

fn string_list_contains(value: &[u8], expected: &str) -> bool {
    value
        .split(|byte| *byte == 0)
        .any(|item| item == expected.as_bytes())
}

fn c_string_equals(value: &[u8], expected: &str) -> bool {
    value.split(|byte| *byte == 0).next() == Some(expected.as_bytes())
}

fn read_u32(value: &[u8]) -> Result<u32, Error> {
    let bytes: [u8; 4] = value
        .get(..4)
        .ok_or(Error::InvalidProperty)?
        .try_into()
        .map_err(|_| Error::InvalidProperty)?;
    Ok(u32::from_be_bytes(bytes))
}

fn read_integer(value: &[u8]) -> Result<u64, Error> {
    match value.len() {
        4 => Ok(u64::from(read_u32(value)?)),
        8 => {
            let bytes: [u8; 8] = value.try_into().map_err(|_| Error::InvalidProperty)?;
            Ok(u64::from_be_bytes(bytes))
        }
        _ => Err(Error::InvalidProperty),
    }
}
