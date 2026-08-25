#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Prove that each vCPU transition ratchet rejects its corresponding regression.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/hyper-vcpu-transition-test.XXXXXX")
trap 'rm -rf "$fixture"' EXIT HUP INT TERM

mkdir -p "$fixture/src/kernel/vm"

check() {
    HYPER_VCPU_TRANSITION_ROOT="$fixture" \
        sh "$root/tests/ci/check-vcpu-transition-contract.sh"
}

write_valid_fixture() {
    printf '%s\n' \
        'pub(crate) unsafe fn activate(execution: *mut VcpuExecution) -> Result<(), Error> {' \
        '    let timer_asserted = unsafe { crate::hal::vm::activate_hardware() }.map_err(HardwareTransitionError::Hardware)?;' \
        '    if let Err(timer) = super::timer::set_host_timer_enabled(!timer_asserted) {' \
        '        let rollback = unsafe { crate::hal::vm::deactivate_hardware() };' \
        '        return match rollback { Ok(()) => Err(timer), Err(error) => Err(error) };' \
        '    }' \
        '    if let Err(publication) = unsafe { super::active_vcpu::set_raw(execution) } {' \
        '        let rollback = unsafe { crate::hal::vm::deactivate_hardware() };' \
        '        let timer = super::timer::set_host_timer_enabled(true);' \
        '        return match (rollback, timer) { value => Err(value) };' \
        '    }' \
        '    Ok(())' \
        '}' \
        'pub(crate) unsafe fn deactivate(execution: &mut VcpuExecution) -> Result<(), Error> {' \
        '    super::active_vcpu::clear(execution).map_err(HardwareTransitionError::Active)?;' \
        '    unsafe { crate::hal::vm::deactivate_hardware() }.map_err(HardwareTransitionError::Hardware)?;' \
        '    super::timer::set_host_timer_enabled(true)' \
        '}' >"$fixture/src/kernel/vm/vcpu.rs"
}

mutate() {
    description=$1
    expression=$2
    write_valid_fixture
    sed "$expression" "$fixture/src/kernel/vm/vcpu.rs" >"$fixture/mutated"
    mv "$fixture/mutated" "$fixture/src/kernel/vm/vcpu.rs"
    if check >/dev/null 2>&1; then
        echo "$description" >&2
        exit 1
    fi
}

write_valid_fixture
check

mutate 'hardware activation must precede timer programming and publication' \
    '2s/activate_hardware/activate_later/'
mutate 'hardware activation failure must propagate immediately' \
    '2s/\.map_err(HardwareTransitionError::Hardware)?;/;/'
mutate 'host timer programming must precede active publication' \
    '3s/set_host_timer_enabled(!timer_asserted)/set_host_timer_later()/'
mutate 'active state must be published only at the transaction seam' \
    '7s/set_raw/publish_later/'
mutate 'timer-programming failure must detach activated hardware' \
    '4s/deactivate_hardware/leave_hardware_active/'
mutate 'timer-programming failure must not continue past rollback' \
    '5s/return match rollback/return Err(timer); \/\/ match rollback/'
mutate 'publication failure must detach activated hardware' \
    '8s/deactivate_hardware/leave_hardware_active/'
mutate 'publication failure must restore the host timer' \
    '9s/set_host_timer_enabled(true)/leave_host_timer_masked()/'
mutate 'teardown must clear active publication first' \
    '15s/active_vcpu::clear/active_vcpu::clear_later/'
mutate 'active-publication removal failure must stop teardown' \
    '15s/\.map_err(HardwareTransitionError::Active)?;/;/'
mutate 'normal teardown must detach hardware' \
    '16s/deactivate_hardware/leave_hardware_active/'
mutate 'hardware-detach failure must stop timer restoration' \
    '16s/\.map_err(HardwareTransitionError::Hardware)?;/;/'
mutate 'normal teardown must restore the host timer last' \
    '17s/set_host_timer_enabled(true)/leave_host_timer_masked()/'

write_valid_fixture
sed '2s/activate_hardware/activate_later/' \
    "$fixture/src/kernel/vm/vcpu.rs" >"$fixture/mutated"
printf '%s\n' \
    'fn unrelated_decoy() { crate::hal::vm::activate_hardware(); }' \
    >>"$fixture/mutated"
mv "$fixture/mutated" "$fixture/src/kernel/vm/vcpu.rs"
if check >/dev/null 2>&1; then
    echo 'transition ordering must be checked inside activate, not across unrelated functions' >&2
    exit 1
fi
