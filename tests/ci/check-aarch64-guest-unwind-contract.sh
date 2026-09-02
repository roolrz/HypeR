#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Protect the typed AArch64 guest-terminal capture and detach transaction.
set -eu

root=${HYPER_GUEST_UNWIND_ROOT:-$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)}
cd "$root"

fixture=$(mktemp -d "${TMPDIR:-/tmp}/hyper-guest-unwind-check.XXXXXX")
trap 'rm -rf "$fixture"' EXIT HUP INT TERM

sed -n '/^fn run_current() {/,/^}/p' src/kernel/vm/vcpu/runner.rs >"$fixture/runner.rs"
sed -n '/^pub(super) unsafe fn detach_stopped(/,/^}/p' \
    src/kernel/vm/vcpu/transition.rs >"$fixture/detach.rs"
sed -n '/^    pub(super) fn finish(/,/^    }/p' \
    src/kernel/vm/vcpu/transition.rs >"$fixture/finish.rs"
sed -n '/^pub(crate) fn dispatch_memory_fault(/,/^}/p' \
    src/kernel/entry/vmexit.rs >"$fixture/memory.rs"
sed -n '/^pub(in crate::kernel) fn dispatch_mmio(/,/^}/p' \
    src/kernel/vm/device/aarch64.rs >"$fixture/device-mmio.rs"
sed -n '/^fn capture_terminal_guest(/,/^}/p' src/arch/aarch64/exception.rs >"$fixture/capture.rs"
sed -n '/^fn capture_waiting_guest(/,/^}/p' src/arch/aarch64/exception.rs >"$fixture/capture-wait.rs"
sed -n '/^fn guest_irq_tail(/,/^}/p' src/kernel/entry/irq.rs >"$fixture/guest-irq-tail.rs"
sed -n '/^fn finish_detached_administrative_stop(/,/^}/p' \
    src/kernel/vm/vcpu/runner.rs >"$fixture/admin-detach.rs"
sed -n '/^    pub(super) fn activate_identifier_for_install(/,/^    }/p' \
    src/kernel/vm/memory.rs >"$fixture/vmid-activate.rs"

require() {
    file=$1
    pattern=$2
    message=$3
    if ! rg -q -U "$pattern" "$file"; then
        echo "$message" >&2
        exit 1
    fi
}

reject() {
    file=$1
    pattern=$2
    message=$3
    if rg -q -U "$pattern" "$file"; then
        echo "$message" >&2
        exit 1
    fi
}

line_in() {
    file=$1
    pattern=$2
    rg -n "$pattern" "$file" | sed -n '1s/:.*//p'
}

require_order() {
    file=$1
    first_pattern=$2
    second_pattern=$3
    message=$4
    first=$(line_in "$file" "$first_pattern")
    second=$(line_in "$file" "$second_pattern")
    if [ -z "$first" ] || [ -z "$second" ] || [ "$first" -ge "$second" ]; then
        echo "$message" >&2
        exit 1
    fi
}

require src/arch/aarch64/context.S \
    '(?s)stp[[:space:]]+x19, x20.*stp[[:space:]]+x29, x30.*stp[[:space:]]+d8, d9.*stp[[:space:]]+d14, d15.*mrs[[:space:]]+x1, fpcr.*mrs[[:space:]]+x1, fpsr' \
    'guest run must save the complete host callee and floating-point control state'
require src/arch/aarch64/context.S \
    '(?s)ldp[[:space:]]+q0, q1.*ldp[[:space:]]+q30, q31' \
    'guest run must restore every guest SIMD register'
require src/arch/aarch64/context.S \
    '(?s)aarch64_unwind_guest:.*add[[:space:]]+sp, sp, #EXCEPTION_FRAME_SIZE.*ldp[[:space:]]+x29, x30.*mov[[:space:]]+x0, #GUEST_RUN_RETURN_STOPPED.*ret' \
    'terminal unwind must discard the private vector frame and return through the saved run frame'
require src/arch/aarch64/vectors.S \
    '(?s)\.Lexception_unwind:.*cmp[[:space:]]+x0, #VECTOR_ACTION_UNWIND.*br[[:space:]]+x1' \
    'terminal vector action must branch out and never return to vector restoration'
