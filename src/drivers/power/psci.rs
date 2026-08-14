use crate::hal::cpu_power::{
    CpuAffinityState, CpuHardwareId, CpuPower, CpuPowerCapabilities, CpuPowerVersion,
    ResumeAddress, SuspendState,
};
use crate::platform::PsciCompatibleVersion;

const PSCI_VERSION: u32 = 0x8400_0000;
const PSCI_CPU_SUSPEND_32: u32 = 0x8400_0001;
const PSCI_CPU_SUSPEND_64: u32 = 0xc400_0001;
const PSCI_CPU_OFF: u32 = 0x8400_0002;
const PSCI_CPU_ON_32: u32 = 0x8400_0003;
const PSCI_CPU_ON_64: u32 = 0xc400_0003;
const PSCI_AFFINITY_INFO_32: u32 = 0x8400_0004;
const PSCI_AFFINITY_INFO_64: u32 = 0xc400_0004;
const PSCI_SYSTEM_OFF: u32 = 0x8400_0008;
const PSCI_SYSTEM_RESET: u32 = 0x8400_0009;
const PSCI_FEATURES: u32 = 0x8400_000a;

const PSCI_SUCCESS: i64 = 0;
const PSCI_NOT_SUPPORTED: i64 = -1;
const PSCI_INVALID_PARAMETERS: i64 = -2;
const PSCI_DENIED: i64 = -3;
const PSCI_ALREADY_ON: i64 = -4;
const PSCI_ON_PENDING: i64 = -5;
const PSCI_INTERNAL_FAILURE: i64 = -6;
const PSCI_NOT_PRESENT: i64 = -7;
const PSCI_DISABLED: i64 = -8;
const PSCI_INVALID_ADDRESS: i64 = -9;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallWidth {
    Bits32,
    Bits64,
}

/// Architecture bridge for issuing one SMCCC firmware call.
pub trait Conduit: Copy {
    const CALL_WIDTH: CallWidth;

    fn invoke(self, function_id: u32, argument0: u64, argument1: u64, argument2: u64) -> u64;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    NotSupported,
    InvalidParameters,
    Denied,
    AlreadyOn,
    OnPending,
    InternalFailure,
    NotPresent,
    Disabled,
    InvalidAddress,
    UnsupportedVersion,
    InvalidAffinityLevel,
    InvalidAffinityState,
    UnexpectedReturn,
    UnknownStatus(i64),
}

#[derive(Clone, Copy)]
pub struct Psci<FirmwareConduit> {
    conduit: FirmwareConduit,
    capabilities: CpuPowerCapabilities,
}

impl<FirmwareConduit: Conduit> Psci<FirmwareConduit> {
    pub fn initialize(
        conduit: FirmwareConduit,
        compatible_version: PsciCompatibleVersion,
    ) -> Result<Self, Error> {
        let raw_version = call(conduit, PSCI_VERSION, 0, 0, 0);
        if raw_version < 0 {
            return Err(status_error(raw_version));
        }
        let version_word = raw_version as u32;
        let version = CpuPowerVersion {
            major: (version_word >> 16) as u16,
            minor: version_word as u16,
        };
        if version.major == 0 && version.minor < 2 {
            return Err(Error::UnsupportedVersion);
        }
        if compatible_version == PsciCompatibleVersion::V1_0 && version.major < 1 {
            return Err(Error::UnsupportedVersion);
        }

        let ids = FunctionIds::for_conduit::<FirmwareConduit>();
        let feature_discovery = version.major >= 1;
        let supported = |function_id: u32| {
            !feature_discovery || call(conduit, PSCI_FEATURES, u64::from(function_id), 0, 0) >= 0
        };
        Ok(Self {
            conduit,
            capabilities: CpuPowerCapabilities {
                version,
                cpu_suspend: supported(ids.cpu_suspend),
                cpu_off: supported(PSCI_CPU_OFF),
                cpu_on: supported(ids.cpu_on),
                affinity_info: supported(ids.affinity_info),
                system_off: supported(PSCI_SYSTEM_OFF),
                system_reset: supported(PSCI_SYSTEM_RESET),
            },
        })
    }

    fn call(&self, function_id: u32, argument0: u64, argument1: u64, argument2: u64) -> i64 {
        call(self.conduit, function_id, argument0, argument1, argument2)
    }

    fn checked_argument(value: u64) -> Result<u64, Error> {
        if FirmwareConduit::CALL_WIDTH == CallWidth::Bits32 && value > u64::from(u32::MAX) {
            Err(Error::InvalidAddress)
        } else {
            Ok(value)
        }
    }
}

