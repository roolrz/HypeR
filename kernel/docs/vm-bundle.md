<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# VM image and boot ownership

HypeR keeps firmware boot, system userspace, and guest boot as separate trust
and ownership layers. U-Boot selects and authenticates the HypeR system image,
loads the HypeR kernel and its system initramfs, and describes both through the
standard platform DTB. It does not select, relocate, or modify a guest VM.

The system initramfs is an uncompressed SVR4 `newc` or `crc` CPIO archive. It
contains `/init`, system services, SDK runtime libraries, configuration, and
zero or more guest FIT images under `/vm`. Early boot validates and reserves
the complete firmware-provided archive before making it the immutable ramfs
backing for the Native root filesystem.

## Userspace ownership

The initial service graph identifies exactly one VM manager by its validated
provisioning purpose, then starts it inside an init-created fleet resource
domain and task group. That bounded ancestor accounts the manager and every
domain it creates, so delegated domain-creation authority cannot charge init's
shared service domain without limit. Init retains the peer CapabilityChannel
endpoint, opens the selected guest image, creates a per-instance ByteChannel
control pair, and transfers the image plus the manager-side control endpoint in
one typed rendezvous. Each transport right survives until its final ownership
hop: the manager attenuates it from the image and control endpoint, while the
runtime retains device-assignment authority on a newly created VirtualSerial
until that capability is consumed into the VM. The retained control endpoint
is the instance's authority-bearing identity; lifecycle requests do not use
ambient numeric VM identifiers. Neither the manager nor a VM runtime receives
physical Console authority.

The initial aggregate policy admits one VM and one vCPU, 32,768 guest pages,
128 MiB of kernel allocation, 12 processes, 24 threads, 512 handles, and 2,048
kernel objects. Every other accounting dimension is likewise the sum of the
one-instance limit and explicit control-plane headroom. This is an admission
boundary rather than a usage target; changing fleet cardinality requires a
reviewed policy update, not merely a larger collection in the manager.

The initial manager deliberately supports one named definition and one active
instance. It retains a read-only duplicate of the image so a stopped instance
can be started again with a fresh resource domain, task group, creation lease,
runtime process, and VirtualSerial. Extending the fleet changes definition and
instance storage rather than the per-instance construction contract.

For each provisioned VM, the manager creates a child resource domain and task
group, derives a one-shot VM creation lease, and starts an isolated
`/svc/vm-runtime` process. The guest image is duplicated into that runtime.
The runtime receives only its guest image, creation lease, read-and-execute
runtime libraries, its own process resources, and one assign-only VirtualSerial
capability.

Each runtime:

1. validates and parses its guest FIT through bounded random-access reads;
2. creates and retains the guest-memory VMO;
3. streams the selected kernel and initramfs payloads into that VMO;
4. constructs guest firmware data, including the Linux DTB;
5. seals a pending VM and atomically publishes the installed VM and boot-vCPU
   handles; and
6. retains supervision handles until acknowledged VM retirement.

The kernel owns stage-2 translation, interrupt virtualization, vCPU execution,
and teardown. It retains an independent reference to the guest-memory backing
through stage-2 invalidation, so process or handle teardown cannot free pages
still reachable by hardware. Attaching the VMO acquires an exclusive hardware
write lease: it fails if a Native writable mapping or direct access is active,
and later direct access, snapshots, or writable mappings stay closed until
acknowledged VM retirement. Read-only Native mappings may coexist.

Stage-2 demand pages are initially read/write and execute-never. A final guest
instruction fault publishes that page's instruction bytes and then promotes
only its stage-2 execute permission using the selected architecture's required
translation invalidation. Faults raised while walking guest page tables remain
data-only. Sealing conservatively publishes and promotes every loader-resident
page because executable-range metadata is not yet part of the VM ABI; sparse
pages retain the demand-promotion path.

A userspace-created VM has no implicit route to the physical host Console.
Before sealing, its VMM may explicitly transfer an assign-capable
VirtualSerial handle into the PendingVirtualMachine. The kernel retains bounded
guest output independently of client attachment and injects host input through
the virtual UART. VM retirement disconnects the port without discarding output
that a client has not yet read. The stream is best-effort: guest execution never
blocks on a full output buffer, so output beyond the bounded retention window
may be discarded. A userspace read claims one prefix transactionally, and a
failed copy does not consume that prefix.

The shell holds only a `WAIT|WRITE` manager-connector endpoint. Every invocation
of `/bin/vmm` creates private byte and capability channels and transfers their
manager endpoints through a short rendezvous, so one slow client cannot own the
shared listener. Lifecycle commands are short control-plane exchanges and may
run concurrently from different physical sessions. `vmm console` additionally
requests an attenuated VirtualSerial data-plane handle. The manager admits only
one console client per VM; disconnecting the private control channel releases
that attachment while leaving other management clients unaffected. The local
Ctrl-] menu detaches without changing VM power state.