require src/arch/aarch64/context.rs \
    '(?s)offset_of!\(VcpuContext, simd\).*VCPU_CONTEXT_SIMD_OFFSET.*offset_of!\(VcpuContext, fpcr\).*VCPU_CONTEXT_FPCR_OFFSET.*offset_of!\(VcpuContext, fpsr\).*VCPU_CONTEXT_FPSR_OFFSET' \
    'guest SIMD and floating-point assembly offsets must be statically validated'
require src/arch/aarch64/context.rs \
    '(?s)enum GuestSynchronousTerminal.*Undecodable.*Failed.*exit: super::vsysreg::GuestSyncExit.*failure: super::vsysreg::GuestSyncFailure.*enum GuestTerminalCause.*MemoryFault.*Mmio.*Synchronous\(GuestSynchronousTerminal\).*struct GuestTerminalExit.*cause: GuestTerminalCause.*syndrome: u64.*fault_address: u64.*program_counter: u64.*processor_state: u64.*vector: u64.*enum GuestRunExit.*Terminal\(GuestTerminalExit\)' \
    'terminal guest exits must carry one complete typed architecture payload'
require src/arch/aarch64/context.rs \
    '(?s)GuestRunExit::Terminal\(GuestTerminalExit.*syndrome: context_ref\.terminal_syndrome.*fault_address: context_ref\.terminal_fault_address.*program_counter: context_ref\.program_counter.*processor_state: context_ref\.processor_state.*vector: context_ref\.terminal_vector' \
    'terminal payload construction must copy the complete captured exception state'

require_order "$fixture/capture.rs" 'capture_terminal\(' 'close_captured_guest\(' \
    'terminal state must be captured before lower-world publication closes'
require src/arch/aarch64/exception.rs \
    '(?s)MemoryFaultAction::Stop[[:space:]]*=>.*GuestDispatch::Terminal.*GuestTerminalCause::MemoryFault' \
    'an explicit guest memory-policy stop must become a typed terminal exit'
require src/arch/aarch64/exception.rs \
    '(?s)MmioAction::Unhandled \| hyper::vm::exit::MmioAction::Stop.*GuestDispatch::Terminal' \
    'terminal MMIO policy must become a typed terminal exit'
require src/arch/aarch64/exception.rs \
    '(?s)action if completion\.apply.*GuestDispatch::Resume.*_ => Err\(\(\)\)' \
    'MMIO completion mismatch must remain a host invariant failure'
require src/arch/aarch64/vsysreg.rs \
    '(?s)enum GuestSyncAction.*Stop\(GuestSyncFailure\).*enum GuestSyncFailure.*VirtualInterrupt\(super::vm_vcpu::Error\).*fn software_interrupt_completion.*Err\(error\).*GuestSyncAction::Stop\(GuestSyncFailure::VirtualInterrupt\(error\)\)' \
    'synchronous emulation stop must retain the exact typed failure'
require src/arch/aarch64/exception.rs \
    '(?s)GuestSyncAction::Stop\(failure\).*GuestSynchronousTerminal::Failed \{ exit, failure \}' \
    'terminal synchronous unwind must retain the decoded exit and failure'

reject "$fixture/runner.rs" 'InterruptMaskGuard' \
    'a CPU-affine interrupt guard must not span migratable guest execution'
require_order "$fixture/runner.rs" 'crate::hal::irq::mask_local\(' 'crate::hal::vm::run\(' \
    'guest entry must explicitly mask local IRQs before the migratable run'
require_order "$fixture/runner.rs" 'crate::hal::vm::run\(' 'detach_stopped\(' \
    'typed guest return must be detached before terminal handling'
require_order "$fixture/runner.rs" 'detach_stopped\(' 'close_vcpu_endpoint\(' \
    'terminal hardware must detach before endpoint lifecycle policy'
require "$fixture/runner.rs" \
    '(?s)VcpuRunDisposition::Terminal\(terminal\).*terminal\.reason\(\).*close_vcpu_endpoint\(.*detached\.finish\(\).*take_terminal_mmio_report\(' \
    'terminal endpoint close, claim release, and report consumption must retain their order'
reject "$fixture/runner.rs" 'enable_local\(' \
    'terminal vCPU continuation must stay IRQ-masked until scheduler thread exit'
require_order "$fixture/detach.rs" 'super::active_vcpu::clear\(' \
    'deactivate_stopped_hardware\(' \
    'terminal detach must close callback publication before hardware teardown'
reject "$fixture/detach.rs" 'close_vcpu_endpoint|release_execution_or_fail' \
    'hardware detach must return a linear token instead of owning lifecycle policy or claim release'
