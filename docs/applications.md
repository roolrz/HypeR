<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Native applications

Applications use the installed SDK, Rust std, and clap. Run `APP --help` for
options (`echo` uses `-n`, `-e` and `-E`; its unknown options are literal text).

## Files and system information

| Command | Examples and behavior |
| --- | --- |
| `cat` | `cat FILE...`, `cat -n FILE`, `cat -bs FILE`, `cat -`; streams bytes without UTF-8 conversion. Number all lines with `-n`, only nonblank lines with `-b`, and squeeze repeated blank lines with `-s`. Numbering spans inputs. |
| `grep` | `grep -in PATTERN FILE`, `grep -F TEXT`, `grep -v PATTERN`; filters files or stdin, with counting, filename and quiet modes; `-m NUM` limits selected lines per input and `-x` matches whole lines. |
| `ls` | `ls /etc/hyper`, `ls FILE DIRECTORY`, `ls -a`, `ls --sort size -r`; defaults to permission mode, IEC size, and sorted names. `--bytes` gives exact file sizes; `-1` prints names only, `-d` lists a directory itself, `-t` sorts newest first, and `-S` sorts largest first. Directory sizes are shown as `-`. |
| `ps` | `ps -T`, `ps -p KOID`, `ps --name vm-runtime`; select decimal/hex KOIDs (repeat `-p` or use commas) and intersect with a name substring, optionally including threads. IDs print in hexadecimal; `--no-headers` prints rows only. Thread rows include `process=NAME`; kernel-owned threads use `OWNER=kernel`, and per-CPU idle threads are named `idle/CPU`. |
| `free` | `free`, `free --bytes`, `free -m -c 5 -s 0.5`; physical totals, ownership and reclaimable cache pages. `-b/-k/-m/-g` select units; `-c` is a finite sample count (default 1), `-s` the interval. |
| `top` | `top -d 0.5`, `top -b -n 3`; interactive refresh with q/Ctrl-C to quit, or plain finite batch snapshots. Rows sort by CPU usage; `--sort name/koid`, `-p KOID`, `--name TEXT` and `--limit NUM` select the view. |
| `handle` | `handle shell`, `handle --kind physical-device`, `handle --all --object KOID`; inspect objects, process capabilities, and the processes holding an object. |
| `echo` | `echo hello world`, `echo -n text`, `echo -e 'one\ntwo'`; `-n` suppresses the newline, `-e` enables escapes and `-E` restores literal backslashes. |

`cat` and `ls` report an error per failed path, continue with remaining paths,
and return failure if any path failed. The interactive shell merges unredirected
stdout and stderr into one terminal queue before displaying its next prompt.
`echo` is an ordinary application, not a second implementation inside the shell.

The shell supports concurrent pipelines and file redirects, for example
`cat /etc/hyper/vms.json | grep image > /matches`. See
[shell and text filtering](shell.md) for syntax, limits and grep options.

Native std provides `std::os::hyper::fs::MetadataExt::mode()` for permission and
special mode bits. File type is queried separately. `metadata` follows symbolic
links; `symlink_metadata` inspects the final link itself.
`std::os::hyper::fs` supplies Native symlink and permission extensions.

## Object and capability inspection

`handle` defaults to the visible object registry. Select a process by its exact
name or full KOID to list its handles. `-p PROCESS` is equivalent to the positional
argument; `--objects` explicitly selects the default registry view. Names beginning
with a digit are parsed as IDs. An ambiguous name reports the matching KOIDs so
you can select the intended process.

```sh
handle
handle shell
handle -p 0x0000000100000012
handle io-runtime --kind physical-device
handle --object 0x0000000100000034 -v
handle --all --object 0x0000000100000034
handle shell --handle 0x0000000001000001 -v
handle --all --right write --right map-dma
handle --kind guest-memory --kind guest-mapping
handle --all --no-headers | grep physical-device
handle --list-kinds
handle --list-rights
```

