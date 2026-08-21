#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Exercise the topical architecture-facade check against common bypasses.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/hyper-arch-facade-test.XXXXXX")
trap 'rm -rf "$fixture"' EXIT HUP INT TERM

mkdir -p "$fixture/src/arch" "$fixture/src/kernel"
source_file=$fixture/src/kernel/time.rs
facade_file=$fixture/src/arch/mod.rs

check() {
    HYPER_ARCH_FACADE_ROOT="$fixture" sh "$root/tests/ci/check-arch-facades.sh"
}

printf '%s\n' 'fn read() { crate::arch::time::Counter::read(); }' >"$source_file"
printf '%s\n' 'pub(crate) mod time;' >"$facade_file"
check

printf '%s\n' 'fn read() { crate::arch::ArchitectureCounter::read(); }' >"$source_file"
if check >/dev/null 2>&1; then
    echo "flat architecture timer paths must be rejected" >&2
    exit 1
fi

printf '%s\n' 'fn read() { crate::arch::time::Counter::read(); }' >"$source_file"
printf '%s\n' 'pub use imp::{ArchitectureCounter, LocalInterruptMask};' >"$facade_file"
if check >/dev/null 2>&1; then
    echo "flat architecture timer re-exports must be rejected" >&2
    exit 1
fi

printf '%s\n' \
    'use crate::arch::{ArchitectureCounter as Counter, LocalInterruptMask};' \
    'fn read() { Counter::read(); }' >"$source_file"
if check >/dev/null 2>&1; then
    echo "grouped flat architecture timer imports must be rejected" >&2
    exit 1
fi

printf '%s\n' 'fn read() { crate :: arch :: TimerError::InvalidFrequency; }' >"$source_file"
if check >/dev/null 2>&1; then
    echo "spaced flat architecture timer paths must be rejected" >&2
    exit 1
fi

printf '%s\n' 'pub(crate) mod cpu;' >"$facade_file"
printf '%s\n' 'fn stop() { crate::arch::halt(); }' >"$source_file"
if check >/dev/null 2>&1; then
    echo "flat CPU lifecycle paths must be rejected" >&2
    exit 1
fi

printf '%s\n' 'fn stop() { crate::arch::cpu::halt(); }' >"$source_file"
printf '%s\n' 'pub use imp::{CpuPowerError, LocalInterruptMask};' >"$facade_file"
if check >/dev/null 2>&1; then
    echo "flat CPU mechanism re-exports must be rejected" >&2
    exit 1
fi

printf '%s\n' 'pub(crate) mod context;' >"$facade_file"
printf '%s\n' 'use crate::arch::context::{ThreadContext, UserContext};' >"$source_file"
check

printf '%s\n' 'type Saved = crate::arch::ThreadContext;' >"$source_file"
if check >/dev/null 2>&1; then
    echo "flat thread-context paths must be rejected" >&2
    exit 1
fi

printf '%s\n' 'type Saved = crate::arch::context::ThreadContext;' >"$source_file"
printf '%s\n' 'pub use imp::{ThreadContext, LocalInterruptMask};' >"$facade_file"
if check >/dev/null 2>&1; then
    echo "flat thread-context re-exports must be rejected" >&2
    exit 1
fi

printf '%s\n' 'pub(crate) mod exception;' >"$facade_file"
printf '%s\n' \
    'use crate::arch::exception::{CrashContext, capture_crash_context};' >"$source_file"
check

printf '%s\n' 'fn capture() { crate::arch::capture_crash_context(); }' >"$source_file"
if check >/dev/null 2>&1; then
    echo "flat exception paths must be rejected" >&2
    exit 1
fi

printf '%s\n' 'fn capture() { crate::arch::exception::capture_crash_context(); }' >"$source_file"
printf '%s\n' 'pub use imp::{CrashContext, LocalInterruptMask};' >"$facade_file"
if check >/dev/null 2>&1; then
    echo "flat exception re-exports must be rejected" >&2
    exit 1
fi

printf '%s\n' 'pub(crate) mod irq;' >"$facade_file"
printf '%s\n' 'type Mask = crate::arch::LocalInterruptMask;' >"$source_file"
if check >/dev/null 2>&1; then
    echo "flat host interrupt paths must be rejected" >&2
    exit 1