require_order "$fixture/finish.rs" 'set_host_timer_enabled\(true\)' \
    'release_execution_or_fail\(' \
    'detached proof must restore host timing before releasing VM execution'

require src/hal/selected/vm.rs \
    '(?s)enum VcpuTerminalReason.*MemoryFault.*Mmio.*Synchronous' \
    'HAL must retain a typed terminal reason across the architecture boundary'
require src/hal/selected/vm.rs \
    '(?s)enum VcpuSynchronousTerminal.*Undecodable.*Failed.*exit: GuestSyncExit.*failure: VcpuInterruptError.*enum VcpuTerminalCause.*Synchronous\(VcpuSynchronousTerminal\).*struct VcpuTerminalExit.*cause: VcpuTerminalCause.*syndrome: u64.*fault_address: u64.*program_counter: u64.*processor_state: u64.*vector: u64.*VcpuRunDisposition.*Terminal\(VcpuTerminalExit\)' \
    'HAL must preserve the complete terminal payload as one typed value'
reject src/hal/selected/vm.rs \
    'const fn reason\(self\) -> &.*str' \
    'HAL terminal reason must not be erased into display text'

require src/kernel/vm/device/aarch64.rs \
    '(?s)admit_unhandled_mmio.*publish_terminal_mmio_report' \
    'an admitted unhandled-MMIO report must move into its active vCPU slot'
reject src/kernel/entry/vmexit/selected.rs 'publish_terminal_supplement' \
    'guest MMIO must not publish a process-global crash supplement'
require src/kernel/entry/vmexit/selected.rs \
    '(?s)guest MMIO exit arrived without an active vCPU.*invalid guest MMIO entry context' \
    'active-vCPU provenance failures must remain host-fatal'
reject src/kernel/entry/vmexit/selected.rs 'Ok\(None\).*MmioAction::Stop' \
    'missing active-vCPU ownership must not collapse into a guest policy stop'
reject "$fixture/memory.rs" 'pr_err!' \
    'guest memory-fault policy must not log before terminal hardware detach'
reject "$fixture/device-mmio.rs" 'pr_err!' \
    'guest device policy must not log before terminal hardware detach'
require src/kernel/vm/device/aarch64.rs \
    '(?s)Error::EndpointClosed.*clear_console_route_exact.*true' \
    'closed endpoint console input must clear the exact stale route without masking host UART'
reject src/arch/aarch64/exception.rs \
    'pub(?:\(crate\))?[[:space:]]+(?:struct|fn)[^\n]*ExceptionFrame' \
    'the raw architecture exception frame must not escape its private module'

require src/arch/aarch64/context.rs \
    '(?s)enum GuestRunExit.*AdministrativeStop\(GuestAdministrativeStopReason\).*ADMINISTRATIVE_STOP.*capture_administrative_stop' \
    'administrative stop must own a typed guest-run unwind disposition'
require src/kernel/entry/irq.rs \
    '(?s)current_guest_stop_requested\(interrupt\).*Action::StopGuest.*guest_postlude_required\(interrupt\)' \
    'administrative stop must win before any guest reactivation postlude is selected'
require_order "$fixture/guest-irq-tail.rs" 'cond_resched_from_irq_tail\(' \
    'complete_detached_stop_if_requested\(' \
    'IRQ-tail must reobserve durable stop after its suspended scheduling interval'
require_order "$fixture/guest-irq-tail.rs" 'complete_detached_stop_if_requested\(' \
    'vcpu::activate\(' \
    'IRQ-tail must complete administrative stop before any guest reactivation'
require src/kernel/vm/vcpu/lifecycle.rs \
    '(?s)administrative_stop_requested\(.*publish_hardware_detached\(.*arm_reap_publication\(' \
    'detached stop completion must publish hardware state before exact reap ownership'
require_order "$fixture/admin-detach.rs" 'detached\.finish\(\)' \
    'publish_hardware_detached_and_arm_reap\(' \
    'HardwareDetached must follow stopped-token timer and execution release'
require "$fixture/runner.rs" \
    '(?s)loop \{.*if let Some\(reason\) = administrative_stop_reason\(execution, current\.thread\).*super::activate\(execution\)' \
    'the vCPU runner must observe stop before every hardware activation'
require "$fixture/runner.rs" \
    '(?s)timer\.retire\(\).*if let Some\(reason\) = administrative_stop_reason\(execution, current\.thread\).*continue' \
    'a WFI continuation must retire its timer and observe stop before reentry'
