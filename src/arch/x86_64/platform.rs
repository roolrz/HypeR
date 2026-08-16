use hyper::platform::{
    CpuPowerInfo, InterruptControllerInfo, PlatformInterrupt, PlatformInterruptTrigger, TimerInfo,
    TimerKind, X2ApicInfo, X86ApicInfo,
    fdt::{NodeId, NodeResources, NodeVisitor},
};

pub const TIMER_VECTOR: u32 = 0xef;

#[derive(Clone, Copy, Debug)]
pub struct EssentialPlatformInfo {
    pub cpu_power: Option<CpuPowerInfo>,
    pub interrupt_controller: Option<InterruptControllerInfo>,
    pub timer: Option<TimerInfo>,
    pub tsc_frequency: u64,
    claims: [Option<hyper::platform::fdt::NodeId>; 0],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidInterrupt,
    MissingInvariantTsc,
    MissingTscFrequency,
    MissingX2Apic,
}

pub struct EssentialDeviceDiscovery {
    tsc_frequency: Option<u64>,
}

impl EssentialDeviceDiscovery {
    pub const fn new() -> Self {
        Self {
            tsc_frequency: None,
        }
    }

    pub fn finish(self) -> Result<EssentialPlatformInfo, Error> {
        let basic = core::arch::x86_64::__cpuid(1);
        if basic.ecx & (1 << 21) == 0 {
            return Err(Error::MissingX2Apic);
        }
        let extended_max = core::arch::x86_64::__cpuid(0x8000_0000).eax;
        let invariant_tsc = extended_max >= 0x8000_0007
            && core::arch::x86_64::__cpuid(0x8000_0007).edx & (1 << 8) != 0;
        if !invariant_tsc && !super::features::running_under_qemu_tcg() {
            return Err(Error::MissingInvariantTsc);
        }
        let frequency = self
            .tsc_frequency
            .or_else(super::features::tsc_frequency)
            .ok_or(Error::MissingTscFrequency)?;
        Ok(EssentialPlatformInfo {
            cpu_power: Some(CpuPowerInfo::X86Apic(X86ApicInfo {
                tsc_frequency: frequency,
            })),
            interrupt_controller: Some(InterruptControllerInfo::X2Apic(X2ApicInfo)),
            timer: Some(TimerInfo {
                kind: TimerKind::X86TscDeadline,
                virtual_timer: PlatformInterrupt {
                    interrupt: TIMER_VECTOR,
                    trigger: PlatformInterruptTrigger::Edge,
                },
                hypervisor_physical: PlatformInterrupt {
                    interrupt: TIMER_VECTOR,
                    trigger: PlatformInterruptTrigger::Edge,
                },
            }),
            tsc_frequency: frequency,
            claims: [],
        })
    }
}

impl EssentialPlatformInfo {
    pub fn claims(&self) -> &[Option<hyper::platform::fdt::NodeId>] {
        &self.claims
    }
}

impl Default for EssentialDeviceDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeVisitor for EssentialDeviceDiscovery {
    fn begin_node(&mut self, _id: NodeId, _name: &str) {}

    fn property(&mut self, _id: NodeId, name: &str, value: &[u8]) {
        if name == "hyper,tsc-frequency-hz" && value.len() == 8 {
            self.tsc_frequency = Some(u64::from_be_bytes([
                value[0], value[1], value[2], value[3], value[4], value[5], value[6], value[7],
            ]));
        }
    }

    fn end_node(&mut self, _node: NodeResources<'_>) {}
}

pub fn decode_platform_interrupt(descriptor: &[u32]) -> Result<PlatformInterrupt, Error> {
    let interrupt = *descriptor.first().ok_or(Error::InvalidInterrupt)?;
    Ok(PlatformInterrupt {
        interrupt,
        trigger: PlatformInterruptTrigger::Edge,
    })
}
