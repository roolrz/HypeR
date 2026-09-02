#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Prove that the AArch64 guest-unwind ratchets reject representative regressions.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/hyper-guest-unwind-test.XXXXXX")
trap 'rm -rf "$fixture"' EXIT HUP INT TERM

copy_tree() {
    rm -rf "$fixture/src"
    mkdir -p "$fixture/src/arch/aarch64" "$fixture/src/hal/selected" \
        "$fixture/src/kernel/entry/vmexit" "$fixture/src/kernel/time" \
        "$fixture/src/kernel/task/scheduler" "$fixture/src/kernel/vm/vcpu" "$fixture/src/kernel/vm/device" \
        "$fixture/src/time"
    cp "$root/src/arch/aarch64/context.S" "$fixture/src/arch/aarch64/context.S"
    cp "$root/src/arch/aarch64/context.rs" "$fixture/src/arch/aarch64/context.rs"
    cp "$root/src/arch/aarch64/exception.rs" "$fixture/src/arch/aarch64/exception.rs"
    cp "$root/src/arch/aarch64/registers.rs" "$fixture/src/arch/aarch64/registers.rs"
    cp "$root/src/arch/aarch64/vsysreg.rs" "$fixture/src/arch/aarch64/vsysreg.rs"
    cp "$root/src/arch/aarch64/vectors.S" "$fixture/src/arch/aarch64/vectors.S"
    cp "$root/src/hal/selected/vm.rs" "$fixture/src/hal/selected/vm.rs"
    cp "$root/src/kernel/entry/vmexit.rs" "$fixture/src/kernel/entry/vmexit.rs"
    cp "$root/src/kernel/entry/vmexit/selected.rs" \
        "$fixture/src/kernel/entry/vmexit/selected.rs"
    cp "$root/src/kernel/entry/irq.rs" "$fixture/src/kernel/entry/irq.rs"
    cp "$root/src/kernel/task/scheduler/mod.rs" "$fixture/src/kernel/task/scheduler/mod.rs"
    cp "$root/src/kernel/vm/device.rs" "$fixture/src/kernel/vm/device.rs"
    cp "$root/src/kernel/vm/device/aarch64.rs" "$fixture/src/kernel/vm/device/aarch64.rs"
    cp "$root/src/kernel/vm/endpoint.rs" "$fixture/src/kernel/vm/endpoint.rs"
    cp "$root/src/kernel/vm/endpoint_wait.rs" "$fixture/src/kernel/vm/endpoint_wait.rs"
    cp "$root/src/kernel/vm/endpoint_state.rs" "$fixture/src/kernel/vm/endpoint_state.rs"
    cp "$root/src/kernel/vm/memory.rs" "$fixture/src/kernel/vm/memory.rs"
    cp "$root/src/kernel/vm/registry.rs" "$fixture/src/kernel/vm/registry.rs"
    cp "$root/src/kernel/time/timers.rs" "$fixture/src/kernel/time/timers.rs"
    cp "$root/src/kernel/vm/vcpu/runner.rs" "$fixture/src/kernel/vm/vcpu/runner.rs"
    cp "$root/src/kernel/vm/vcpu/lifecycle.rs" "$fixture/src/kernel/vm/vcpu/lifecycle.rs"
    cp "$root/src/kernel/vm/vcpu/transition.rs" "$fixture/src/kernel/vm/vcpu/transition.rs"
    cp "$root/src/time/owned_queue.rs" "$fixture/src/time/owned_queue.rs"
}

check() {
    HYPER_GUEST_UNWIND_ROOT="$fixture" \
        sh "$root/tests/ci/check-aarch64-guest-unwind-contract.sh"
}

mutate() {
    description=$1
    file=$2
    expression=$3
    copy_tree
    sed "$expression" "$fixture/$file" >"$fixture/mutated"
    mv "$fixture/mutated" "$fixture/$file"
    if check >/dev/null 2>&1; then
        echo "$description" >&2
        exit 1
    fi
}

copy_tree
check

mutate 'terminal vector unwind must not return to vector restoration' \
    src/arch/aarch64/vectors.S 's/br      x1/blr     x1/'
mutate 'terminal capture must precede lower-world closure' \
    src/arch/aarch64/exception.rs 's/\.capture_terminal(frame, cause)/.capture_terminal_later(frame, cause)/'
mutate 'terminal hardware must detach before report consumption' \
    src/kernel/vm/vcpu/runner.rs 's/super::transition::detach_stopped/super::transition::detach_stopped_later/'
mutate 'endpoint close must precede execution-claim release' \
    src/kernel/vm/vcpu/runner.rs 's/close_vcpu_endpoint/close_vcpu_endpoint_after_release/'