fi

printf '%s\n' 'type Mask = crate::arch::irq::LocalMask;' >"$source_file"
printf '%s\n' 'pub use imp::{LocalInterruptMask, ThreadContext};' >"$facade_file"
if check >/dev/null 2>&1; then
    echo "flat host interrupt re-exports must be rejected" >&2
    exit 1
fi

printf '%s\n' 'pub(crate) mod irq;' >"$facade_file"
printf '%s\n' \
    'use crate::arch as machine;' \
    'fn disable() { machine::disable_local_interrupts(); }' >"$source_file"
if check >/dev/null 2>&1; then
    echo "root architecture aliases must not bypass topical facade checks" >&2
    exit 1
fi

printf '%s\n' 'pub(crate) mod memory;' >"$facade_file"
printf '%s\n' 'fn flush() { crate::arch::ArchitectureCache::data_line_size(); }' >"$source_file"
if check >/dev/null 2>&1; then
    echo "flat host memory paths must be rejected" >&2
    exit 1
fi

printf '%s\n' 'fn flush() { crate::arch::memory::Cache::data_line_size(); }' >"$source_file"
printf '%s\n' 'pub use imp::{PreparedAddressSpace, ThreadContext};' >"$facade_file"
if check >/dev/null 2>&1; then
    echo "flat host memory re-exports must be rejected" >&2
    exit 1
fi

printf '%s\n' \
    'use crate::arch::{self as machine};' \
    'fn disable() { machine::disable_local_interrupts(); }' >"$source_file"
if check >/dev/null 2>&1; then
    echo "grouped self aliases must not bypass topical facade checks" >&2
    exit 1
fi

printf '%s\n' \
    'use crate::{arch as machine};' \
    'fn disable() { machine::disable_local_interrupts(); }' >"$source_file"
if check >/dev/null 2>&1; then
    echo "crate-root aliases must not bypass topical facade checks" >&2
    exit 1
fi

printf '%s\n' \
    'use crate::{arch::{self as machine}};' \
    'fn disable() { machine::disable_local_interrupts(); }' >"$source_file"
if check >/dev/null 2>&1; then
    echo "nested crate-root aliases must not bypass topical facade checks" >&2
    exit 1
fi

printf '%s\n' 'fn stop() { crate::arch::aarch64::halt(); }' >"$source_file"
if check >/dev/null 2>&1; then
    echo "direct selected-backend paths must not bypass topical facades" >&2
    exit 1
fi

printf '%s\n' 'fn flush() { super::super::arch::ArchitectureCache::data_line_size(); }' >"$source_file"
if check >/dev/null 2>&1; then
    echo "relative flat memory paths must not bypass topical facades" >&2
    exit 1
fi

printf '%s\n' \
    'use crate::{arch::{ArchitectureCache as Cache}};' \
    'fn flush() { Cache::data_line_size(); }' >"$source_file"
if check >/dev/null 2>&1; then
    echo "nested crate imports must not bypass topical facades" >&2
    exit 1
fi

printf '%s\n' \
    'use crate::{arch::memory::Cache};' \
    'fn flush() { Cache::data_line_size(); }' >"$source_file"
printf '%s\n' 'pub(crate) mod memory;' >"$facade_file"
check

printf '%s\n' 'fn flush() { crate::arch::memory::Cache::data_line_size(); }' >"$source_file"
printf '%s\n' \
    'use imp::ArchitectureCache as HiddenCache;' \
    'pub use HiddenCache as Cache;' >"$facade_file"
if check >/dev/null 2>&1; then
    echo "private backend aliases must not restore flat memory exports" >&2
    exit 1
fi

printf '%s\n' 'pub(crate) mod platform;' >"$facade_file"
printf '%s\n' 'fn io() { crate::arch::port_io(); }' >"$source_file"
if check >/dev/null 2>&1; then
    echo "flat host platform paths must be rejected" >&2
    exit 1
fi

printf '%s\n' 'fn io() { crate::arch::platform::port_io(); }' >"$source_file"
printf '%s\n' 'pub use imp::{EssentialPlatformInfo, ThreadContext};' >"$facade_file"
if check >/dev/null 2>&1; then
    echo "flat host platform re-exports must be rejected" >&2
    exit 1
