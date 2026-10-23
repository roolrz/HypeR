<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Detailed design

These are the contracts used to change an implementation safely. Start with
[architecture](../architecture/README.md) for ownership, or use
[application manuals](../usage/applications/README.md) for commands.

| Area | Contracts |
| --- | --- |
| Boot | [Stage-1 handoff](boot/bootstrap-handoff.md), [kernel Native init](../../kernel/docs/design/native-init.md), [VM images](../../kernel/docs/design/vm-bundle.md) |
| Services | [Component supervision and routes](services/components.md), [terminal service](services/terminal.md) |
| Storage | [Filesystem workers and RPC](storage/filesystem-services.md), [asynchronous block I/O](storage/block-io.md), [FAT semantics](storage/fat.md), [kernel VFS/cache](../../kernel/docs/design/vfs.md) |
| Virtualization | [I/O VM](virtualization/io-vm.md), [virtio serial](virtualization/virtio-console.md), [device/DMA authority](../../kernel/docs/design/io-isolation.md), [SMMUv3](../../kernel/docs/design/smmuv3.md) |
| Native ABI | [Syscall architecture](../../kernel/docs/design/syscall-abi.md), [object interfaces](../../kernel/docs/design/syscall-families.md), [generated ABI reference](../../sdk/abi/docs/native.md), [physical devices](../../sdk/abi/docs/physical-devices.md) |
| Kernel execution | [Timers/callouts](../../kernel/docs/design/timers.md), [clocks](../../kernel/docs/design/time.md), [floating-point ownership](../../kernel/docs/design/floating-point.md), [random numbers](../../kernel/docs/design/random.md) |

Source-owner references document the [SDK](../../sdk/README.md),
[Native substrate](../../stdlib/README.md), [bootstrap loader](../../bootstrap/stage1/userspace-loader/README.md)
and [formal dynamic loader](../../lib/dynamic-loader/README.md).