The manager validates the runtime's monotonic lifecycle records and publishes
one terminal event on the per-instance endpoint. Loss of either control peer or
malformed protocol data triggers an idempotent forced-stop transition; it does
not terminate the fleet manager. A client stop first requests cooperative
shutdown. The state machine also defines an explicit grace-deadline escalation
transition. The manager computes a finite absolute deadline from the Native
monotonic clock, passes it directly to the same multi-object wait, and forces
the runtime Process when the deadline expires. It does not approximate time
with polling, scheduler yields, or unrelated inspection authority.

## Guest FIT contract

Guest images use the standard flattened image tree container with embedded
payload data. The current packer emits this shape:

```text
/
  images/
    kernel@1/
      data
      type = "kernel"
      arch = "arm64"
      os = "linux"
      compression = "none"
      load
      entry
    ramdisk@1/
      data
      type = "ramdisk"
      arch = "arm64"
      os = "linux"
      compression = "gzip"
      load
  configurations/
    default = "conf@1"
    conf@1/
      compatible = "hyper,guest-image-v1"
      kernel = "kernel@1"
      ramdisk = "ramdisk@1"
      bootargs
      hyper,memory-size
      hyper,vcpu-count
      hyper,platform-profile = "aarch64-reference"
```

`load`, `entry`, and `hyper,memory-size` must each be encoded as exactly one
64-bit big-endian value.
`hyper,vcpu-count` is one 32-bit cell. The selected configuration must identify
the `hyper,guest-image-v1` storage contract and one supported immutable virtual
platform profile. References and strings are exact, NUL-terminated UTF-8
values. Selected configuration and image records are property-only schema
leaves; child nodes are rejected. Duplicate selected nodes or properties,
embedded NUL bytes, overlapping FDT blocks, invalid ranges, unsupported image
types, and architecture mismatches are rejected before VM construction.

The current AArch64 runtime supports one vCPU, power-of-two page-aligned RAM of
at least 64 MiB, an uncompressed Linux kernel, and an optional Linux-supported
compressed initramfs. It validates the raw Linux `Image` magic, entry and
`text_offset` placement, and nonzero `image_size`; placement reserves that
complete occupied extent rather than only the bytes embedded in the FIT. Both
the 2 MiB-aligned placement base and the occupied image extent must lie in
guest RAM. The loader rejects overlap between that extent, the initramfs, and
the generated DTB, and verifies that every selected payload range lies within
the FIT source.
FIT validation also requires an aligned, zero-terminated memory reservation
map which does not overlap the structure or string blocks. Parsing has fixed
budgets for reservation records, structure tokens, and nesting depth; embedded
payload properties are skipped in constant parser work and copied only after
the complete metadata contract succeeds. These are runtime implementation
limits, not a storage-format promise for other architectures or future VMM
implementations.

`hyper,vcpu-count` fixes the machine's immutable processor topology before
construction. `pending_virtual_machine_set_bootstrap` configures only boot vCPU
0, and installation returns the machine and that boot-vCPU handle; it does not
represent the complete topology as a variable-length syscall result. Secondary
processors begin in their architecture-defined powered-off state. AArch64 PSCI
`CPU_ON` or x86 INIT-SIPI supplies their runtime entry state. Future secondary
vCPU control handles will therefore be exposed by an additive
`virtual_machine_open_vcpu(machine, id)` operation over the predeclared topology,
without changing the existing construction calls. Creating or publishing a
secondary vCPU must use a generation-qualified installed-machine lease and bind
its endpoint while holding the `InstalledMachine` runtime lock. The lifecycle
state check and endpoint publication are one transaction: publication is
permitted only while the machine is `Installed` or `Running`, and must fail once
it is `Stopping` or `Stopped`. This prevents retirement from observing an
unbound endpoint and then racing a late secondary-vCPU publication. The current
implementation rejects a `vcpu_count` other than one until those architecture
startup paths and concurrent vCPU execution are implemented.

`tools/fit-pack` creates deterministic development and CI images from an
external kernel and initramfs. Generated guest artifacts remain ignored by Git.

## Integrity and licensing

FDT structural validation is not authenticity. Production composition must
authenticate the HypeR kernel, platform DTB, system initramfs, and guest images
according to deployment policy before granting VM creation authority. FIT hash
or signature nodes can be added without moving image selection into U-Boot or
the kernel.

Guest kernels and userspace retain their upstream licenses. Downloaded guest
payloads are test artifacts and are not part of HypeR's Apache-2.0 source.