fi

printf '%s\n' 'fn io() { super::super::arch::port_io(); }' >"$source_file"
if check >/dev/null 2>&1; then
    echo "relative flat platform paths must not bypass topical facades" >&2
    exit 1
fi

printf '%s\n' 'pub(crate) mod guest;' >"$facade_file"
printf '%s\n' 'const RAM: u64 = crate::arch::LINUX_GUEST_RAM_IPA;' >"$source_file"
if check >/dev/null 2>&1; then
    echo "flat Linux guest ABI paths must be rejected" >&2
    exit 1
fi

printf '%s\n' \
    'fn ram() -> u64 { crate::arch::guest::linux_abi().ram_base().get() }' >"$source_file"
printf '%s\n' 'pub use imp::{LINUX_GUEST_RAM_IPA, ThreadContext};' >"$facade_file"
if check >/dev/null 2>&1; then
    echo "flat Linux guest ABI re-exports must be rejected" >&2
    exit 1
fi

printf '%s\n' 'fn validate() { super::super::arch::validate_linux_host(); }' >"$source_file"
if check >/dev/null 2>&1; then
    echo "relative flat Linux guest ABI paths must not bypass topical facades" >&2
    exit 1
fi

printf '%s\n' 'use crate::arch::guest::{PayloadMemory, PayloadRange};' >"$source_file"
printf '%s\n' 'pub(crate) mod guest;' >"$facade_file"
check

printf '%s\n' 'type Range = crate::arch::PayloadRange;' >"$source_file"
if check >/dev/null 2>&1; then
    echo "flat Linux payload contract types must be rejected" >&2
    exit 1
fi

printf '%s\n' \
    'fn range() -> crate::arch::guest::PayloadRange { todo!() }' >"$source_file"
printf '%s\n' 'pub use imp::{PayloadRange, ThreadContext};' >"$facade_file"
if check >/dev/null 2>&1; then
    echo "flat Linux payload contract re-exports must be rejected" >&2
    exit 1
fi

printf '%s\n' 'pub(crate) mod vm;' >"$facade_file"
printf '%s\n' 'type Tables = crate::arch::Stage2AddressSpace;' >"$source_file"
if check >/dev/null 2>&1; then
    echo "flat hardware virtualization paths must be rejected" >&2
    exit 1
fi

printf '%s\n' 'type Tables = crate::arch::vm::Stage2AddressSpace;' >"$source_file"
printf '%s\n' 'pub use imp::{VcpuContext, ThreadContext};' >"$facade_file"
if check >/dev/null 2>&1; then
    echo "flat VM mechanism re-exports must be rejected" >&2
    exit 1
fi

printf '%s\n' 'type Tables = crate::arch::vm::Stage2AddressSpace;' >"$source_file"
printf '%s\n' 'pub(crate) type Stage2AddressSpace = imp::Stage2AddressSpace;' >"$facade_file"
if check >/dev/null 2>&1; then
    echo "root backend type aliases must not restore flat architecture types" >&2
    exit 1
fi

printf '%s\n' 'fn exit(frame: &mut super::super::arch::GuestSyncFrame) {}' >"$source_file"
if check >/dev/null 2>&1; then
    echo "relative legacy raw-frame paths must not bypass the VM facade" >&2
    exit 1
fi

printf '%s\n' 'fn stop() { super::super::arch::aarch64::halt(); }' >"$source_file"
if check >/dev/null 2>&1; then
    echo "relative selected-backend paths must not bypass topical facades" >&2
    exit 1
fi

printf '%s\n' \
    'use crate::{arch::{aarch64 as machine}};' \
    'fn stop() { machine::halt(); }' >"$source_file"
if check >/dev/null 2>&1; then
    echo "nested selected-backend imports must not bypass topical facades" >&2
    exit 1
fi

printf '%s\n' \
    'use super::super::arch as machine;' \
    'fn io() { machine::port_io(); }' >"$source_file"
if check >/dev/null 2>&1; then
    echo "relative architecture aliases must not bypass topical facades" >&2
    exit 1
fi
