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
        '    execution.claim_execution();' \
        '    super::memory::activate();' \
        '    let timer_asserted = match unsafe { crate::hal::vm::activate_hardware() } {' \
        '        Ok(asserted) => asserted,' \
        '        Err(error) => {' \
        '            release_execution_or_fail(execution);' \
        '            return Err(HardwareTransitionError::Hardware(error));' \
        '        }' \
        '    };' \
        '    if let Err(timer) = super::timer::set_host_timer_enabled(!timer_asserted) {' \
        '        let rollback = unsafe { crate::hal::vm::deactivate_hardware() };' \
        '        return match rollback { Ok(()) => { release_execution_or_fail(execution); Err(timer) }, Err(hardware) => fatal_ambiguous_hardware("rollback failed", hardware) };' \
        '    }' \
        '    if let Err(publication) = unsafe { super::active_vcpu::set_raw(execution) } {' \
        '        let rollback = unsafe { crate::hal::vm::deactivate_hardware() };' \
        '        return match rollback {' \
        '            Ok(()) => match super::timer::set_host_timer_enabled(true) {' \
        '                Ok(()) => { release_execution_or_fail(execution); Err(publication) }' \
        '                Err(timer) => { release_execution_or_fail(execution); Err(timer) }' \
        '            },' \
        '            Err(hardware) => fatal_ambiguous_hardware("rollback failed", hardware),' \
        '        };' \
        '    }' \
        '    Ok(())' \
        '}' \
        'pub(crate) unsafe fn deactivate(execution: &mut VcpuExecution) -> Result<(), Error> {' \
        '    super::active_vcpu::clear(execution).map_err(HardwareTransitionError::Active)?;' \
        '    if let Err(error) = unsafe { crate::hal::vm::deactivate_hardware() } {' \
        '        fatal_ambiguous_hardware("detach failed", error);' \
        '    }' \
        '    release_execution_or_fail(execution);' \
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
    '4s/activate_hardware/activate_later/'
mutate 'hardware activation failure must release VM execution' \
    '7s/release_execution_or_fail/retain_execution_forever/'
mutate 'host timer programming must precede active publication' \
    '11s/set_host_timer_enabled(!timer_asserted)/set_host_timer_later()/'
mutate 'active state must be published only at the transaction seam' \
    '15s/set_raw/publish_later/'
mutate 'timer-programming failure must detach activated hardware' \
    '12s/deactivate_hardware/leave_hardware_active/'
mutate 'timer-programming failure must not continue past rollback' \
    '13s/return match rollback/return Err(timer); \/\/ match rollback/'
mutate 'ambiguous timer rollback must fail-stop' \
    '13s/fatal_ambiguous_hardware/ignore_ambiguous_hardware/'
mutate 'publication failure must detach activated hardware' \
    '16s/deactivate_hardware/leave_hardware_active/'
mutate 'publication failure must restore the host timer' \
    '18s/set_host_timer_enabled(true)/leave_host_timer_masked()/'
mutate 'ambiguous publication rollback must fail-stop' \
    '22s/fatal_ambiguous_hardware/ignore_ambiguous_hardware/'
mutate 'teardown must clear active publication first' \
    '28s/active_vcpu::clear/active_vcpu::clear_later/'
mutate 'active-publication removal failure must stop teardown' \
    '28s/\.map_err(HardwareTransitionError::Active)?;/;/'
mutate 'normal teardown must detach hardware' \
    '29s/deactivate_hardware/leave_hardware_active/'
mutate 'ambiguous hardware-detach failure must fail-stop' \
    '30s/fatal_ambiguous_hardware/ignore_ambiguous_hardware/'
mutate 'normal teardown must release VM execution' \
    '32s/release_execution_or_fail/retain_execution_forever/'
mutate 'normal teardown must restore the host timer last' \
    '33s/set_host_timer_enabled(true)/leave_host_timer_masked()/'

write_valid_fixture
sed '4s/activate_hardware/activate_later/' \
    "$fixture/src/kernel/vm/vcpu.rs" >"$fixture/mutated"
printf '%s\n' \
    'fn unrelated_decoy() { crate::hal::vm::activate_hardware(); }' \
    >>"$fixture/mutated"
mv "$fixture/mutated" "$fixture/src/kernel/vm/vcpu.rs"
if check >/dev/null 2>&1; then
    echo 'transition ordering must be checked inside activate, not across unrelated functions' >&2
    exit 1
fi
