<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Monotonic and UTC clocks

Scheduler deadlines, sleeps, and timeouts use the existing monotonic clock.
`clock_get_realtime` separately returns UTC as signed Unix seconds plus
nanoseconds in `0..1_000_000_000`. The SDK maps this to Native timestamps and
Rust `SystemTime`; it does not reinterpret uptime as a calendar date.

On platforms describing an `arm,pl031` RTC in their device tree, kernel device
discovery binds its permanent MMIO resource to a read-only PL031 driver. The
driver reads only an already enabled counter: it does not reset, program,
start, or enable interrupts on the device. UTC initialization brackets the RTC
read with monotonic samples and rejects excessive interruption between them.
The kernel publishes one immutable UTC/monotonic anchor and advances it using
the clocksource. Reading UTC thereafter performs no MMIO and requires no
polling worker or RTC interrupt.

PL031 supplies whole seconds in a 32-bit unsigned counter, covering dates from
1970 through early 2106. Its initial precision is one second; nanosecond
interpolation does not improve that initial accuracy. The current implementation
has no time-setting, network synchronization, suspend compensation, or later
host-clock adjustment protocol. A future clock-discipline service must define
these semantics explicitly rather than changing monotonic deadlines.

QEMU's AArch64 `virt` machine provides a simulated PL031 RTC. By default, QEMU
initializes it from host UTC. A reproducible test can select, for example,
`-rtc base=2026-01-01T00:00:00,clock=vm`. A simulated device can therefore supply
a real calendar time; a fabricated kernel timestamp is unnecessary. See the
[QEMU RTC options](https://www.qemu.org/docs/master/system/qemu-manpage.html)
and [virt platform documentation](https://www.qemu.org/docs/master/system/arm/virt.html).

Missing, disabled, unmappable RTCs or rejected calibration samples leave UTC
unavailable. MMIO bus faults are not converted into optional-clock absence. Native
calls then return `NotSupported`; APIs with an error channel propagate it.
Rust's infallible `SystemTime::now()` follows the platform unsupported panic
path. Secondary architectures continue to compile without claiming an RTC
source their platform has not supplied.

Host tests cover timestamp normalization and arithmetic. QEMU integration must
verify real-time reads and filesystem timestamps through the installed std
adapter. Physical hardware qualification must additionally confirm device-tree
resources, Device memory attributes, an initialized UTC counter and correct
clocksource behavior; QEMU cannot establish those board-specific properties.