The IDs above are examples; use those reported by your running system. Handle,
process and object IDs print in full-width hexadecimal; input accepts decimal
or `0x` hexadecimal; `ps` also prints full-width hexadecimal KOIDs. A handle number is local to its
process; use the OBJECT KOID to correlate the same object across processes.
Both forms include their generation. Neither an observed ID nor this command
grants access to the observed object.

`--kind` accepts a catalog name or numeric kind ID; repeated kinds match any of
them. Unknown names are errors. `--right` filters **granted** handle rights and
requires a process or `--all`; repeated rights must all be present. No matching
rows is a successful query with an explicit empty-result message. A specifically
selected process, object (in the registry view), or handle that is unavailable
returns failure. `--no-headers` omits banners, column headers and empty-result
messages and type-specific details for pipelines. `-v` instead adds human-readable detail and cannot be
combined with `--no-headers`.

The object table separates handle state/count from total strong references:
`unpublished` means no handle has yet been published, `active` means live handle
references exist, and `retired` means the last handle reference was released.
A retired object can remain alive through kernel references. `-v` shows the
object's supported rights and reference counts by class (kernel service, VM
device binding, scheduler, operation, user authority, publication, diagnostic,
and retirement). Supported rights are a ceiling, **not** the rights granted to
each holder; inspect process handles for that. Handle detail also includes the
raw rights mask and flags, whose interpretation depends on the object kind.
`--list-kinds` describes all SDK object types. A newer kernel kind unknown to
this SDK is still listed as `unknown(0x...)`, preserving its numeric type ID.

The default service manifests grant the session and shell object inspector
`INSPECT | INSPECT_DETAILS | DUPLICATE | TRANSFER`. The shell adds an inspector
with exactly `INSPECT | INSPECT_DETAILS` only when the resolved executable path
is `/bin/handle` (including the bare command `handle`). A copy at another path
does not receive it. This is launcher policy for the trusted `/bin` namespace;
the kernel checks rights and scope, never the executable name. The task inspector
passed to `handle`, `ps`, and `top` retains basic `INSPECT` only. Neither inspector
passed to `handle` has transfer, duplicate, or derive rights.

An exact `--object KOID` registry query or `PROCESS --handle HANDLE` also prints:

- Thread: scheduler TID and role, plus user-thread lifecycle when available.
  Bootstrap TID zero is valid; a thread not yet assigned a TID says unavailable.
- VMAR: address range and liveness, then current and maximum `rwx` protections
  for mappings within that region. These protections are separate from handle
  rights. Destroyed regions retain their range but no live mappings.
- Byte/capability channel: peer KOID, open state and local queue counters.
  `handle` uses ordinary process/handle scans to list visible peer holders; the
  kernel performs no reverse owner lookup. No visible holder does not prove
  that the peer is unused: handles can be in transit or outside the scope.
- Physical device: profile, lifecycle, device identity, host IRQ domain and
  interrupt range, plus host physical resource ranges and guest aperture offsets.
  This reads captured metadata, never device registers or PCI configuration.

Other object kinds keep their basic record and report that details are not
available. Detailed reads require both rights on an **object inspector**, not
`INSPECT` on the target handle. Global KOID selection requires system scope;
process-local handle selection additionally checks that process against scope.
The existing inspector derivation calls return their original base rights and
never propagate `INSPECT_DETAILS`. Target handles remain generation-qualified,
and diagnostic pins do not revive retired handles or confer operation rights.

Launchers that supply only an object inspector can use registry and numeric
process queries within its scope. Task inspection supplies process names and
holder discovery. Basic-only inspectors can still request exact table rows with
`--no-headers`; detailed reads report access denied. The catalogs and `--help`
need no inspector capabilities.

