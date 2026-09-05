#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Protect durable saved-interrupt publication and the AArch64 guest-exit tail.
set -eu

root=${HYPER_VCPU_INTERRUPT_ROOT:-$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)}
cd "$root"

vcpu=src/kernel/vm/vcpu/transition.rs
device=src/kernel/vm/device/aarch64.rs
registry=src/kernel/vm/registry.rs
scheduler=src/kernel/task/scheduler/state.rs
irq=src/kernel/entry/irq.rs
linux=src/kernel/vm/linux/mod.rs

activate=$(mktemp "${TMPDIR:-/tmp}/hyper-vcpu-interrupt-activate.XXXXXX")
receive=$(mktemp "${TMPDIR:-/tmp}/hyper-vcpu-interrupt-receive.XXXXXX")
console_access=$(mktemp "${TMPDIR:-/tmp}/hyper-vcpu-console-access.XXXXXX")
console_receive=$(mktemp "${TMPDIR:-/tmp}/hyper-vcpu-console-receive.XXXXXX")
scheduler_query=$(mktemp "${TMPDIR:-/tmp}/hyper-vcpu-running-query.XXXXXX")
trap 'rm -f "$activate" "$receive" "$console_access" "$console_receive" "$scheduler_query"' EXIT HUP INT TERM
sed -n '/^[^[:space:]].*unsafe fn activate(/,/^}/p' "$vcpu" >"$activate"
sed -n '/^pub(super) fn receive_console_input/,/^}/p' "$device" >"$receive"
sed -n '/^    fn access(/,/^    fn receive(/p' "$device" | sed '$d' >"$console_access"
sed -n '/^    fn receive(/,/^}/p' "$device" | sed '$d' >"$console_receive"
sed -n '/^    pub fn running_vcpu_cpu(/,/^    pub fn current_user(/p' "$scheduler" | sed '$d' >"$scheduler_query"

line_first() {
    rg -n "$2" "$1" | sed -n '1s/:.*//p'
}

line_last() {
    rg -n "$2" "$1" | sed -n '$s/:.*//p'
}

require_order() {
    first=$(line_first "$1" "$2")
    second=$(line_first "$1" "$3")
    if [ -z "$first" ] || [ -z "$second" ] || [ "$first" -ge "$second" ]; then
        echo "$4" >&2
        exit 1
    fi
}

take_count=$(rg -o 'take_interrupt_reconcile\(' "$activate" | wc -l | tr -d ' ')
if [ "$take_count" -ne 2 ]; then
    echo 'vCPU activation must claim reconcile work before both controller transactions' >&2
    exit 1
fi

require_order "$activate" 'take_interrupt_reconcile\(' 'activate_hardware\(' \
    'initial reconcile claim must precede hardware/controller activation'
last_take=$(line_last "$activate" 'take_interrupt_reconcile\(')
publication=$(line_first "$activate" 'active_vcpu::set_raw\(')
if [ -z "$last_take" ] || [ -z "$publication" ] || [ "$last_take" -ge "$publication" ]; then
    echo 'final reconcile claim must precede active-vCPU publication' >&2
    exit 1
fi
require_order "$activate" 'reconcile_active_interrupts\(' 'active_vcpu::set_raw\(' \
    'claimed final work must reconcile before active-vCPU publication'

if rg -q 'active_vcpu' "$receive"; then
    echo 'host console input must not depend on a CPU-local active-vCPU borrow' >&2
    exit 1
fi
require_order "$receive" '\.receive\(' 'publish_interrupt_reconcile\(' \
    'saved device/controller mutation must complete before durable publication'

for method in "$console_access" "$console_receive"; do
    method_updates=$(rg -o 'update\(self\.console_interrupt' "$method" | wc -l | tr -d ' ')
    closure_updates=$(sed -n '/self\.console\.with/,/^            })/p' "$method" |
        rg -o 'update\(self\.console_interrupt' | wc -l | tr -d ' ')
    if [ "$method_updates" -ne 1 ] || [ "$closure_updates" -ne 1 ]; then
        echo 'console mutation and vGIC line update must share the Console->vGIC lock transaction' >&2
        exit 1
    fi
done

rg -q 'InterruptSpinLock<Option<ConsoleRoute>' "$device" || {
    echo 'console routing must be generation-replaceable under an IRQ-safe lock' >&2
    exit 1
}
rg -q 'vm: VmId' "$device" &&
    rg -q 'thread: crate::kernel::task::thread::ThreadId' "$device" || {
    echo 'console route must retain exact VM generation and Thread identity' >&2
    exit 1
}
rg -q 'update_saved_guest_device_interrupt' "$device" || {
    echo 'console input must update saved interrupt state without active hardware' >&2
    exit 1
}
rg -q 'running_vcpu_cpu' "$registry" && rg -q 'request_guest_exit' "$registry" || {
    echo 'VM publication must query scheduler authority before a guest-exit prompt' >&2
    exit 1
}
rg -q 'ThreadState::Running =>' "$scheduler_query" &&
    rg -q 'then_some\(Some\(cpu\)\)' "$scheduler_query" &&
    rg -U -q 'ThreadState::Dormant\s*\| ThreadState::Ready\s*\| ThreadState::Blocked\s*\| ThreadState::Migrating\s*\| ThreadState::Terminated => Ok\(None\)' "$scheduler_query" || {
    echo 'scheduler location query must classify running and non-running transitions' >&2
    exit 1
}
rg -q 'current\.vm == expected_vm && current\.thread == expected_thread' "$device" || {
    echo 'console teardown must match the exact VM generation and Thread route' >&2
    exit 1
}
rg -U -q 'Error::EndpointClosed[\s\S]*clear_console_route_exact\(route\.vm, route\.thread\);[\s\S]*ConsoleInputDisposition::from_guest_claim\(true\)' "$device" || {
    echo 'closed vCPU endpoints must retire the exact console route without crossing into Native input' >&2
    exit 1
}
if rg -q 'try_publish_console_route' "$registry" ||
    ! rg -q 'try_publish_console_route' "$linux"; then
    echo 'host-console selection must remain explicit Linux service policy, not registry policy' >&2
    exit 1
fi
rg -U -q 'Err\(error\) => \{\s*restore_reconcile_if_claimed\(execution, reconcile_claimed\);\s*release_execution_or_fail\(execution, execution_claim\);' "$activate" || {
    echo 'hardware activation failure must restore claimed reconcile work before releasing execution' >&2
    exit 1
}
rg -U -q 'if let Err\(error\) = crate::hal::vm::reconcile_active_interrupts[\s\S]*restore_reconcile_if_claimed\(execution_ref, true\);\s*rollback_unpublished_activation\(execution_ref, execution_claim, consumed_reconcile\);' "$activate" || {
    echo 'final reconcile failure must restore work before unpublished activation rollback' >&2
    exit 1
}
rg -q 'current_interrupt_reconcile_pending' "$irq" && rg -q 'guest_irq_postlude' "$irq" || {
    echo 'AArch64 IRQ tail must select guest reconciliation from durable VM work' >&2
    exit 1
}
