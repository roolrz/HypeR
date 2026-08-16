# CI test policy

The scripts in this directory are the stable boundary between GitHub Actions
and the project build/test system. They are also directly runnable locally.

| Suite | Required contract |
| --- | --- |
| `quality` | Formatting plus host, Kconfig, and kallsyms tests |
| `scripts` | ShellCheck for test and guest-acquisition scripts |
| `aarch64-build` | Clippy, canonical build, stripped-image identity, image ABI/instruction checks, and a separate kernel-self-test image |
| `aarch64-qemu` | Linux userspace and console RX plus the complete AArch64 feature markers described below |
| `riscv64-qemu` | The RISC-V Linux guest boots and hands control to `/init` |
| `x86_64-build` | Clippy and successful canonical/stripped image compilation; no runtime requirement yet |

The AArch64 QEMU matrix covers one and four CPUs on both the Armv8.0
`cortex-a72` model and the feature-rich `max` model, plus a constrained-memory
case. Together these require both nVHE/VHE host modes and LL/SC/LSE atomics.
Every case verifies kernel self-tests, guarded thread/IRQ/emergency stacks,
scheduler and sleeping synchronization, SMP admission, GICv3/vGIC, host and
guest timers, virtual system registers, PL011 RX, KASLR geometry, allocator
ownership statistics, lazy guest demand paging, and Linux userspace.

Failed QEMU jobs retain their complete serial logs as CI artifacts. Guest
Linux inputs are checksum-pinned by `tools/guest` and cached only as CI inputs;
they are not included in the distributable kernel artifact.
