#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Keep late replicated-local IRQ installation transactional and preserve the
# compact dedicated-SGI allocation around the shared Kernel RPC doorbell.
set -eu

root=${HYPER_LOCAL_IRQ_LIFECYCLE_ROOT:-$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)}
cd "$root"

fixture=$(mktemp -d "${TMPDIR:-/tmp}/hyper-local-irq-check.XXXXXX")
trap 'rm -rf "$fixture"' EXIT HUP INT TERM
interrupt=src/kernel/irq/interrupt.rs
sed -n '/^    pub fn prepare_shared_mapping(/,/^    }/p' "$interrupt" >"$fixture/prepare.rs"
sed -n '/^pub fn activate(/,/^}/p' "$interrupt" >"$fixture/activate.rs"
sed -n '/^pub fn unregister(/,/^}/p' "$interrupt" >"$fixture/unregister.rs"
sed -n '/^    fn dispatch_one(/,/^    }/p' "$interrupt" >"$fixture/dispatch.rs"
sed -n '/^    fn set_local_enabled(/,/^    }/p' "$interrupt" >"$fixture/local-enable.rs"
sed -n '/^fn synchronize_local_lifecycle(/,/^}/p' "$interrupt" >"$fixture/synchronize.rs"
sed -n '/^fn install(/,/^}/p' src/kernel/device/serial.rs >"$fixture/serial-install.rs"
sed -n '/^pub(crate) fn initialize(/,/^}/p' src/kernel/vm/mod.rs >"$fixture/vm-initialize.rs"
sed -n '/^pub fn wait_for_interrupt_masked(/,/^}/p' \
    src/arch/riscv64/mod.rs >"$fixture/riscv-masked-wait.rs"

require() {
    pattern=$1
    source=$2
    message=$3
    LC_ALL=C rg -q -U "$pattern" "$source" || {
        echo "$message" >&2
        exit 1
    }
}

require_order() {
    source=$1
    first_pattern=$2
    second_pattern=$3
    message=$4
    first_line=$(LC_ALL=C rg -n -m1 "$first_pattern" "$source" | cut -d: -f1 || true)
    second_line=$(LC_ALL=C rg -n -m1 "$second_pattern" "$source" | cut -d: -f1 || true)
    if [ -z "$first_line" ] || [ -z "$second_line" ] || [ "$first_line" -ge "$second_line" ]; then
        echo "$message" >&2
        exit 1
    fi
}

require 'struct LocalControllerSlot\(UnsafeCell<Option<LocalController>>\)' "$interrupt" \
    'each CPU must own a lockless local-controller capability'
require 'lifecycle: MappingLifecycle::Prepared' "$fixture/prepare.rs" \
    'late IRQ preparation must publish a non-deliverable Prepared mapping'
require 'LocalLifecycleOperation::Configure' "$fixture/prepare.rs" \
    'late IRQ preparation must configure every frozen participant while disabled'

require_order "$fixture/activate.rs" 'MappingLifecycle::Enabling' \
    'LocalLifecycleOperation::Enable' \
    'activation must make the handler dispatchable before enabling hardware'
require_order "$fixture/activate.rs" 'LocalLifecycleOperation::Enable' \
    'MappingLifecycle::Active' \
    'activation may commit Active only after all CPUs acknowledge enable'
require 'if let Err\(error\) = enabled' \
    "$fixture/activate.rs" \
    'a compensated replicated-local rejection must return its activation capability'
require 'mapping\]\.lifecycle = MappingLifecycle::Prepared' "$fixture/activate.rs" \
    'failed replicated-local activation must restore Prepared state'

require 'if late \{[[:space:]]*mapping\.lifecycle = MappingLifecycle::Disabling;[[:space:]]*return Ok\(Some' \
    "$fixture/unregister.rs" 'final late-handler removal must remain dispatchable while disabling'
require_order "$fixture/unregister.rs" 'LocalLifecycleOperation::Disable' \
    'mapping\.handlers\.swap_remove\(handler\)' \
    'the final late handler may be removed only after all CPUs acknowledge disable'

require '(?s)lifecycle[[:space:]]*== MappingLifecycle::Prepared[[:space:]]*\{.*set_hardware_enabled\(hardware, false\).*controller\.end\(hardware\).*return DispatchOutcome::Prepared' \
    "$fixture/dispatch.rs" 'Prepared IRQ delivery must be masked and EOIed without calling handlers'
require 'lifecycle != MappingLifecycle::Active[[:space:]]*\{[[:space:]]*return Err\(Error::MappingBusy\.into\(\)\)|lifecycle != MappingLifecycle::Active[[:space:]]*\{[[:space:]]*return Err\(TransitionFailure::NotApplied\(Error::MappingBusy\)\)' \
    "$fixture/local-enable.rs" 'local mask control must reject non-Active mappings'

