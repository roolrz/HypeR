<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# HypeR

[![CI](https://github.com/roolrz/HypeR/actions/workflows/ci.yml/badge.svg)](https://github.com/roolrz/HypeR/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

**An experimental virtualization host built around a Rust kernel and a native
capability-based userspace.**

HypeR owns the scheduler, memory management, hardware virtualization, and host
service runtime. Native userspace services manage Linux guests through explicit
capabilities. AArch64 is the primary platform and requires FEAT_VHE (Virtualization
Host Extensions).

## Why run HypeR?

Run HypeR if you want to explore how a virtualization host is built, from CPU
entry to VM management, and change those layers together:

- **Control the whole host.** Work on the kernel, Native ABI, SDK, and VM
  services in one source tree, with ownership and lifecycle contracts across
  their boundaries.
- **Experiment with capability-based VM management.** Native services receive
  explicit handles and rights; a userspace VM manager delegates work to
  separate per-VM runtime processes. You can study and extend how authority is
  granted, narrowed, and retired.
- **Develop an AArch64-first Rust system.** Exercise the kernel and Native
  service stack in QEMU, with repeatable acceptance tests for SMP and Linux
  guest execution.

Today, the reason to choose HypeR is to develop this architecture. It is not
ready to host production or untrusted workloads, and the project makes no
claim of better performance or stronger security than established alternatives.

## How does it compare?

These projects occupy different parts of the virtualization stack. Choose
according to the layer you want to operate or develop:

| Project | Where it fits |
| --- | --- |
| [KVM](https://docs.kernel.org/virt/kvm/api.html) | Linux kernel virtualization interfaces used by userspace VMMs; a fit when Linux is your host foundation. |
| [Xen](https://handbook.xenproject.org/users/introduction.html) | An established bare-metal hypervisor with a domain-based architecture; a fit when you want to build on that ecosystem. |
| [bhyve](https://docs.freebsd.org/en/books/handbook/virtualization/#virtualization-host-bhyve) | FreeBSD's hypervisor; a fit when FreeBSD is your host environment. |
| [Cloud Hypervisor](https://www.cloudhypervisor.org/) | A Rust VMM focused on modern cloud workloads. |
| [crosvm](https://crosvm.dev/book/) | A hosted Rust VMM focused on sandboxed guest execution. |

HypeR's focus is developing its own Rust kernel and Native capability runtime
together. Choose it when that host architecture is the thing you want to
experiment with; choose an established stack when running VMs is the goal.

## What works today?

The AArch64 QEMU system boots Native init, a shell, and VM management services
that start a Linux guest and expose its console. RISC-V runs Native init,
shell, std applications, and userspace-managed Linux guests on QEMU. x86-64 currently has build and image validation only.

The Native ABI is pre-release. Broad hardware support, general-purpose virtual
I/O, device assignment, and multi-vCPU userspace management remain unfinished.
See [implementation status](docs/status.md) and the [roadmap](docs/roadmap.md).

## Near-term roadmap

**Run HypeR on Raspberry Pi 5 with a trimmed, trusted Linux I/O VM as its
network and block-device backend.** Linux drives the physical devices; HypeR
Native services use an explicit shared-memory I/O protocol.

- [x] Implement host GICv2 and Pi 5 debug-UART support; validate in QEMU.
- [ ] Boot the Native shell on Pi 5 and validate SMP, timer and console behavior.
- [ ] Add GICv2 guest interrupt support and boot the Linux I/O VM on Pi 5.
- [ ] Assign network/storage devices and implement shared queues, notifications
  and buffer ownership.
- [ ] Exercise Native network and storage I/O on Pi 5, including failure handling
  and performance measurements.

See the [roadmap](docs/roadmap.md) for scope and acceptance criteria, and the
[Pi 5 boot guide](kernel/docs/rpi5.md) for the recommended firmware setup.

## Try it

With the [prerequisites](docs/getting-started.md#prerequisites) installed, run
from the repository root:

```sh
make defconfig
make run
```

This builds and boots the AArch64 system in QEMU, downloading checksum-pinned
Linux guest assets on the first run. See [getting started](docs/getting-started.md)
for configuration, other architectures, and build targets.

## Explore and contribute

- [Development guide](docs/development.md): editor setup, testing, source layout,
  and contribution requirements.
- [Architecture](kernel/docs/architecture.md): kernel boundaries and ownership.
- [Shell and text filtering](docs/shell.md): pipelines, file redirection, and grep.
- [Native userspace](kernel/docs/native-init.md) and [SDK](sdk/README.md): host
  services and application interfaces.
- [Issues](https://github.com/roolrz/HypeR/issues): bugs, experiments, and design
  discussions. Architecture work, tests, and rigorous reviews are welcome.

[Apache-2.0](LICENSE). External guest images have [separate licensing obligations](docs/development.md#guest-artifacts-and-licensing).
For vulnerability reporting, see [SECURITY.md](SECURITY.md).
