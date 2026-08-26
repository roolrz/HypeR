#!/bin/sh
# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

# Exercise two-phase replicated-local IRQ installation against regressions.
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)
fixture=$(mktemp -d "${TMPDIR:-/tmp}/hyper-local-irq-test.XXXXXX")
trap 'rm -rf "$fixture"' EXIT HUP INT TERM

copy_sources() {
    rm -rf "$fixture/src"
    mkdir -p "$fixture/src/kernel/irq" "$fixture/src/kernel/device" \
        "$fixture/src/kernel/vm" "$fixture/src/arch/aarch64" \
        "$fixture/src/arch/riscv64" "$fixture/src/arch/x86_64"
    cp "$root/src/kernel/irq/interrupt.rs" "$fixture/src/kernel/irq/interrupt.rs"
    cp "$root/src/kernel/device/serial.rs" "$fixture/src/kernel/device/serial.rs"
    cp "$root/src/kernel/vm/mod.rs" "$fixture/src/kernel/vm/mod.rs"
    cp "$root/src/arch/aarch64/registers.rs" "$fixture/src/arch/aarch64/registers.rs"
    cp "$root/src/arch/aarch64/interrupts.rs" "$fixture/src/arch/aarch64/interrupts.rs"
    cp "$root/src/arch/aarch64/smp.rs" "$fixture/src/arch/aarch64/smp.rs"
    cp "$root/src/arch/riscv64/interrupts.rs" "$fixture/src/arch/riscv64/interrupts.rs"
    cp "$root/src/arch/riscv64/mod.rs" "$fixture/src/arch/riscv64/mod.rs"
    cp "$root/src/arch/x86_64/interrupts.rs" "$fixture/src/arch/x86_64/interrupts.rs"
    cp "$root/src/kernel/irq/cross_call.rs" "$fixture/src/kernel/irq/cross_call.rs"
}

check() {
    HYPER_LOCAL_IRQ_LIFECYCLE_ROOT="$fixture" \
        sh "$root/tests/ci/check-local-irq-lifecycle-contract.sh"
}

mutate() {
    description=$1
    source_file=$2
    expression=$3
    copy_sources
    before=$(cksum "$fixture/$source_file")
    sed "$expression" "$fixture/$source_file" >"$fixture/mutated"
    mv "$fixture/mutated" "$fixture/$source_file"
    after=$(cksum "$fixture/$source_file")
    if [ "$before" = "$after" ]; then
        echo "mutation did not change $source_file: $description" >&2
        exit 1
    fi
    if check >/dev/null 2>&1; then
        echo "$description" >&2
        exit 1
    fi
}

copy_sources
check

mutate 'deliverable late preparation was accepted' src/kernel/irq/interrupt.rs \
    '/^    pub fn prepare_shared_mapping(/,/^    }/s/lifecycle: MappingLifecycle::Prepared/lifecycle: MappingLifecycle::Active/'
mutate 'activation without Enabling publication was accepted' src/kernel/irq/interrupt.rs \
    '/^pub fn activate(/,/^}/s/MappingLifecycle::Enabling/MappingLifecycle::Prepared/'
mutate 'activation before cross-CPU enable was accepted' src/kernel/irq/interrupt.rs \
    '/^pub fn activate(/,/^}/s/LocalLifecycleOperation::Enable/LocalLifecycleOperation::Configure/'
mutate 'compensated local rejection was still made fatal' src/kernel/irq/interrupt.rs \
    '/^pub fn activate(/,/^}/s/if !late && crate::kernel::cpu::frozen_topology()/if crate::kernel::cpu::frozen_topology()/'
mutate 'final removal without Disabling publication was accepted' src/kernel/irq/interrupt.rs \
    '/^pub fn unregister(/,/^}/s/MappingLifecycle::Disabling/MappingLifecycle::Active/'
mutate 'Prepared dispatch mutation was accepted' src/kernel/irq/interrupt.rs \
    '/^    fn dispatch_one(/,/^    }/s/== MappingLifecycle::Prepared/== MappingLifecycle::Enabling/'
mutate 'non-Active local enable mutation was accepted' src/kernel/irq/interrupt.rs \
    '/^    fn set_local_enabled(/,/^    }/s/!= MappingLifecycle::Active/!= MappingLifecycle::Prepared/'
mutate 'partial rollback target mutation was accepted' src/kernel/irq/interrupt.rs \
    '/^fn synchronize_local_lifecycle(/,/^}/s/let targets = \[true;/let targets = [false;/'
mutate 'recoverable final-disable rejection was accepted' src/kernel/irq/interrupt.rs \
    '/^fn synchronize_local_lifecycle(/,/^}/s/LocalLifecycleOperation::Disable => crate::kernel::crash::fatal/LocalLifecycleOperation::Disable => Some/'
mutate 'AArch64 masked RPC progress mutation was accepted' src/arch/aarch64/interrupts.rs \
    's/irq::service_kernel_rpc()/core::hint::spin_loop()/'
mutate 'RISC-V masked RPC progress mutation was accepted' src/arch/riscv64/interrupts.rs \
    's/irq::service_kernel_rpc()/core::hint::spin_loop()/'
mutate 'unknown RPC reasons were silently accepted' src/kernel/irq/cross_call.rs \
    's/reasons.has_unknown()/false/'
mutate 'reserved Kernel RPC mappings were accepted' src/kernel/irq/interrupt.rs \
    's/reject_reserved_interrupt(hardware)?;/accept_interrupt(hardware)?;/'
mutate 'RISC-V boot-hart SSIE arming was omitted' src/arch/riscv64/mod.rs \
    's/interrupts::enable_software_interrupt_source()/interrupts::leave_software_interrupt_masked()/'
mutate 'RISC-V masked idle WFI was replaced' src/arch/riscv64/mod.rs \
    's/asm!("wfi", options(nostack))/asm!("nop", options(nostack))/'
mutate 'RISC-V masked idle wait opened SIE before WFI' src/arch/riscv64/mod.rs \
    's/asm!("wfi", options(nostack))/asm!("csrsi sstatus, 2", "wfi", options(nostack))/'
mutate 'AArch64 unreachable secondary route was admitted' src/arch/aarch64/smp.rs \
    '/pub fn register_cpu/,/^}/s/affinity & 0xff >= 16/false/'
mutate 'serial activation-before-source mutation was accepted' src/kernel/device/serial.rs \
    's/port.enable_runtime_input()/port.arm_runtime_input()/'
mutate 'VM activation dependency mutation was accepted' src/kernel/vm/mod.rs \
    's/binding.activate()/binding.commit()/'
mutate 'AArch64 Kernel RPC SGI gap mutation was accepted' src/arch/aarch64/registers.rs \
    's/GIC_KERNEL_RPC_SGI = 8;/GIC_KERNEL_RPC_SGI = 13;/'
mutate 'AArch64 reschedule SGI mutation was accepted' src/arch/aarch64/registers.rs \
    's/GIC_RESCHEDULE_SGI = 9;/GIC_RESCHEDULE_SGI = 14;/'