impl<FirmwareConduit: Conduit> CpuPower for Psci<FirmwareConduit> {
    type Error = Error;

    fn capabilities(&self) -> CpuPowerCapabilities {
        self.capabilities
    }

    fn cpu_on(
        &self,
        target: CpuHardwareId,
        entry: ResumeAddress,
        context: u64,
    ) -> Result<(), Self::Error> {
        let ids = FunctionIds::for_conduit::<FirmwareConduit>();
        decode_status(self.call(
            ids.cpu_on,
            Self::checked_argument(target.get())?,
            Self::checked_argument(entry.get())?,
            Self::checked_argument(context)?,
        ))
    }

    fn cpu_off(&self) -> Result<(), Self::Error> {
        decode_non_returning(self.call(PSCI_CPU_OFF, 0, 0, 0))
    }

    fn cpu_suspend(
        &self,
        state: SuspendState,
        entry: ResumeAddress,
        context: u64,
    ) -> Result<(), Self::Error> {
        let ids = FunctionIds::for_conduit::<FirmwareConduit>();
        decode_status(self.call(
            ids.cpu_suspend,
            u64::from(state.get()),
            Self::checked_argument(entry.get())?,
            Self::checked_argument(context)?,
        ))
    }

    fn affinity_info(
        &self,
        target: CpuHardwareId,
        lowest_affinity_level: u8,
    ) -> Result<CpuAffinityState, Self::Error> {
        if lowest_affinity_level > 3 {
            return Err(Error::InvalidAffinityLevel);
        }
        let ids = FunctionIds::for_conduit::<FirmwareConduit>();
        match self.call(
            ids.affinity_info,
            Self::checked_argument(target.get())?,
            u64::from(lowest_affinity_level),
            0,
        ) {
            0 => Ok(CpuAffinityState::On),
            1 => Ok(CpuAffinityState::Off),
            2 => Ok(CpuAffinityState::OnPending),
            value if value < 0 => Err(status_error(value)),
            _ => Err(Error::InvalidAffinityState),
        }
    }

    fn system_off(&self) -> Result<(), Self::Error> {
        decode_non_returning(self.call(PSCI_SYSTEM_OFF, 0, 0, 0))
    }

    fn system_reset(&self) -> Result<(), Self::Error> {
        decode_non_returning(self.call(PSCI_SYSTEM_RESET, 0, 0, 0))
    }
}

struct FunctionIds {
    cpu_suspend: u32,
    cpu_on: u32,
    affinity_info: u32,
}

impl FunctionIds {
    fn for_conduit<FirmwareConduit: Conduit>() -> Self {
        match FirmwareConduit::CALL_WIDTH {
            CallWidth::Bits32 => Self {
                cpu_suspend: PSCI_CPU_SUSPEND_32,
                cpu_on: PSCI_CPU_ON_32,
                affinity_info: PSCI_AFFINITY_INFO_32,
            },
            CallWidth::Bits64 => Self {
                cpu_suspend: PSCI_CPU_SUSPEND_64,
                cpu_on: PSCI_CPU_ON_64,
                affinity_info: PSCI_AFFINITY_INFO_64,
            },
        }
    }
}

fn call<FirmwareConduit: Conduit>(
    conduit: FirmwareConduit,
    function_id: u32,
    argument0: u64,
    argument1: u64,
    argument2: u64,
) -> i64 {
    let result = conduit.invoke(function_id, argument0, argument1, argument2);
    if function_id & 0x4000_0000 == 0 {
        i64::from(result as u32 as i32)
    } else {
        result as i64
    }
}

fn decode_status(status: i64) -> Result<(), Error> {
    if status == PSCI_SUCCESS {
        Ok(())
    } else {
        Err(status_error(status))
    }
}

fn decode_non_returning(status: i64) -> Result<(), Error> {
    if status == PSCI_SUCCESS {
        Err(Error::UnexpectedReturn)
    } else {
        Err(status_error(status))
    }
}

fn status_error(status: i64) -> Error {
    match status {
        PSCI_NOT_SUPPORTED => Error::NotSupported,
        PSCI_INVALID_PARAMETERS => Error::InvalidParameters,
        PSCI_DENIED => Error::Denied,
        PSCI_ALREADY_ON => Error::AlreadyOn,
        PSCI_ON_PENDING => Error::OnPending,
        PSCI_INTERNAL_FAILURE => Error::InternalFailure,
        PSCI_NOT_PRESENT => Error::NotPresent,
        PSCI_DISABLED => Error::Disabled,
        PSCI_INVALID_ADDRESS => Error::InvalidAddress,
        value => Error::UnknownStatus(value),
    }
}