mutate 'HAL must preserve a typed terminal reason' \
    src/hal/selected/vm.rs 's/enum VcpuTerminalReason/enum ErasedTerminalReason/'
mutate 'terminal payload must retain captured processor state' \
    src/arch/aarch64/context.rs \
    's/processor_state: context_ref.processor_state/processor_state: 0/'
mutate 'synchronous terminal payload must retain the decoded exit' \
    src/arch/aarch64/exception.rs \
    's/GuestSynchronousTerminal::Failed { exit, failure }/GuestSynchronousTerminal::Undecodable/'
mutate 'synchronous emulation failure must retain its typed error' \
    src/arch/aarch64/vsysreg.rs \
    's/Err(error) =>/Err(_error) =>/'
mutate 'a CPU-affine guard must not span a migratable guest run' \
    src/kernel/vm/vcpu/runner.rs \
    's/fn run_current() {/fn run_current() { let _migration_bug: Option<InterruptMaskGuard<LocalMask>> = None;/'
mutate 'terminal continuation must remain IRQ-masked until scheduler exit' \
    src/kernel/vm/vcpu/runner.rs \
    's/let report = execution_ref.take_terminal_mmio_report();/let report = execution_ref.take_terminal_mmio_report(); crate::hal::irq::enable_local();/'
mutate 'guest MMIO must not leave a stale global crash supplement' \
    src/kernel/entry/vmexit/selected.rs \
    's/) -> hyper::vm::exit::MmioAction {/) -> hyper::vm::exit::MmioAction { let _ = crate::kernel::crash::publish_terminal_supplement(format_args!("stale"));/'
mutate 'unhandled MMIO must retain its report in the vCPU execution' \
    src/kernel/vm/device/aarch64.rs 's/publish_terminal_mmio_report/drop_terminal_mmio_report/'
mutate 'WFx ISS.TI must not reverse WFI and WFE' \
    src/arch/aarch64/vsysreg.rs 's/ESR_WFX_TI_WFE != 0/ESR_WFX_TI_WFE == 0/'
mutate 'WFI must use the reserved endpoint timer before parking' \
    src/kernel/vm/vcpu/runner.rs 's/arm_wfi_timer/allocate_wfi_timer/'
mutate 'WFI endpoint notification must use the registered-notification facade' \
    src/kernel/vm/endpoint.rs \
    's/notify_registered_fair_boundary(ticket)/resolve_wait(ticket, crate::kernel::task::WaitOutcome::Notified)/'
mutate 'reserved expiry must claim ownership under the queue lock' \
    src/time/owned_queue.rs 's/claim(event, node.claim_context)/claim_after_unlock(event, node.claim_context)/'
mutate 'abandoned reserved expiry must recycle its node' \
    src/time/owned_queue.rs 's/impl Drop for ExpiredTimer/impl Drop for AbandonedExpiredTimer/'
mutate 'reserved callback completion must not permit rearm before token retirement' \
    src/kernel/time/timers.rs 's/ReservationState::Completed/ReservationState::PrematureIdle/g'
mutate 'administrative stop must be observed before guest reactivation' \
    src/kernel/vm/vcpu/runner.rs 's/administrative_stop_reason/late_administrative_stop_reason/g'
mutate 'administrative stop must select typed IRQ unwind' \
    src/kernel/entry/irq.rs 's/Action::StopGuest/Action::Resume { postlude: None }/'
mutate 'IRQ-tail must observe stop before guest reactivation' \
    src/kernel/entry/irq.rs \
    's/complete_detached_stop_if_requested/complete_detached_stop_after_reactivation/'
mutate 'HardwareDetached must not precede execution release' \
    src/kernel/vm/vcpu/runner.rs \
    's/publish_hardware_detached_and_arm_reap/publish_hardware_detached_before_release/'
mutate 'vCPU reap publication must follow payload destruction' \
    src/kernel/task/scheduler/mod.rs 's/complete_vcpu_reap/complete_vcpu_reap_before_drop/'
mutate 'exact vCPU reap completion must remain non-cloneable' \
    src/kernel/vm/registry.rs \
    's/#\[derive(Debug, Eq, PartialEq)\]/#[derive(Clone, Copy, Debug, Eq, PartialEq)]/'
mutate 'active address spaces must not reach field destruction' \
    src/kernel/vm/memory.rs 's/destruction_is_safe(state)/true/'
mutate 'repeated VMID activation must preserve Active ownership' \
    src/kernel/vm/memory.rs 's/activation_may_begin(state)/true/'