require 'let targets = \[true; hyper::cpu::MAX_CPUS\]' "$fixture/synchronize.rs" \
    'replicated-local lifecycle must target the complete FrozenTopology'
require 'let rollback = super::cross_call::execute\([^;]*&targets' "$fixture/synchronize.rs" \
    'controller rejection must compensate the complete original target set'
require 'LocalLifecycleOperation::Disable => crate::kernel::crash::fatal' \
    "$fixture/synchronize.rs" \
    'a rejected final disable must fail-stop instead of changing prior per-CPU mask state'
require 'crate::kernel::cpu::frozen_topology\(\)\.ok_or' "$fixture/synchronize.rs" \
    'late lifecycle transactions must snapshot FrozenTopology'

for source in src/arch/aarch64/interrupts.rs src/arch/riscv64/interrupts.rs \
    src/arch/x86_64/interrupts.rs; do
    require 'fn wait_for_lock_owner\(\) \{[^}]*crate::arch::irq::service_kernel_rpc\(\);' \
        "$source" "$source must poll Kernel RPC while IRQ-masked lock contention blocks delivery"
done
require 'if reasons\.has_unknown\(\) \{[[:space:]]*poison\("kernel RPC reason mailbox contains unknown bits"\)' \
    src/kernel/irq/cross_call.rs 'unknown Kernel RPC reason bits must fail-stop'
require 'fn reject_reserved_interrupt\([^}]*kernel_rpc_interrupt\(\)\.is_some_and\(\|kernel_rpc\| interrupt == kernel_rpc\)[^}]*Error::ReservedInterrupt' \
    "$interrupt" 'the physical Kernel RPC doorbell must be reserved from IRQ domains'
reserved_checks=$(LC_ALL=C rg -c 'reject_reserved_interrupt\(hardware\)\?;' "$interrupt")
if [ "$reserved_checks" -ne 3 ]; then
    echo "every IRQ mapping entry point must reject the Kernel RPC doorbell (found $reserved_checks, expected 3)" >&2
    exit 1
fi
require_order "$interrupt" 'arm_kernel_rpc_source\(\)' 'kernel_rpc_interrupt\(\)' \
    'each CPU must arm its architecture Kernel RPC source before inspecting its registry ID'
require 'pub fn arm_kernel_rpc_source\(\) \{[[:space:]]*interrupts::enable_software_interrupt_source\(\);' \
    src/arch/riscv64/mod.rs 'the RISC-V boot hart must arm SSIE through the generic transport hook'
require 'mask = in\(reg\) registers::SIE_SSIE as usize' src/arch/riscv64/interrupts.rs \
    'the RISC-V Kernel RPC hook must enable the supervisor software source'
require 'asm!\("wfi", options\(nostack\)\)' "$fixture/riscv-masked-wait.rs" \
    'the RISC-V masked idle wait must use WFI while SSTATUS.SIE remains clear'
if LC_ALL=C rg -q 'csrsi[[:space:]]+sstatus' "$fixture/riscv-masked-wait.rs"; then
    echo 'the RISC-V masked idle wait must not open an interrupt window before WFI' >&2
    exit 1
fi
affinity_checks=$(LC_ALL=C rg -c 'affinity & 0xff >= 16' src/arch/aarch64/smp.rs)
if [ "$affinity_checks" -ne 2 ]; then
    echo "boot and secondary AArch64 CPUs must reject unreachable SGI affinities" >&2
    exit 1
fi

require_order "$fixture/serial-install.rs" 'prepare_shared_mapping' '\*slot = Some\(RuntimeConsole' \
    'serial IRQ preparation must precede handler-context publication'
require_order "$fixture/serial-install.rs" '\*slot = Some\(RuntimeConsole' 'enable_runtime_input' \
    'serial handler context must be published before the source is armed'
require_order "$fixture/serial-install.rs" 'enable_runtime_input' 'interrupt::activate' \
    'serial source state must be published before controller activation'
require_order "$fixture/vm-initialize.rs" 'timer::prepare' 'initialize_devices' \
    'VM IRQ mappings must be prepared before publishing device dependencies'
require_order "$fixture/vm-initialize.rs" 'initialize_interrupts' 'binding\.activate\(\)' \
    'VM IRQ mappings must activate only after virtualization state is published'

registers=src/arch/aarch64/registers.rs
require 'GIC_KERNEL_RPC_SGI = 8;' "$registers" \
    'AArch64 Kernel RPC must use the first project-reserved non-secure SGI'
require 'GIC_RESCHEDULE_SGI = 9;' "$registers" \
    'AArch64 reschedule must use the next dedicated SGI'
require 'GIC_CRASH_STOP_SGI = 10;' "$registers" \
    'AArch64 crash-stop must use the next dedicated SGI'
