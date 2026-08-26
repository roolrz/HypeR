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
    'claim_execution\(' \
    'super::memory::activate\(' \
    'exclusive VM execution must be claimed before stage-2 activation'
require_order "$fixture/activate.rs" \
    'super::memory::activate\(' \
    'crate::hal::vm::activate_hardware\(' \
    'the current mapping epoch must be active before vCPU hardware'
require_order "$fixture/activate.rs" \
    'crate::hal::vm::activate_hardware\(' \
    'super::timer::set_host_timer_enabled\(!timer_asserted\)' \
    'hardware must be active before the host timer is programmed'
require_order "$fixture/activate.rs" \
    'super::timer::set_host_timer_enabled\(!timer_asserted\)' \
    'super::active_vcpu::set_raw\(' \
    'the active vCPU must be published only after timer programming succeeds'
require_in "$fixture/activate.rs" \
    '(?s)Err\(error\)[[:space:]]*=>[[:space:]]*\{[[:space:]]*release_execution_or_fail\(execution\);[[:space:]]*return Err\(HardwareTransitionError::Hardware\(error\)\);' \
    'hardware activation failure must release VM execution before returning'
require_in "$fixture/activate.rs" \
    'if let Err\(timer\) = super::timer::set_host_timer_enabled\(!timer_asserted\)[^}]*deactivate_hardware\([^;]*\)[[:space:]]*\};[[:space:]]*return match rollback' \
    'timer-programming failure must detach hardware before returning'
ambiguous_rollbacks=$(rg -o 'Err\(hardware\) => fatal_ambiguous_hardware' \
    "$fixture/activate.rs" 2>/dev/null | wc -l | tr -d ' ')
if [ "$ambiguous_rollbacks" -ne 2 ]; then
    echo 'both ambiguous activation rollbacks must fail-stop while retaining VM execution' >&2
    exit 1
fi
require_in "$fixture/activate.rs" \
    '(?s)if let Err\(publication\) = unsafe \{ super::active_vcpu::set_raw\(execution\) \}.*?deactivate_hardware\([^;]*\)[[:space:]]*\};[[:space:]]*return match rollback.*?Ok\(\(\)\) => match super::timer::set_host_timer_enabled\(true\).*?Err\(hardware\) => fatal_ambiguous_hardware' \
    'publication failure must detach hardware before restoring the host timer'

require_order "$fixture/deactivate.rs" \
    'super::active_vcpu::clear\(' \
    'crate::hal::vm::deactivate_hardware\(' \
    'teardown must remove callback visibility before detaching hardware'
require_order "$fixture/deactivate.rs" \
    'crate::hal::vm::deactivate_hardware\(' \
    'release_execution_or_fail\(' \
    'VM execution must remain claimed until architecture hardware is detached'
require_order "$fixture/deactivate.rs" \
    'release_execution_or_fail\(' \
    'super::timer::set_host_timer_enabled\(true\)' \
    'teardown must release VM execution before restoring the host timer'
require_in "$fixture/deactivate.rs" \
    'super::active_vcpu::clear\(execution\)\.map_err\(HardwareTransitionError::Active\)\?;' \
    'active-vCPU removal failure must stop teardown immediately'
require_in "$fixture/deactivate.rs" \
    '(?s)if let Err\(error\) = unsafe \{.*?crate::hal::vm::deactivate_hardware\(.*?\)[[:space:]]*\}[[:space:]]*\{.*?fatal_ambiguous_hardware\(.*?error\);.*?\}[[:space:]]*release_execution_or_fail\(execution\);' \
    'ambiguous hardware detach must fail-stop while retaining VM execution'
