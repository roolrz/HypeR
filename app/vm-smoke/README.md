<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Native VM lifecycle smoke test

`make test-vm-smoke ARCH=riscv64` boots this privileged fixture as `/init` in a
separate initramfs. It is not part of the interactive system's app bundle.
The fixture needs the normal initial task, filesystem, console, resource-domain,
and VM-creation bootstrap capabilities. Its child executes the same `/init`
with a narrowed creation lease and rendezvous endpoint.

The position-independent guest in `src/guest.S` is assembled into the fixture
and copied into a fresh RAM VMO. Cases cover bootstrap register arguments,
administrative stop of a CPU-bound guest, SBI and direct Sstc timers, WFI with
global interrupts disabled, FP/GPR retention, software-interrupt clearing,
FIFO receive timeout and PLIC completion, fatal guest memory access, VU trap
delegation, process-owner cancellation, and repeated VM retirement.

Every blocking observation has a finite deadline. `VM-SMOKE: PASS` is emitted
only after all cases finish; failures include `VM-SMOKE: FAIL`. The owner-loss
case deliberately keeps the VM handle in the cancelled child and transfers
only a vCPU observer to its parent, so observation cannot keep the VM alive.
