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
runtime creates its own VirtualSerial and registers a caller-allocated output
VMO before transferring device-assignment authority into the VM. The retained control endpoint
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
runtime process, and console connector. Extending the fleet changes definition and
instance storage rather than the per-instance construction contract.

For each provisioned VM, the manager creates a child resource domain and task
group, derives a one-shot VM creation lease, and starts an isolated
`/svc/vm-runtime` process. The guest image is duplicated into that runtime.
The runtime receives only its guest image, creation lease, read-and-execute
runtime libraries, its own process resources, and a CapabilityChannel for receiving
authorized console clients.

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
The runtime creates a VirtualSerial, allocates a 68 KiB VMO, maps it read-only,
and registers those whole pages before assigning the port into the pending VM.
Registration acquires an exclusive write lease: existing writable aliases reject
registration; direct VMO access, snapshots, and new writable aliases remain
excluded until final retirement. The kernel pins the backing but allocates no
private output queue. One header page contains production and loss counters;
the remaining 64 KiB holds atomic byte slots. Runtime reads acquire-observe the
producer and acknowledge absolute consumption through a READ-authorized syscall.
Invalid backwards or future acknowledgements change neither cursor nor signals.

Output publication, acknowledgement, and closure serialize under one port lock.
The first byte of an unread batch asserts READABLE; subsequent bytes do not
notify. Acknowledgement clears it only if all published bytes were consumed.
Thus output arriving before clearing stays readable and output arriving after
clearing reasserts readiness. The existing signal wait protocol closes the
check-to-park race. Full rings drop new bytes and count loss without blocking
the guest or overwriting unconsumed bytes. See the generated ABI for layout.

`vm-runtime` uses a persistent WaitSet for output, control, vCPU termination,
and actionable client channel states, with an infinite deadline. It owns a
bounded 64 KiB retention queue and nonblocking client forwarding. Guest input
uses bounded partial injection and WRITABLE notification when its queue regains
space. The runtime subscribes to client READABLE only when it can accept another
message, and to client WRITABLE only while output is pending. Peer closure drains
already accepted input. Neither direction requires periodic polling.

The lock order is port, signal state, then scheduler/WaitSet notification.
Guest-device kicks run after releasing the port lock. Output byte publication
uses Release and readers use Acquire; consumption completes before the syscall
releases slots under the port lock. Atomic byte access also tolerates a hostile
caller prematurely acknowledging slots. Physical AArch64 qualification must
stress publication/acknowledgement across CPUs, idle wakeups, full queues, and
runtime termination during output; QEMU alone does not prove weak ordering.

The shell holds only a `WAIT|WRITE` manager-connector endpoint. Each `/bin/vmm`
invocation obtains private control and capability channels. For `vmm console`,
the manager authorizes one client, creates a ByteChannel pair, sends one end to
the runtime's connector, and transfers the other end to the client. Neither
console bytes nor VirtualSerial handles are relayed through the manager to the
client. Closing the private control channel releases attachment policy; the
runtime observes the data peer closing. Ctrl-] detaches without stopping the VM.

VirtualSerial handles stay in their creating runtime: generic capability
transfer and ProcessBuilder storage are forbidden. The same-process device
assignment consumes only the binding handle. Runtime loss therefore closes all
userspace port handles and synchronously closes serial output admission under
the port lock, after any admitted writer completes. VM stop and acknowledged
retirement quiesce vCPU execution before those owners can release registered
pages. User unmapping, handle close, and runtime address-space teardown therefore
cannot turn a cached kernel pointer into a write to freed/reused memory. Pinned
page charges follow the registration lifetime, independently of user mappings.

The manager validates the runtime's monotonic lifecycle records and publishes
one terminal event on the per-instance endpoint. Loss of either control peer or
malformed protocol data triggers an idempotent forced-stop transition; it does
not terminate the fleet manager. A client stop first requests cooperative
shutdown. The state machine also defines an explicit grace-deadline escalation
transition. The manager computes a finite absolute deadline from the Native
monotonic clock, passes it directly to the same multi-object wait, and forces
the runtime Process when the deadline expires. It does not approximate time
with polling, scheduler yields, or unrelated inspection authority.

## Serial validation

`make test-native` covers console attachment, output, and detachment.
`make test-runtime-crash` builds the explicit `test-runtime-crash` fixture and
boots a separate archive; the production runtime keeps its default features.
The test first stops the boot-critical initial VM cleanly. The fixture then
exits without destructors during console forwarding on later instances. Five fresh
VM/runtime cycles in a 512 MiB machine check teardown and continued allocation.
Kernel self-tests verify invalid registration, hostile cursor values, full-ring
behavior, exclusive assignment, last-handle output closure, and release of
pinned and committed pages only after the final device owner drops. Host tests
exercise cursor wrap and exhaustion.

Physical AArch64 qualification still needs concurrent guest output/runtime
consumption under migration, forced runtime termination, and repeated VM
restart on VHE and nVHE hardware. Check payload ordering and final memory
accounting under load; QEMU does not establish weak-memory or cache behavior.

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
