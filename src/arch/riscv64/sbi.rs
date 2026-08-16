use core::arch::asm;

use hyper::hal::cpu_power::{
    CpuAffinityState, CpuHardwareId, CpuPower, CpuPowerCapabilities, CpuPowerVersion,
    ResumeAddress, SuspendState,
};

const EID_BASE: usize = 0x10;
const EID_TIME: usize = 0x5449_4d45;
const EID_HSM: usize = 0x0048_534d;
const EID_SRST: usize = 0x5352_5354;

const FID_BASE_GET_SPEC_VERSION: usize = 0;
const FID_BASE_PROBE_EXTENSION: usize = 3;
const FID_TIME_SET_TIMER: usize = 0;
const FID_HSM_HART_START: usize = 0;
const FID_HSM_HART_STOP: usize = 1;
const FID_HSM_HART_STATUS: usize = 2;
const FID_HSM_HART_SUSPEND: usize = 3;
const FID_SRST_RESET: usize = 0;

const SBI_SUCCESS: isize = 0;
const SBI_ERR_FAILED: isize = -1;
const SBI_ERR_NOT_SUPPORTED: isize = -2;
const SBI_ERR_INVALID_PARAM: isize = -3;
const SBI_ERR_DENIED: isize = -4;
const SBI_ERR_INVALID_ADDRESS: isize = -5;
const SBI_ERR_ALREADY_AVAILABLE: isize = -6;
const SBI_ERR_ALREADY_STARTED: isize = -7;
const SBI_ERR_ALREADY_STOPPED: isize = -8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Failed,
    NotSupported,
    InvalidParameter,
    Denied,
    InvalidAddress,
    AlreadyAvailable,
    AlreadyStarted,
    AlreadyStopped,
    Unknown(isize),
    MissingRequiredExtension,
    UnsupportedVersion,
}

#[derive(Clone, Copy)]
pub struct Sbi {
    capabilities: CpuPowerCapabilities,
}

#[derive(Clone, Copy)]
struct Return {
    error: isize,
    value: usize,
}

pub fn bind() -> Result<Sbi, Error> {
    let version = call(EID_BASE, FID_BASE_GET_SPEC_VERSION, [0; 6])?;
    let major = (version >> 24) as u16;
    let minor = (version & 0x00ff_ffff) as u16;
    if major == 0 && minor < 2 {
        return Err(Error::UnsupportedVersion);
    }
    if !probe(EID_HSM)? || !probe(EID_TIME)? {
        return Err(Error::MissingRequiredExtension);
    }
    Ok(Sbi {
        capabilities: CpuPowerCapabilities {
            version: CpuPowerVersion { major, minor },
            cpu_suspend: true,
            cpu_off: true,
            cpu_on: true,
            affinity_info: true,
            system_off: probe(EID_SRST)?,
            system_reset: probe(EID_SRST)?,
        },
    })
}

pub fn set_timer(deadline: u64) -> Result<(), Error> {
    call(
        EID_TIME,
        FID_TIME_SET_TIMER,
        [deadline as usize, 0, 0, 0, 0, 0],
    )
    .map(|_| ())
}

impl CpuPower for Sbi {
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
        call(
            EID_HSM,
            FID_HSM_HART_START,
            [
                target.get() as usize,
                entry.get() as usize,
                context as usize,
                0,
                0,
                0,
            ],
        )
        .map(|_| ())
    }

    fn cpu_off(&self) -> Result<(), Self::Error> {
        call(EID_HSM, FID_HSM_HART_STOP, [0; 6]).map(|_| ())
    }

    fn cpu_suspend(
        &self,
        state: SuspendState,
        entry: ResumeAddress,
        context: u64,
    ) -> Result<(), Self::Error> {
        call(
            EID_HSM,
            FID_HSM_HART_SUSPEND,
            [
                state.get() as usize,
                entry.get() as usize,
                context as usize,
                0,
                0,
                0,
            ],
        )
        .map(|_| ())
    }

    fn affinity_info(
        &self,
        target: CpuHardwareId,
        _lowest_affinity_level: u8,
    ) -> Result<CpuAffinityState, Self::Error> {
        match call(
            EID_HSM,
            FID_HSM_HART_STATUS,
            [target.get() as usize, 0, 0, 0, 0, 0],
        )? {
            0 => Ok(CpuAffinityState::On),
            1 => Ok(CpuAffinityState::Off),
            2 | 3 => Ok(CpuAffinityState::OnPending),
            _ => Err(Error::Failed),
        }
    }

    fn system_off(&self) -> Result<(), Self::Error> {
        call(EID_SRST, FID_SRST_RESET, [0, 0, 0, 0, 0, 0]).map(|_| ())
    }

    fn system_reset(&self) -> Result<(), Self::Error> {
        call(EID_SRST, FID_SRST_RESET, [1, 0, 0, 0, 0, 0]).map(|_| ())
    }
}

fn probe(extension: usize) -> Result<bool, Error> {
    Ok(call(
        EID_BASE,
        FID_BASE_PROBE_EXTENSION,
        [extension, 0, 0, 0, 0, 0],
    )? != 0)
}

fn call(extension: usize, function: usize, arguments: [usize; 6]) -> Result<usize, Error> {
    let result = raw_call(extension, function, arguments);
    match result.error {
        SBI_SUCCESS => Ok(result.value),
        SBI_ERR_FAILED => Err(Error::Failed),
        SBI_ERR_NOT_SUPPORTED => Err(Error::NotSupported),
        SBI_ERR_INVALID_PARAM => Err(Error::InvalidParameter),
        SBI_ERR_DENIED => Err(Error::Denied),
        SBI_ERR_INVALID_ADDRESS => Err(Error::InvalidAddress),
        SBI_ERR_ALREADY_AVAILABLE => Err(Error::AlreadyAvailable),
        SBI_ERR_ALREADY_STARTED => Err(Error::AlreadyStarted),
        SBI_ERR_ALREADY_STOPPED => Err(Error::AlreadyStopped),
        other => Err(Error::Unknown(other)),
    }
}

fn raw_call(extension: usize, function: usize, arguments: [usize; 6]) -> Return {
    let mut error = arguments[0];
    let mut value = arguments[1];
    unsafe {
        asm!(
            "ecall",
            inlateout("a0") error,
            inlateout("a1") value,
            in("a2") arguments[2],
            in("a3") arguments[3],
            in("a4") arguments[4],
            in("a5") arguments[5],
            in("a6") function,
            in("a7") extension,
            options(nostack)
        );
    }
    Return {
        error: error as isize,
        value,
    }
}