Scans are capability-scoped and weakly consistent, not atomic system snapshots.
`--all --object KOID` finds visible **process handle holders**, not every kernel
reference. A process that exits or falls outside the object inspector's scope
while scanning is skipped with a warning on stderr; other scan errors fail the
command. Already printed rows remain valid observations of their capture time.
Transient inspection references and concurrent changes mean reference totals
and separate scans need not agree exactly. An empty holder list does not prove
that an object is unused or destroyed.

VMAR detail cursors scan at most 32 mapping slots per call, outside the address-space
lock after retaining its immutable mapping snapshot. Concurrent map changes can
skip or repeat mappings; a stale cursor fails rather than implying an atomic
snapshot. All detail callbacks run after registry and handle-table locks release.

`handle --summary` counts matching registry objects by kind. With a process or
`--all`, it reports matching handle counts and distinct object counts per kind;
duplicate handles to the same object count once in the object column. Filters
apply before aggregation, and `--no-headers` also works for summary output.
Counts describe the scan's visible observations, not an atomic system snapshot.

## Named virtual machines

```text
vmm list
vmm status alpine
vmm start alpine
vmm console alpine
vmm stop alpine
vmm restart alpine
vmm create test --config /data/new-vms.json
vmm create worker --config /data/new-vms.json --start
vmm delete test
```

`vmm` without a subcommand lists definitions. Every control operation requires
an explicit name from that list. Console attachment applies to that VM alone;
Ctrl-] opens the detach menu. Guest input uses nonblocking sends and a bounded
64 KiB pending buffer. If the guest stops consuming input, excess keyboard data
is discarded with a warning; the detach controls remain responsive. Detaching
discards input still buffered locally. Delete requires the VM to be stopped and removes
only its definition, leaving its image intact. Stop cancels a pending restart.
Start/restart acceptance means the lifecycle request was accepted; use status to
observe `starting`, `running`, `stopping`, `stopped`, or `failed`.

Use `vmm start alpine --wait`, `vmm stop alpine --wait` or
`vmm restart alpine --wait --timeout 60` to wait for `running`/`stopped`. The
default total deadline is 30 seconds, including command admission and replies.
A failed/unavailable VM, manager error or timeout produces a failing exit status.
Timeout does not cancel or undo the operation; inspect status before retrying.
A running state proves vCPU execution, not completion of Linux boot, DHCP or
application startup. Concurrent clients can still change the VM's state; waiting
observes manager state and does not acquire an exclusive lifecycle lock.
Ordinary command exchanges also have a 30-second response deadline; attached
console sessions remain interactive without that time limit.

