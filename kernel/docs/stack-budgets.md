<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Kernel stack budgets

Ordinary scheduler threads, including Native runners and vCPUs, have 16 KiB
guarded kernel stacks. IRQ and emergency stacks are separate 32 KiB mappings.
Early boot retains 256 KiB for allocation-free discovery and scheduler setup;
that capacity is not available to a normal syscall. Do not increase a runtime
stack or reduce an ABI batch merely to hide a large temporary.

## Reproducing the checks

Run `make test-stack ARCH=aarch64`. This builds a dedicated initramfs in
`target/stack-audit/aarch64`, substituting the std workload for its `/bin/ps`
so the existing shell policy supplies thread inspection authority. The normal
application output and production initramfs are preserved. The workload checks
thread scan pagination, snapshot isolation, boundary/eager/COW mappings, and
dynamic child startup/exit, then stops the Linux guest to observe vCPU retirement.
The console attachment also exercises interactive guest input.

The target builds with `kernel-stack-audit` and `STACK_METADATA=1`. The latter
scopes `RUSTC_BOOTSTRAP=1` and `-Zemit-stack-sizes` to diagnostic kernel builds
using the pinned compiler. `.stack_sizes` is non-allocated ELF metadata: it is
not included in the loaded kernel image. Ordinary builds need neither option.
The static checker rejects missing metadata instead of silently accepting an
uninstrumented image. Its JSON report includes every reported symbol, its frame
size, its ceiling, and the reason for each reviewed exception.

The AArch64 policy uses a 4096-byte local-frame ceiling with tighter limits for
selected runtime paths. Boot, emergency, initial process construction and
full-batch serial exceptions are named individually. Initial process construction
still has a large frame on a normal 16 KiB `native-init` worker stack; being
one-shot does not make it safe automatically. Its nested calls and measured
reserve remain relevant. These are regression limits, not recommended frame sizes.
Changing the compiler or inlining layout requires reviewing the resulting
frames and their callers, not simply raising limits until a build passes.

## What the evidence does and does not prove

Compiler metadata describes individual fixed frames. It does not add the frames
of nested calls, model indirect calls or recursion, or account for assembly
entry and interrupt frames. Several individually acceptable frames can still
overflow a 16 KiB thread stack. Review complete hot/error/cleanup call chains.

The optional audit samples only detached, terminated threads, after the switch
tail releases CPU ownership and before their stack is reclaimed. It checks
canaries and modified-byte watermarks. IRQ samples are restricted to the local
CPU, with migration and interrupts excluded during the scan. It never scans
another CPU's live stack. Logging reports the first sample and new maxima;
`samples` is the observation ordinal at that maximum, not a final sample count.

Watermarks can miss reserved but untouched stack space. Still-running services,
the reaper's own stack and IRQ stacks on unsampled CPUs are not covered. QEMU
workload coverage is evidence, not a proof against every stack overflow.
The harness requires retired kernel/user/vCPU and local IRQ observations, intact canaries
and at least 2048 bytes of measured reserve. `STACK_MINIMUM_REMAINING` can set a
stricter local threshold. Guard pages catch out-of-range access, but exception
entry itself still requires space; recovery from arbitrary exhaustion is not
guaranteed.

## Performance comparisons

Use ordinary production images with `verify-stack.py --no-audit --repetitions 9`
and the same dedicated initramfs for both revisions. Audit scans and logging add
intentional overhead and must not be used to establish production performance.
The harness waits for the guest's idle prompt before measuring. Compare repeated
stage timings, accounting for QEMU and host scheduling noise; CI deliberately
does not enforce a wall-clock timing threshold.

The runtime optimizations preserve eight-record inspector pages and serial
transfer limits. Caller-owned inspector output removes large aggregate return
copies. Snapshot and private-map initialization copy between pinned page
mappings directly, eliminating page-sized bounce buffers and a second copy.
Interactive serial writes use a 128-byte specialization; full 4096-byte writes
retain their complete buffer, one usercopy and one publication, preserving
failure and partial-write behavior without allocating per input event.

`make test-stack ARCH=riscv64` runs the same workload and watermark checks.
The static frame policy currently covers AArch64 only; a policy must be measured
and reviewed independently before enabling it on another architecture.
