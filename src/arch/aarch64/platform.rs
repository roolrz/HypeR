use hyper::platform::{
    ConsoleInfo, ConsoleKind, CpuPowerInfo, GicV3Info, InterruptControllerInfo,
    MAX_GIC_REDISTRIBUTOR_REGIONS, PhysicalRange, PlatformInterruptTrigger, PsciCompatibleVersion,
    PsciConduit, PsciInfo, RegionList, TimerInfo, TimerKind,
    fdt::{NodeId, NodeResources, NodeVisitor},
};

const MAX_FDT_DEPTH: usize = 32;
const MAX_EARLY_CLAIMS: usize = 8;

#[derive(Clone, Copy, Debug)]
pub struct EssentialPlatformInfo {
    pub console: Option<ConsoleInfo>,
    pub cpu_power: Option<CpuPowerInfo>,
    pub interrupt_controller: Option<InterruptControllerInfo>,
    pub timer: Option<TimerInfo>,
    claims: [Option<NodeId>; MAX_EARLY_CLAIMS],
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
    InvalidGic,
    InvalidInterrupt,
    InvalidProperty,
    InvalidPsci,
    TooManyClaims,
}

#[derive(Clone, Copy)]
struct Candidate {
    pl011: bool,
    gic_v3: bool,
    timer: bool,
    psci_legacy: bool,
    psci_version: Option<PsciCompatibleVersion>,
    psci_conduit: Option<PsciConduit>,
    redistributor_regions: u32,
    redistributor_stride: Option<u64>,
}

impl Candidate {
    const EMPTY: Self = Self {
        pl011: false,
        gic_v3: false,
        timer: false,
        psci_legacy: false,
        psci_version: None,
        psci_conduit: None,
        redistributor_regions: 1,
        redistributor_stride: None,
    };
}

/// Allocation-free AArch64 matching for devices required before driver probing.
pub struct EssentialDeviceDiscovery {
    nodes: [Candidate; MAX_FDT_DEPTH],
    depth: usize,
    result: EssentialPlatformInfo,
    error: Option<Error>,
}

impl EssentialDeviceDiscovery {
    pub const fn new() -> Self {
        Self {
            nodes: [Candidate::EMPTY; MAX_FDT_DEPTH],
            depth: 0,
            result: EssentialPlatformInfo {
                console: None,
                cpu_power: None,
                interrupt_controller: None,
                timer: None,
                claims: [None; MAX_EARLY_CLAIMS],
                claim_count: 0,
            },
            error: None,
        }
    }

    pub fn finish(self) -> Result<EssentialPlatformInfo, Error> {
        match self.error {
            Some(error) => Err(error),
            None if self.depth != 0 => Err(Error::InvalidDepth),
            None => Ok(self.result),
        }
    }

    fn claim(&mut self, id: NodeId) -> Result<(), Error> {
        let slot = self
            .result
            .claims
            .get_mut(self.result.claim_count)
            .ok_or(Error::TooManyClaims)?;
        *slot = Some(id);
        self.result.claim_count += 1;
        Ok(())
    }

    fn discover_node(
        &mut self,
        node: NodeResources<'_>,
        candidate: Candidate,
    ) -> Result<(), Error> {
        if !node.enabled {
            return Ok(());
        }
        if candidate.pl011 && self.result.console.is_none() {
            let register = first_register(&node)?;
            self.result.console = Some(ConsoleInfo {
                kind: ConsoleKind::Pl011,
                base: register.start(),
            });
            self.claim(node.id)?;
        }
        if candidate.gic_v3 && self.result.interrupt_controller.is_none() {
            self.result.interrupt_controller = Some(discover_gic(&node, candidate)?);
            self.claim(node.id)?;
        }
        if candidate.timer && self.result.timer.is_none() {
            self.result.timer = Some(discover_timer(&node)?);
            self.claim(node.id)?;
        }
        if (candidate.psci_version.is_some() || candidate.psci_legacy)
            && self.result.cpu_power.is_none()
        {
            self.result.cpu_power = Some(discover_psci(candidate)?);
            self.claim(node.id)?;
        }
        Ok(())
    }
}

