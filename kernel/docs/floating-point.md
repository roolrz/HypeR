<!-- SPDX-FileCopyrightText: 2026 roolrz -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Floating-point context ownership

AArch64 Native threads and guest vCPUs restore FP/SIMD state on first use in a
machine run. An integer-only run does not load or save that state. Once loaded,
the complete register image remains resident across direct Native syscalls,
handled guest exits, and interrupts which return without scheduling.

The privileged AArch64 kernel uses `aarch64-unknown-none-softfloat`. Native
applications continue using the normal hard-float ABI. Kernel Rust code,
compiler builtins, and ordinary assembly may not use FP/SIMD registers. The
linked-image test checks this requirement outside three audited assembly leaves
which initialize, restore, or save and clear an owner's state. An unexpected
kernel FP access is a fatal synchronous exception, never a request to borrow a
user's registers.

## Run boundaries

Each initialized machine context owns all 32 128-bit vector registers, FPCR,
and FPSR. New contexts start at zero. There is no independent CPU-local owner
pointer: the existing generation-qualified Native or guest run publication
identifies the only lower world permitted to acquire the registers.

Under VHE, `CPTR_EL2.FPEN` records residency for the interrupted lower world.
Exception entry saves CPTR in its integer frame and disables FP before calling
Rust. A first-use trap validates the published owner, restores its complete
state, enables access in the return frame, and retries the instruction without
advancing the PC. Guest CPACR_EL1 remains guest-owned; HypeR's EL2 control does
not override the guest operating system's own FP access policy. Optional SVE
and SME state remains unsupported and trapped.

A real Native unwind saves used state before closing run publication. Guest
wait, device, terminal, and administrative-stop exits do the same. Guest IRQ
postludes are also ownership boundaries: they can deactivate, schedule, migrate,
or terminate the vCPU without unwinding the original exception frame. Before
invoking such a callback, entry saves the state and marks the suspended frame
as requiring another first-use restore. This remains necessary when the
postlude happens to reconcile interrupts without changing CPUs.

Save and release clears the physical vector registers and FP controls before
returning. Startup clears firmware state on every CPU. Consequently no prior
owner's register contents remain on a CPU after an ownership boundary, even
while a later owner has not yet used FP. All state leaves execute with local
exceptions masked and keep FP disabled on return. No destructor, remote flush,
or reference surviving migration is required.

## Scope and validation

This is run-scoped lazy restore, not cross-run CPU ownership caching. An FP-using
thread may trap again after a blocking call or scheduling boundary. Retaining
state across that boundary would require another lifetime and migration
protocol and is deliberately outside this design.

Removing unconditional register copies reduces entry/exit work for integer-only
runs. This is not a blanket performance guarantee: an FP-heavy workload pays a
first-use trap after each ownership boundary, and the soft-float kernel also
changes the generated code for operations such as memory copies. Workload
measurements are separate from the isolation tests.

RISC-V and x86-64 retain their existing context handling. This does not enable
x86 lazy restore or extend the guest CPU model with additional register banks.

The QEMU self-tests exercise integer-only Native execution, initialized FP
state, preservation across direct and deferred syscalls, competing owners,
preemption, and guest affinity migration. The normal Linux guest acceptance
also exercises Native/guest interleaving. These establish architectural behavior
in the emulator; physical AArch64 validation remains necessary for implementation
specific trap and speculative-execution behavior.
