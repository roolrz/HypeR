#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Protect the ordered activation and teardown transactions of a local vCPU.
set -eu

root=${HYPER_VCPU_TRANSITION_ROOT:-$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)}
cd "$root"

source=src/kernel/vm/vcpu.rs
fixture=$(mktemp -d "${TMPDIR:-/tmp}/hyper-vcpu-transition-check.XXXXXX")
trap 'rm -rf "$fixture"' EXIT HUP INT TERM

sed -n '/^[^[:space:]].*unsafe fn activate(/,/^}/p' "$source" >"$fixture/activate.rs"
sed -n '/^[^[:space:]].*unsafe fn deactivate(/,/^}/p' "$source" >"$fixture/deactivate.rs"

require_in() {
    checked_source=$1
    pattern=$2
    message=$3
    if ! rg -q -U "$pattern" "$checked_source"; then
        echo "$message" >&2
        exit 1
    fi
}

require_occurrences() {
    pattern=$1
    expected=$2
    message=$3
    count=$(rg -o "$pattern" "$source" 2>/dev/null | wc -l | tr -d ' ')
    if [ "$count" -ne "$expected" ]; then
        echo "$message (expected $expected, found $count)" >&2
        exit 1
    fi
}

line_in() {
    checked_source=$1
    pattern=$2
    rg -n "$pattern" "$checked_source" | sed -n '1s/:.*//p'
}

require_order() {
    checked_source=$1
    first_pattern=$2
    second_pattern=$3
    message=$4
    first=$(line_in "$checked_source" "$first_pattern")
    second=$(line_in "$checked_source" "$second_pattern")
    if [ -z "$first" ] || [ -z "$second" ] || [ "$first" -ge "$second" ]; then
        echo "$message" >&2
        exit 1
    fi
}

require_occurrences 'crate::hal::vm::activate_hardware\(' 1 \
    'vCPU activation must attach hardware exactly once'
require_occurrences 'crate::hal::vm::deactivate_hardware\(' 3 \
    'vCPU transitions must cover both activation rollbacks and normal teardown'
require_occurrences 'super::active_vcpu::set_raw\(' 1 \
    'active-vCPU ownership must have one publication seam'
require_occurrences 'super::active_vcpu::clear\(' 1 \
    'active-vCPU ownership must have one teardown seam'

require_order "$fixture/activate.rs" \
    'crate::hal::vm::activate_hardware\(' \
    'super::timer::set_host_timer_enabled\(!timer_asserted\)' \
    'hardware must be active before the host timer is programmed'
require_order "$fixture/activate.rs" \
    'super::timer::set_host_timer_enabled\(!timer_asserted\)' \
    'super::active_vcpu::set_raw\(' \
    'the active vCPU must be published only after timer programming succeeds'
require_in "$fixture/activate.rs" \
    'crate::hal::vm::activate_hardware\([^;]*\)[[:space:]]*\}[[:space:]]*\.map_err\(HardwareTransitionError::Hardware\)\?;' \
    'hardware activation failure must propagate before any later state changes'
require_in "$fixture/activate.rs" \
    'if let Err\(timer\) = super::timer::set_host_timer_enabled\(!timer_asserted\)[^}]*deactivate_hardware\([^;]*\)[[:space:]]*\};[[:space:]]*return match rollback' \
    'timer-programming failure must detach hardware before returning'
require_in "$fixture/activate.rs" \
    '(?s)if let Err\(publication\) = unsafe \{ super::active_vcpu::set_raw\(execution\) \}.*?deactivate_hardware\([^;]*\)[[:space:]]*\};[[:space:]]*let timer = super::timer::set_host_timer_enabled\(true\);[[:space:]]*return match \(rollback, timer\)' \
    'publication failure must detach hardware and restore the host timer'

require_order "$fixture/deactivate.rs" \
    'super::active_vcpu::clear\(' \
    'crate::hal::vm::deactivate_hardware\(' \
    'teardown must remove callback visibility before detaching hardware'
require_order "$fixture/deactivate.rs" \
    'crate::hal::vm::deactivate_hardware\(' \
    'super::timer::set_host_timer_enabled\(true\)' \
    'teardown must restore the host timer only after hardware is detached'
require_in "$fixture/deactivate.rs" \
    'super::active_vcpu::clear\(execution\)\.map_err\(HardwareTransitionError::Active\)\?;' \
    'active-vCPU removal failure must stop teardown immediately'
require_in "$fixture/deactivate.rs" \
    'crate::hal::vm::deactivate_hardware\([^;]*\)[[:space:]]*\}[[:space:]]*\.map_err\(HardwareTransitionError::Hardware\)\?;' \
    'hardware-detach failure must stop teardown before timer restoration'
