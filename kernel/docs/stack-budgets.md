<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Kernel stack budgets

Ordinary scheduler threads, including Native runners and vCPUs, have 32 KiB
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
The console attachment also exercises interactive guest input. For diagnosis,
`verify-stack.py --stage inspect|memory|process` isolates a workload stage so
its terminated thread can be measured independently.

The target builds with `kernel-stack-audit`. Acceptance uses observed stack
watermarks, canaries, and reserve limits for complete workloads. Individual
compiler-generated function frames are not CI budgets: inlining and compiler
versions change their layout without necessarily changing end-to-end usage.
No unstable compiler flags or `.stack_sizes` metadata are required.

## What the evidence does and does not prove

Review complete hot/error/cleanup call chains; workload coverage does not
exercise every possible combination of calls or interrupt timing.

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
and at least 2048 bytes of measured reserve. It also rejects a measured maximum
above 24 KiB, independently of the allocated stack size. This leaves 8 KiB
between the watermark ceiling and the ordinary stack capacity (including the
canary). `STACK_MAXIMUM_USED` can override the workload ceiling.
`STACK_MINIMUM_REMAINING` can set a stricter reserve threshold. Guard pages catch out-of-range access, but exception
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

FAT directory parsing fills a caller-owned entry and borrows its long-name
buffer. A mounted volume owns one persistent sector-cache allocation in place
of its former outer volume allocation, reducing mount return-value copies
without adding allocations or I/O. ELF segments retain their validated ELF
fields and derive page extents, reducing sorting scratch without replacing the
sorting algorithm. Process rollback extracts the address-space owner from its
existing unique allocation instead of moving the complete process state onto
the stack. Bootstrap and child startup release encoding scratch before later
thread and capability publication phases.

`make test-stack ARCH=riscv64` runs the same workload and watermark checks.
Both architectures use the same runtime limits rather than per-function ceilings.
