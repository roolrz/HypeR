// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! UTC seeded once from firmware's RTC and advanced by the host clocksource.

use hyper::drivers::platform::{DriverServices, PlatformDevice};
use hyper::drivers::rtc::Pl031;
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
        if !device.is_compatible("arm,pl031") {
            continue;
        }
        let Some(resource) = device.registers().first() else {
            continue;
        };
        let Some(rtc) = services
            .map_mmio(*resource)
            .ok()
            .and_then(|map| Pl031::bind(map).ok())
        else {
            continue;
        };
        let mut sample = None;
        for _ in 0..3 {
            let Ok(before) = super::monotonic_nanoseconds() else {
                return;
            };
            let Some(seconds) = rtc.seconds() else {
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
                sample = Some((seconds, before + elapsed / 2));
                break;
            }
        }
        let Some((seconds, monotonic)) = sample else {
            continue;
        };
        let Some(utc) = Timestamp::new(i64::from(seconds), 0) else {
            return;
        };
        // RTC precision is one second; interpolation does not improve initial
        // accuracy. No image-build timestamp or uptime is substituted for UTC.
        if ANCHOR.publish(Anchor { utc, monotonic }).is_ok() {
            crate::pr_info!("HypeR: UTC clock initialized from PL031 RTC");
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