impl Default for EssentialDeviceDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeVisitor for EssentialDeviceDiscovery {
    fn begin_node(&mut self, _id: NodeId, _name: &str) {
        if self.depth == MAX_FDT_DEPTH {
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
            .and_then(|index| self.nodes.get_mut(index))
        else {
            self.error = Some(Error::InvalidDepth);
            return;
        };
        let result = match name {
            "compatible" => {
                candidate.pl011 = string_list_contains(value, "arm,pl011");
                candidate.gic_v3 = string_list_contains(value, "arm,gic-v3");
                candidate.timer = string_list_contains(value, "arm,armv8-timer");
                candidate.psci_legacy = string_list_contains(value, "arm,psci");
                candidate.psci_version = if string_list_contains(value, "arm,psci-1.0") {
                    Some(PsciCompatibleVersion::V1_0)
                } else if string_list_contains(value, "arm,psci-0.2") {
                    Some(PsciCompatibleVersion::V0_2)
                } else {
                    None
                };
                Ok(())
            }
            "method" => {
                candidate.psci_conduit = if c_string_equals(value, "smc") {
                    Some(PsciConduit::Smc)
                } else if c_string_equals(value, "hvc") {
                    Some(PsciConduit::Hvc)
                } else {
                    None
                };
                Ok(())
            }
            "#redistributor-regions" => read_u32(value).map(|value| {
                candidate.redistributor_regions = value;
            }),
            "redistributor-stride" => read_u64_property(value).map(|value| {
                candidate.redistributor_stride = Some(value);
            }),
            _ => Ok(()),
        };
        if let Err(error) = result {
            self.error = Some(error);
        }
    }

    fn end_node(&mut self, node: NodeResources<'_>) {
        let Some(index) = self.depth.checked_sub(1) else {
            self.error = Some(Error::InvalidDepth);
            return;
        };
        let candidate = self.nodes[index];
        self.depth = index;
        if self.error.is_none()
            && let Err(error) = self.discover_node(node, candidate)
        {
            self.error = Some(error);
        }
    }
}

fn first_register(node: &NodeResources<'_>) -> Result<PhysicalRange, Error> {
    node.registers
        .first()
        .copied()
        .ok_or(Error::InvalidProperty)
}

fn discover_gic(
    node: &NodeResources<'_>,
    candidate: Candidate,
) -> Result<InterruptControllerInfo, Error> {
    let distributor = first_register(node)?;
    let region_count =
        usize::try_from(candidate.redistributor_regions).map_err(|_| Error::InvalidGic)?;
    if region_count == 0 || region_count > MAX_GIC_REDISTRIBUTOR_REGIONS {
        return Err(Error::InvalidGic);
    }
    let end = 1usize.checked_add(region_count).ok_or(Error::InvalidGic)?;
    let regions = node.registers.get(1..end).ok_or(Error::InvalidGic)?;
    let mut redistributors = RegionList::new();
    for &region in regions {
        redistributors
            .insert(region)
            .map_err(|_| Error::InvalidGic)?;
    }
    Ok(InterruptControllerInfo::GicV3(GicV3Info {
        distributor,
        redistributors,
        redistributor_stride: candidate.redistributor_stride,
    }))
}

fn discover_timer(node: &NodeResources<'_>) -> Result<TimerInfo, Error> {
    let descriptor = node
        .interrupt_cells
        .get(9..12)
        .ok_or(Error::InvalidInterrupt)?;
    Ok(TimerInfo {
        kind: TimerKind::ArmGenericHypervisorPhysical,
        interrupt: decode_gic_interrupt(descriptor)?,
        trigger: decode_gic_trigger(descriptor[2])?,
    })
}

fn discover_psci(candidate: Candidate) -> Result<CpuPowerInfo, Error> {
    Ok(CpuPowerInfo::Psci(PsciInfo {
        compatible_version: candidate.psci_version.ok_or(Error::InvalidPsci)?,
        conduit: candidate.psci_conduit.ok_or(Error::InvalidPsci)?,
    }))
}

fn decode_gic_interrupt(descriptor: &[u32]) -> Result<u32, Error> {
    let interrupt_type = *descriptor.first().ok_or(Error::InvalidInterrupt)?;
    let number = *descriptor.get(1).ok_or(Error::InvalidInterrupt)?;
    match interrupt_type {
        0 => number.checked_add(32).ok_or(Error::InvalidInterrupt),
        1 if number < 16 => Ok(number + 16),
        _ => Err(Error::InvalidInterrupt),
    }
}

fn decode_gic_trigger(flags: u32) -> Result<PlatformInterruptTrigger, Error> {
    match flags & 0xf {
        1 | 2 => Ok(PlatformInterruptTrigger::Edge),
        4 | 8 => Ok(PlatformInterruptTrigger::Level),
        _ => Err(Error::InvalidInterrupt),
    }
}

fn string_list_contains(value: &[u8], expected: &str) -> bool {
    value
        .split(|byte| *byte == 0)
        .any(|candidate| candidate == expected.as_bytes())
}

fn c_string_equals(value: &[u8], expected: &str) -> bool {
    value.split(|byte| *byte == 0).next() == Some(expected.as_bytes())
}

fn read_u32(bytes: &[u8]) -> Result<u32, Error> {
    let raw: [u8; 4] = bytes
        .get(..4)
        .ok_or(Error::InvalidProperty)?
        .try_into()
        .map_err(|_| Error::InvalidProperty)?;
    Ok(u32::from_be_bytes(raw))
}

fn read_u64_property(bytes: &[u8]) -> Result<u64, Error> {
    match bytes.len() {
        4 => Ok(u64::from(read_u32(bytes)?)),
        8 => {
            let raw: [u8; 8] = bytes.try_into().map_err(|_| Error::InvalidProperty)?;
            Ok(u64::from_be_bytes(raw))
        }
        _ => Err(Error::InvalidProperty),
    }
}
