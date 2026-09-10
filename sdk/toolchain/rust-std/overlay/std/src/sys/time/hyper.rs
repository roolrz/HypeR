// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use crate::time::Duration;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct Instant(Duration);
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct SystemTime {
    seconds: i64,
    nanoseconds: u32,
}
pub const UNIX_EPOCH: SystemTime = SystemTime {
    seconds: 0,
    nanoseconds: 0,
};
impl Instant {
    pub fn now() -> Self {
        Self(Duration::from_nanos(unsafe {
            crate::sys::pal::ffi::__hyper_std_clock()
        }))
    }
    pub fn checked_sub_instant(&self, other: &Self) -> Option<Duration> {
        self.0.checked_sub(other.0)
    }
    pub fn checked_add_duration(&self, duration: &Duration) -> Option<Self> {
        Some(Self(self.0.checked_add(*duration)?))
    }
    pub fn checked_sub_duration(&self, duration: &Duration) -> Option<Self> {
        Some(Self(self.0.checked_sub(*duration)?))
    }
}
impl SystemTime {
    pub const MAX: Self = Self {
        seconds: i64::MAX,
        nanoseconds: 999_999_999,
    };
    pub const MIN: Self = Self {
        seconds: i64::MIN,
        nanoseconds: 0,
    };
    pub fn from_native(seconds: i64, nanoseconds: u32) -> Option<Self> {
        (nanoseconds < 1_000_000_000).then_some(Self {
            seconds,
            nanoseconds,
        })
    }
    pub fn native(&self) -> (i64, u32) {
        (self.seconds, self.nanoseconds)
    }
    pub fn now() -> Self {
        let mut seconds = 0;
        let mut nanoseconds = 0;
        let status =
            unsafe { crate::sys::pal::ffi::__hyper_std_realtime(&mut seconds, &mut nanoseconds) };
        match (status, Self::from_native(seconds, nanoseconds)) {
            (0, Some(time)) => time,
            _ => panic!("wall clock unavailable on this platform"),
        }
    }
    fn nanos(&self) -> i128 {
        self.seconds as i128 * 1_000_000_000 + self.nanoseconds as i128
    }
    fn from_nanos(value: i128) -> Option<Self> {
        let seconds = i64::try_from(value.div_euclid(1_000_000_000)).ok()?;
        Some(Self {
            seconds,
            nanoseconds: value.rem_euclid(1_000_000_000) as u32,
        })
    }
    pub fn sub_time(&self, other: &Self) -> Result<Duration, Duration> {
        let difference = self.nanos() - other.nanos();
        let magnitude = difference.unsigned_abs();
        let duration = Duration::new(
            (magnitude / 1_000_000_000) as u64,
            (magnitude % 1_000_000_000) as u32,
        );
        if difference < 0 {
            Err(duration)
        } else {
            Ok(duration)
        }
    }
    pub fn checked_add_duration(&self, duration: &Duration) -> Option<Self> {
        Self::from_nanos(self.nanos().checked_add(duration.as_nanos() as i128)?)
    }
    pub fn checked_sub_duration(&self, duration: &Duration) -> Option<Self> {
        Self::from_nanos(self.nanos().checked_sub(duration.as_nanos() as i128)?)
    }
}