Each VM owns a separate runtime process, resource domain, task group, lifecycle
tracker, and console session. The current policy allows eight definitions and budgets up to eight business
VM instances plus the resident I/O VM and manager overhead. Actual resource
admission may fail below that ceiling. Guest memory, vCPU count and
Linux boot arguments come exclusively from each definition's `configuration`.
The default Alpine configuration uses 256 MiB and two vCPUs on AArch64 (one on
RISC-V). AArch64 supports 1..8 vCPUs; RISC-V currently supports one. RAM must be
a power of two of at least 64 MiB, subject to host resource admission.
Set `configuration.vcpus` to `4` for a four-vCPU VM; the ITB is reusable without
repacking. The runtime places initramfs at the top of configured RAM and validates
kernel, initramfs and generated DTB ranges before allocating memory.
`allocated VM backing` measures resident physical backing, not memory used
inside Linux. Attaching an I/O-backed disk or network currently populates the
guest's entire RAM VMO. See [I/O allocation and zero-copy](io-vm.md#allocation-and-zero-copy-scope)
for the allocation policy and [memory observations](io-vm.md#memory-observations-and-queue-overhead)
for shared-memory accounting and queue costs.
Guest PSCI poweroff stops the instance cleanly; guest reboot creates a fresh
instance under the same definition and disconnects the old console session.

## Configuration

Service wiring in `/etc/hyper/services.json` selects a separate VM configuration:

```json
"virtual-machines": { "config": "/etc/hyper/vms.json" }
```

The VM file contains definitions, not service capabilities:

```json
{
  "format": "hyper.vm-config",
  "virtual-machines": [
    {
      "name": "alpine",
      "image": "/data/vm/alpine.itb",
      "autostart": true,
      "configuration": {
        "vcpus": 2,
        "memory-bytes": 268435456,
        "bootargs": "console=ttyAMA0 rdinit=/init loglevel=7 hyper.root=/dev/sda"
      },
      "disk": { "client": 1, "volume": "alpine" },
      "network": { "client": 1, "network": "default", "mac": "02:48:59:00:00:01" }
    }
  ]
}
```

Names are unique, at most 32 ASCII letters, digits, underscores, dots, or hyphens,
and start with a letter or digit. Images use absolute paths without parent
components or control characters. Unknown fields are errors. The manager validates
and opens every image before publishing a batch of definitions. If autostart
fails afterward, the definitions remain visible with the failed VM state.

Disk and network are optional. When both are present, they must use the same
I/O client. The complete client, volume, network and MAC binding must match
the board's bootstrap policy. The example above is the runtime format; board
JSON assigns client IDs when generating these definitions.

VM configuration is managed at deployment time. Board images provide
`/data/vms.json`; update the board's VM definitions before building the image.
`vmm create` and `vmm delete` only change the running manager and do not persist
across reboot. `vmm create NAME --config FILE` reads that named definition from
FILE; `--from SOURCE` copies another named definition under NAME. Disk/network
assignments and all runtime settings are copied together; `--start` starts the
new instance. Two definitions cannot share an I/O client, an exclusive disk
volume or a network MAC address. Native test images
use `app/init/tests/config/vms.json` (or `vms-riscv64.json`).

The `vcpus`, `memory-bytes` and `bootargs` fields are required. `bootargs` may be empty and is
limited to 2048 UTF-8 bytes without NUL. Architecture comes only from the ITB's
`arch` metadata (`arm64` or `riscv`); no architecture or platform belongs in JSON.
The manager rejects images whose architecture differs from the host before
publishing their definitions. The runtime repeats this check on every start,
before allocating guest RAM, and selects the corresponding reference platform.
Architecture-specific vCPU limits are checked against the image.
The manager snapshots definitions when it loads or creates them; editing the file
does not mutate a running VM. Reload a definition by stopping and deleting it,
then creating it from the updated file, or reboot the host.

An optional `configuration.affinity` list sets default placement for selected
vCPUs, using zero-based vCPU and host CPU indices. For example:

```json
"affinity": [
  { "vcpu": 0, "cpus": [0, 2] },
  { "vcpu": 1, "cpus": [1, 3] }
]
```

Each `cpus` list is the allowed host CPU set, not a preference order. Omit the
field, use `[]`, or leave individual vCPUs out to let the scheduler assign their
host CPUs automatically. Board examples omit this optional field by default.
Lists must be nonempty with unique CPU IDs in 0..255; each vCPU may
appear once and must be below `vcpus`. All specified masks are applied before
any vCPU starts, including for I/O VMs. A mask rejected by the host (for example,
one containing no schedulable CPU) fails startup. `vmm affinity` can still change
the current instance; restarting reapplies the saved definition's defaults.
For board deployments, edit the board JSON's VM `configuration` or `io-vm`
object; generated configuration files and ITBs need no manual edits. Ordinary
builds preserve an existing disk, including `/data/vms.json`; see
[updating existing deployments](board-storage.md#updating-existing-deployments)
to apply a changed configuration.

Migration: rebuild old ITBs with the current packer and add `configuration` to
existing JSON definitions. The v2 ITB contract rejects v1 bundles and embedded
runtime policy; it never falls back to values from an image.

Init waits for the manager to accept the complete fleet configuration, including
an empty fleet or definitions with no autostart VMs. It continues supervising
the manager as a critical service. Every guest lifecycle belongs to the manager:
an autostart or later runtime failure marks that VM failed without stopping init
or unrelated guests. Unavailable storage or rejected fleet configuration leaves
the Native services running. The manager continues answering inspection requests
but rejects VM creation and startup until a subsequent boot supplies a valid
configuration. Critical service termination and malformed provisioning protocol
still fail bootstrap; configuration unavailability is an explicit init request,
not an implicit interpretation of a closed channel.

## Validation

`make test-apps ARCH=aarch64` checks file tools, error statuses, batch monitoring,
two simultaneous VMs, name isolation, and create/delete/restart through
the real shell. It runs as part of Native CI, alongside the existing startup and
repeated runtime-crash cleanup tests.
`make test-fleet-config` checks empty and idle fleets, atomic configuration
rejection, and an autostart failure followed by a successful independent guest.

## Interactive transport

Session routing and shell output supervision use nonblocking byte-channel
relays. Each direction retains at most one message and rotates I/O priority.
A readiness notification is an observation, so handlers retry the complete wait
set when a subsequent nonblocking read finds no message. An unredirected
foreground child shares the terminal input endpoint with the shell; only an
actual read consumes input. This preserves typed-ahead commands when the child
does not read stdin. File redirections and pipelines use separate binary streams.
The shell drains child output before its next prompt and stops a supervised child
if its terminal connection fails.

Terminal stdin carries both `stdio.input` and a same-object
`stdio.terminal-input` capability. The runtime verifies their identity. The
console-input service splits command/EOF records and normalizes CR and CRLF to LF across
hardware reads; std terminal input also accepts CR and consumes standalone
Ctrl-D as EOF without closing the shared endpoint. Native channel readers such
as `vmm console` receive normalized line endings and the unchanged Ctrl-D byte. This interactive path is not a
binary serial tunnel or a full POSIX tty. Ordinary pipes and files preserve all
bytes. Default std child-process inheritance preserves terminal provenance.
See [the runtime contract](../sdk/lib/README.md) for compatibility details.

`make test-console ARCH=aarch64` checks typeahead bursts, terminal EOF and CRLF,
std child inheritance and binary pipes, randomized individually echoed keystrokes,
idle-to-input transitions, burst recovery, and returning from top and VM console.
`QEMU_CPUS` selects the CPU count; `CONSOLE_TYPED_ROUNDS` overrides the default
40 paced commands. Diagnostics are written to `target/app/aarch64/console.log`.

### Memory cache reporting

`free` retains the physical accounting identity `total = reserved + used + free`.
Its `cache` column combines unpinned clean VFS file pages and complete physical
pages recoverable by draining the allocator's CPU-local magazines. A magazine
page counts only when all outstanding central reservations are cached tokens:
a live caller or an in-flight token excludes it. Tokens distributed across
CPUs are combined and physical pages are counted once. These bytes are already
included in `used`; they are a reclaimability estimate, not additional memory.

The census uses bounded preallocated scratch storage, preserves the caches and
adds no global atomic operation to allocation fast paths. Magazine epochs,
updated under their existing locks, validate a coherent capture. If concurrent
mutation prevents validation after bounded retries, `cache` and `reclaimable`
show `—` rather than claiming a zero or counting a partial observation. Physical
`free` remains available independently. The snapshot is not a reservation against
subsequent allocations and does not guarantee contiguous allocation success.

`buffers` reports physical VFS cache payloads, including pages being loaded or
still pinned after eviction. These bytes overlap used kernel memory and the
unpinned subset reported as `cache`; the columns must not be summed. Cache
metadata remains ordinary heap memory. FAT uses the common clean file-page
cache, which grows with demand below managed-memory watermarks and reclaims
old pages asynchronously under pressure. Ramfs content is authoritative data,
not discardable cache, and bypasses this layer. The VFS census scans in bounded
lock intervals and rejects a sample if table replacement invalidates it;
concurrent reader pins may change after any observation.
Default units retain small KiB quantities; `free --bytes` avoids rounding.

## Console and storage visibility

The critical virtual console manager (`/svc/session`) starts and supervises the
ordinary `/bin/sh` client. Ctrl-D or `exit` ends that client and opens a fresh
shell on the same console. Init does not treat shell termination as a system
failure. Shell channels are recreated on each launch; the physical transport
continues to belong to the console services.

On board images, `vmm list` and `vmm status NAME` include the infrastructure I/O VM
as read-only, using `io-vm.name` from the board JSON (default `io`). The snapshot reports its lifecycle, vCPU count, guest memory and current pCPU
assignment of its boot vCPU;
an unavailable management endpoint is reported as `unavailable`, not `stopped`.
The manager holds a broker observation channel for this entry, not its VM handle
or runtime control channel. `start`, `stop`, `restart`, `delete`, `affinity` and
`console` require management authority and are rejected for observation-only
entries. Authorization follows the capability held for the resolved entry;
names are not reserved by service role. The owner reports the VM name and image path from its
loaded board configuration. After I/O readiness, the manager discovers that name over
the broker, and checks for conflicts when admitting definitions and before each
instance start. A failed observation blocks admission/start instead of treating
the observed namespace as empty. Without an I/O broker, `io` is an ordinary
managed VM name. The infrastructure VM's lifecycle remains under `io-runtime`
ownership.

For a running managed VM, `vmm affinity alpine 0 1,3` sets the allowed host
CPUs for guest vCPU 0. If its current CPU remains allowed, placement is unchanged;
otherwise the scheduler chooses an eligible CPU and performs the safe handoff.
CPU lists accept comma-separated IDs and inclusive ranges, such as `0,2-3`.
This changes affinity, not guest CPU topology, and does not enable automatic
load balancing. Affinity persists across guest CPU off/on cycles.

Acceptance can precede completion of a running vCPU's handoff. `vmm status alpine`
reports each vCPU's currently assigned physical CPU (`pCPU`). Assignment does not mean
the vCPU is executing at that instant. A control timeout reports an unknown
outcome rather than claiming the affinity update was rejected.

The configuration volume at `/data` uses FAT: long filenames preserve their
spelling, while lookup compares Unicode uppercase forms and also accepts ASCII
case-insensitive short-name aliases. Names differing only in letter case are
not distinct files. This differs from ramfs, whose names are case-sensitive.
Do not assume FAT reproduces every Linux VFAT Unicode/codepage corner case.
This change does not alter either filesystem's case semantics.

QEMU loads the hypervisor image directly from the host, so the configuration
volume contains guest artifacts and configuration, not `hyper.img`. Pi 5 still
needs its firmware boot files. Existing disks are intentionally preserved by
`make` and `make run`; use `make rebuild` to reset the disk and refresh its
packaged content. Normal `make` refreshes only the host-side kernel and ramdisk.

`vmm list` and `vmm status NAME` distinguish RAM capacity from **allocated VM
backing**. Allocation is a live kernel snapshot of resident primary backing:
RAM, uploaded image pages not yet accessed by the guest, and explicitly admitted
shared pools. The I/O VM currently has 128 MiB of boot RAM plus a separate 1 MiB
Native storage initiator pool, so its allocated backing can exceed the displayed
RAM capacity. Dynamically attached alias windows are not primary backing. The
metric excludes
the runtime process, page tables, and I/O VM mappings of other guests' memory;
it does not describe Linux's used/free memory. Repeated mappings of the same
backing range within a VM count once, while a page shared by different VMs is
attributed to each VM, so per-VM numbers must not be summed as system usage.

Inspection uses the runtime control connection and grants no additional VM
control rights. One command has a bounded observation budget shared by all
listed VMs. Starting, stopping, disconnected or busy runtimes may have unavailable
metrics; unavailable is never displayed as zero. Residency is sampled in short
locked chunks and can grow during a query, rather than freezing guest execution.
