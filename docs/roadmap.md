<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Roadmap

[Project overview](../README.md)

The roadmap describes direction rather than release promises. Adjacent work may
overlap, but a later stage must not bypass an ownership or isolation prerequisite
from an earlier one. Every stage keeps AArch64 healthy and preserves buildable
secondary architectures.

## 1. Establish native EL0 and the capability ABI

- extend the current loader-backed AArch64 VHE split `TTBR0_EL2`/`TTBR1_EL2`
  and nVHE stage-2-only Native path into a service runtime before publishing a
  binary ABI, retaining a minimal EL1 relay only as a justified compatibility
  fallback;
- extend the implemented Process, UserThread, ProcessImage, TaskGroup,
  ResourceDomain, and address-space lifecycle with multi-Thread race coverage
  and atomic exec quiescence;
- expand the checked schema's current Rust values, C header, layouts, metadata,
  and reference into generated dispatch wrappers, architecture stubs, and vDSO
  exports;
- expose the typed VMO/VMAR capability core through Native syscalls, then
  extend the implemented Event, Channel, and single-object wait foundation with
  EventPair, WaitSet, clock/timer, and atomic-wait primitives sufficient for a
  real service runtime;
- add temporary pre-release debug output, then extend the implemented blocking
  cancellation with multi-Thread Process qualification before atomic exec.

## 2. Extend VM lifetime and topology

- install and start multi-vCPU groups transactionally without parallel global
  registries;
- add explicit pause, resume, shutdown, interrupt injection, and richer
  resource-accounting lifecycles on the current lease-backed teardown path.

## 3. Evolve native VMM policy

- replace the initial single-guest manager policy with a validated fleet
  manifest, restart policy, and per-VM health reporting;
- add explicit guest-memory grants and virtual-device sessions without sharing
  whole-VM authority with backend services;
- integrate RISC-V Linux Image/FDT loading and product VMM configuration on the
  Native reference platform;
- extend the userspace-owned lifecycle to x86-64 after its stop and stage-2
  retirement mechanisms meet the AArch64 contract.

## 4. Build device-isolation resources

- extend the existing owned MMIO/IRQ model to DMA, IOMMU, firmware, and
  physical-memory authorities;
- implement revocable MemoryGrant, DeviceLease, DmaMapping, and
  InterruptSession lifecycles with fail-closed teardown;
- resolve generic FDT phandles, `interrupt-map`, and `interrupts-extended`
  without moving binding policy into the parser.

## 5. Add the transitional Linux driver domain

- run Linux as an untrusted driver-domain VM, never as HypeR's host OS;
- introduce a bounded copy-based backend transport before shared zero-copy
  queues, then add virtio block and network frontends;
- require stage-2 and IOMMU confinement before assigning physical devices or
  permitting DMA into granted memory.

## 6. Mature kernel execution

- qualify IRQ-tail and vCPU preemption on secondary architectures, then add
  controlled migration and load balancing over existing affinity metadata;
- strengthen power-management, suspend/resume, and CPU hotplug lifecycles;
- expand diagnostics, tracing, crash analysis, and management interfaces;
- validate ordering, cache maintenance, and interrupt behavior on physical
  AArch64 hardware.

## 7. Add foreign binary personalities

- add a supervised execution route which is immutable for each installed
  ProcessImage and prove it first with a small alternate test ABI;
- implement Linux and FreeBSD compatibility supervisors with private fd,
  signal, credential, VFS, restart, auxiliary-vector, and vDSO policy;
- keep the route extensible for a future whole-personality in-kernel engine,
  while rejecting per-syscall mixing of kernel and supervisor semantics.
