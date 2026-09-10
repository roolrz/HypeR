// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! UTC seeded once from firmware's RTC and advanced by the host clocksource.

use hyper::drivers::platform::{DriverServices, PlatformDevice};
use hyper::drivers::rtc::{Goldfish, Pl031};
use hyper::sync::PublishedOnce;
use hyper::time::Timestamp;

struct Anchor {
    utc: Timestamp,
    monotonic: u64,
}

static ANCHOR: PublishedOnce<Anchor> = PublishedOnce::new();

/// Optional boot-time discovery. Platform policy owns DTB resources and their
/// permanent mappings; physical register access remains in the RTC driver.
pub(crate) fn initialize(devices: &[PlatformDevice], services: &impl DriverServices) {
    for device in devices {
        let Some(mut rtc) = Clock::bind(device, services) else {
            continue;
        };
        let mut sample = None;
        for _ in 0..3 {
            let Ok(before) = super::monotonic_nanoseconds() else {
                return;
            };
            let Some(utc) = rtc.read() else {
                break;
            };
            let Ok(after) = super::monotonic_nanoseconds() else {
                return;
            };
            let Some(elapsed) = after.checked_sub(before) else {
                continue;
            };
            // Reject interruption between the two clock samples rather than
            // silently turning arbitrary scheduling delay into a UTC offset.
            if elapsed <= 1_000_000 {
                sample = Some((utc, before + elapsed / 2));
                break;
            }
        }
        let Some((utc, monotonic)) = sample else {
            continue;
        };
        // The anchor retains the device's precision (seconds for PL031,
        // nanoseconds for Goldfish). Interpolation cannot improve accuracy.
        // No image-build timestamp or uptime is substituted for UTC.
        if ANCHOR.publish(Anchor { utc, monotonic }).is_ok() {
            crate::pr_info!("HypeR: UTC clock initialized from {} RTC", rtc.name());
        }
        return;
    }
}

pub(crate) fn now() -> Option<Timestamp> {
    let anchor = ANCHOR.get()?;
    let elapsed = super::monotonic_nanoseconds()
        .ok()?
        .checked_sub(anchor.monotonic)?;
    anchor.utc.checked_add_nanoseconds(elapsed)
}

// Device selection and the UTC anchor are policy. Ordered register sampling
// remains in the physical drivers. Boot owns the sole Goldfish latch reader.
enum Clock {
    Pl031(Pl031),
    Goldfish(Goldfish<crate::hal::memory::Barrier>),
}

impl Clock {
    fn bind(device: &PlatformDevice, services: &impl DriverServices) -> Option<Self> {
        let pl031 = device.is_compatible("arm,pl031");
        let goldfish = device.is_compatible("google,goldfish-rtc");
        if !pl031 && !goldfish {
            return None;
        }
        let resource = device.registers().first()?;
        let mapping = services.map_mmio(*resource).ok()?;
        if pl031 {
            Pl031::bind(mapping).ok().map(Self::Pl031)
        } else {
            Goldfish::bind(mapping).ok().map(Self::Goldfish)
        }
    }

    fn read(&mut self) -> Option<Timestamp> {
        match self {
            Self::Pl031(clock) => Timestamp::new(i64::from(clock.seconds()?), 0),
            Self::Goldfish(clock) => {
                let nanos = clock.nanoseconds();
                Timestamp::new(
                    nanos.div_euclid(1_000_000_000),
                    nanos.rem_euclid(1_000_000_000) as u32,
                )
            }
        }
    }

    const fn name(&self) -> &'static str {
        match self {
            Self::Pl031(_) => "PL031",
            Self::Goldfish(_) => "Goldfish",
        }
    }
}