require src/kernel/task/scheduler/mod.rs \
    '(?s)take_vcpu_reap_publication\(\).*drop\(thread\).*complete_vcpu_reap\(publication\)' \
    'Reaped publication must follow Thread, vCPU execution, and strong VM binding destruction'
reject src/kernel/vm/registry.rs \
    '(?s)#\[derive\([^]]*(?:Clone|Copy)[^]]*\)\][[:space:]]*pub\(in crate::kernel\) struct VcpuReapPublication' \
    'exact vCPU reap completion authority must remain linear and non-cloneable'
require src/kernel/vm/endpoint_state.rs \
    '(?s)STOP_REQUESTED.*HARDWARE_DETACHED.*REAPED.*publish_hardware_detached.*publish_reaped' \
    'administrative endpoint lifecycle must retain explicit one-way phases'
require src/kernel/vm/memory.rs \
    '(?s)impl Drop for GuestAddressSpace.*destruction_is_safe\(state\).*crate::hal::cpu::halt\(\)' \
    'active guest address-space destruction must fail-stop before owned fields are dropped'
require_order "$fixture/vmid-activate.rs" 'activation_may_begin\(state\)' \
    'mem::replace' \
    'VMID activation must reject Active without replacing it with a drop-safe state'

require src/arch/aarch64/vsysreg.rs \
    '(?s)ESR_WFX_TI_WFE != 0.*WaitInstruction::Event.*WaitInstruction::Interrupt' \
    'WFx ISS.TI must decode zero as WFI and one as WFE'
require src/arch/aarch64/vsysreg.rs \
    '(?s)Wait\(WaitInstruction::Event\) => GuestSyncAction::Advance.*Wait\(WaitInstruction::Interrupt\) => GuestSyncAction::Wait' \
    'WFE must resume while WFI selects the typed wait disposition'
require_order "$fixture/capture-wait.rs" 'capture_wait\(' 'close_captured_guest\(' \
    'WFI state must be captured before lower-world publication closes'
require src/arch/aarch64/exception.rs \
    '(?s)let applied = super::apply_guest_sync_action\(.*if action == super::GuestSyncAction::Wait.*GuestDispatch::Wait' \
    'WFI must complete its PC update before selecting the typed unwind'
require "$fixture/runner.rs" \
    '(?s)VcpuRunDisposition::Wait.*wfi_wait_ticket\(.*wfi_state\(.*detached\.finish\(.*arm_wfi_timer\(.*prepare_wfi_wait\(.*park\.complete\(\).*timer\.retire\(' \
    'WFI must detach before an allocation-free conditional park and retire its timer before reentry'
require src/kernel/vm/endpoint.rs \
    '(?s)struct VcpuEndpoint \{.*timer: crate::kernel::time::ReservedTimer,.*id: u32,.*wait: InterruptSpinLock' \
    'endpoint timer ownership must drop before every field reachable by its callback'
require src/kernel/vm/endpoint.rs \
    '(?s)state\.publication\.signal\(\).*scheduler::notify_registered_fair_boundary\(ticket\)' \
    'endpoint publication must resolve the exact stored scheduler generation'
reject src/kernel/vm/endpoint.rs \
    'scheduler::resolve_wait\([^\n]*WaitOutcome::Notified' \
    'endpoint notification must not use the scheduler timeout/cancellation facade'
require src/kernel/vm/endpoint.rs \
    '(?s)struct EndpointPark.*waiter: crate::kernel::task::WaitTicket.*scheduler::complete_park\(self\.park\).*complete_wait\(self\.waiter, outcome\)' \
    'a parked WFI must reconcile its exact endpoint ticket after scheduler resume'
require src/kernel/time/timers.rs \
    '(?s)Only exact retirement may return.*ReservationState::Completed.*ReservationState::Completed.*ReservationState::Idle' \
    'reserved timer callback completion must remain owned until exact token retirement'
require src/time/owned_queue.rs \
    '(?s)if let Some\(claim\) = node\.claim\.take\(\).*claim\(event, node\.claim_context\).*Some\(ExpiredTimer' \
    'reserved expiry must claim firing ownership before leaving the timer queue'
require src/time/owned_queue.rs \
    '(?s)impl Drop for ExpiredTimer.*RetiredTimer::Reserved.*self\.callback.*self\.recycle' \
    'abandoned reserved expiry must notify and recycle rather than strand ownership'
