<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Native applications

Applications use the installed SDK, Rust std, and clap. Run `APP --help` for
options (`echo` uses `-n`, `-e` and `-E`; its unknown options are literal text).
Ordinary commands, including formal `mount` and `umount`, live in `/bin`.
Bootstrap recovery uses `/svc/fallback-sh` and `/bin/{mount,umount}` before root
switching; the old archive remains visible under `/boot` until its references
allow retirement.
The independent system filesystem becomes `/` before formal init starts.
Bootstrap remains temporarily at `/boot`, then retires after provider adoption.
See [boot and system layout](boot-layout.md) for the handoff and mount policy.

## Service administration

`componentctl` queries component state and capability routes and accepts start/stop/restart requests.
`journalctl -c PATH -n 100 -f` follows attributed stdout/stderr records in the
volatile journal. See [service management](services.md) for permissions, component
configuration and failure semantics.

## Files and system information

| Command | Examples and behavior |
| --- | --- |
| `cat` | `cat FILE...`, `cat -n FILE`, `cat -bs FILE`, `cat -`; streams bytes without UTF-8 conversion. Number all lines with `-n`, only nonblank lines with `-b`, and squeeze repeated blank lines with `-s`. Numbering spans inputs. |
| `grep` | `grep -in PATTERN FILE`, `grep -F TEXT`, `grep -v PATTERN`; filters files or stdin, with counting, filename and quiet modes; `-m NUM` limits selected lines per input and `-x` matches whole lines. |
| `ls` | `ls /etc/hyper`, `ls FILE DIRECTORY`, `ls -a`, `ls --sort size -r`; defaults to permission mode, IEC size, and sorted names. Symlinks show `name@ -> target`, including dangling targets. `--bytes` gives exact file sizes; `-1` prints names only (retaining `@`, omitting targets), `-d` lists a directory itself, `-t` sorts newest first, and `-S` sorts largest first. Directory sizes are shown as `-`. |
| `ps` | `ps -T`, `ps -p KOID`, `ps --name vm-runtime`; select decimal/hex KOIDs (repeat `-p` or use commas) and intersect with a name substring, optionally including threads. IDs print in hexadecimal; `--no-headers` prints rows only. Thread rows include `process=NAME`; kernel-owned threads use `OWNER=kernel`, and per-CPU idle/timer threads are named `idle/CPU` and `ktimer/CPU`. |
| `free` | `free`, `free --bytes`, `free -m -c 5 -s 0.5`; physical totals, ownership and reclaimable cache pages. `-b/-k/-m/-g` select units; `-c` is a finite sample count (default 1), `-s` the interval. |
| `top` | `top -d 0.5`, `top -b -n 3`; interactive refresh with q/Ctrl-C to quit, or plain finite batch snapshots. Rows sort by CPU usage; `--sort name/koid`, `-p KOID`, `--name TEXT` and `--limit NUM` select the view. |
| `handle` | `handle shell`, `handle --kind physical-device`, `handle --all --object KOID`; inspect objects, process capabilities, and the processes holding an object. |
| `ldd` | `ldd /bin/vmm`, `ldd --tree /svc/vm-runtime`, `ldd --direct FILE`, `ldd -v FILE...`; inspect ELF interpreter and direct/transitive library dependencies without executing code. `--library-dir DIR` checks an alternate library set; unresolved or incompatible dependencies cause a failing exit status. |
| `echo` | `echo hello world`, `echo -n text`, `echo -e 'one\ntwo'`; `-n` suppresses the newline, `-e` enables escapes and `-E` restores literal backslashes. |

`cat`, `ls` and `ldd` report an error per failed path, continue with remaining paths,
and return failure if any path failed. The interactive shell merges unredirected
stdout and stderr into one terminal queue before displaying its next prompt.
`echo` is an ordinary application, not a second implementation inside the shell.

The shell supports concurrent pipelines and file redirects, for example
`cat /data/vms.json | grep image > /matches`. See
[shell and text filtering](shell.md) for syntax, limits and grep options.

Native std provides `std::os::hyper::fs::MetadataExt::mode()` for permission and
special mode bits. File type is queried separately. `metadata` follows symbolic
links; `symlink_metadata` inspects the final link itself.
`std::os::hyper::fs` supplies Native symlink and permission extensions.

`ldd /bin/vmm` uses the default system deployment: libraries come from
`/lib/<arch>-hyper-hyper`, and declared ELF interpreter paths are inspected literally.
Other interpreter names are inspected literally. `--loader` and `--library-dir`
select explicit locations when inspecting another deployment; these options
do not modify runtime loader policy. It checks dependency metadata and architecture/ELF ABI compatibility, not symbol bindings or
relocations. See [the command reference](../app/README.md#ldd-shared-library-dependency-inspection)
for scope and diagnostics.

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

The default service manifests grant the session manager and shell object inspector
`INSPECT | INSPECT_DETAILS | DUPLICATE | TRANSFER`. The shell adds an inspector
with exactly `INSPECT | INSPECT_DETAILS` only when the resolved executable path
is `/bin/handle` (including the bare command `handle`).
A copy at another path does not receive it. This is launcher policy for these
trusted command paths;
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
Ctrl-] opens the detach menu; `d`, `q`, or another Ctrl-] detaches. Any other
key prints an invalid-command notice and resumes the console without forwarding
that key to the guest. Guest input uses nonblocking sends and a bounded
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
tracker, and console session. The current policy allows eight definitions and
budgets up to eight business VM instances plus manager overhead in a dedicated
fleet domain. The resident I/O VM uses the separate core-services domain and
does not consume business fleet slots. Actual resource
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

The VM manager's service unit selects a separate VM configuration through
its `service.fleet` field (the unit directory comes from init configuration):

```json
"fleet": "/etc/hyper/vms.json"
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
use `bootstrap/stage1/init/tests/config/vms.json` (or `vms-riscv64.json`).

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

Init waits up to 60 seconds for the manager to accept the complete fleet
configuration, including an empty fleet or definitions with no autostart VMs.
The default manifest makes vm-manager noncritical: its startup failure, malformed
provisioning reply, or later exit cannot stop Native storage and core services.
A broken provisioning exchange stops that manager, while a rejected or unavailable
configuration leaves it running for inspection with VM admission disabled.
An omitted fleet configuration receives the same explicit unavailable response.
Each guest lifecycle belongs to the manager; an individual runtime failure marks
that VM failed without stopping init or unrelated guests.

I/O startup needs no business VM manager. Init creates the business resource
domain, task group and provisioning channel only when launching the manager.
Session's VMM client connection is separate from those resources. If no manager
is configured or its launch fails, init closes the unused server endpoint so
commands report unavailability promptly. The I/O broker remains a dependency of
business guest I/O; losing its manager peer stops new client admission without
stopping Native storage. A deployment can explicitly mark vm-manager critical
if it requires system-wide shutdown on that service's failure.

## Validation

`make test-apps ARCH=aarch64` checks file tools, error statuses, batch monitoring,
two simultaneous VMs, name isolation, and create/delete/restart through
the real shell. It runs as part of Native CI, alongside the existing startup and
repeated runtime-crash cleanup tests.
`make test-fleet-config` checks empty and idle fleets, atomic configuration
rejection, and an autostart failure followed by a successful independent guest.

## Interactive transport

Physical byte transport belongs to `console-raw`; formal line discipline belongs
to `terminal-service`. Session-manager supervises the terminal and shell, without
relaying input or output. The shell gives a scoped terminal lease to a command
with unredirected terminal stdin; in a pipeline this is the first stage.
It becomes active only after its Process observer is bound; termination restores
the parent settings and preserves unconsumed typeahead. File redirections and
pipeline edges remain separate binary streams.

Terminal stdin uses a connector and one private request/response connection per
process. Ordinary `Read` requests only the caller's byte count instead of
prefetching terminal input into Rust's global stdin buffer. The generic runtime
does not interpret CR or Ctrl-D: canonical EOF, conversion, raw binary handling
and read timing are implemented by the formal terminal. `top` and `vmm console`
select raw mode and use the same runtime input owner as `std::io`.
See [Native terminals](terminal.md) and the
[runtime contract](../stdlib/hyper/README.md).

`make test-console ARCH=aarch64` checks interactive shell typeahead, EOF,
std child inheritance, binary pipes, paced input, idle wakeups and return from
full-screen/VM clients. `make test-terminal ARCH=aarch64` additionally checks
terminal metadata, read timeouts, canonical EOF, all 256 raw byte values and
restoration after a raw-mode child exits. `terminal-probe` is included only in
development images. `QEMU_CPUS` selects the CPU count;
`CONSOLE_TYPED_ROUNDS` overrides the default 40 paced commands.

### Memory cache reporting

The kernel observation exposes raw physical allocations, without choosing the
`free` presentation policy: total RAM, allocator-managed RAM, free pages,
direct kernel pages, slab backing, large heap backing, page tables, VMO backing,
private guest backing, and file-cache backing. The physical allocation
categories are disjoint and captured under the allocator lock. Consumers derive
reserved RAM as `total - managed` and allocated RAM as `managed - free`.

`free` computes its main row in userspace:

- `reclaimable = file-reclaimable + slab-reclaimable`;
- `available = free + reclaimable`;
- `used = total - available`, including reserved RAM;
- consequently `total = used + free + reclaimable`.

`available` estimates recoverable space, without reserving pages or promising
contiguous allocations. File and slab censuses have independent validity. If
either cannot complete, the dependent main-row estimates show `—`. If the slab
census is unavailable, `kernel` and `buffered/cached` also show `—`, since their
boundary depends on reclaimable slab. Total, free, reserved, allocated, user,
guest and other remain available. A successful empty census is zero. Cache
reader pins can change after any observation.

The `Owners` row partitions managed allocations without double counting:

- `kernel = direct kernel pages + slab + large heap + page tables - slab-reclaimable`;
- `buffered/cached = file-cache backing + slab-reclaimable`;
- `user` and `guest` separate Native and guest backing as described below;
- `other` covers any allocated pages outside the raw ownership counters.

`kernel` represents allocations not eligible for cache reclamation at sampling
time; it does not imply permanent allocation. Kernel stacks, heap objects and
page tables can still be released when their owners exit or drop them. The
kernel image and early boot reservations remain in `reserved`.

VMO backing retained by an exclusive guest-memory lease is counted once in
`guest`, even when a backend aliases it or the lease is cloned. It is removed
from `user`; that field describes remaining VMO backing, not summed process
RSS. Guest accounting includes loaned transport buffers, not just advertised
RAM. Sparse uncommitted guest RAM counts as zero. Final lease release removes
guest use even if Native handles still retain the underlying VMO pages.

`Cache` separates unpinned clean file pages from complete slab pages recoverable
by draining CPU-local magazines. File-cache ownership also includes loading or
pinned pages, which cannot immediately be reclaimed. A slab page is recoverable
only if all its outstanding central reservations are cached tokens. The bounded
census validates magazine epochs and never drains caches to measure them.
Neither cache detail is an additional physical pool to add to `Owners`.

`Ramfs` shows a logical capacity metric within `kernel`. It sums writable ramfs
payload-capacity reservations, including transient replacement reservations.
It excludes allocator rounding, directory metadata, and borrowed boot-archive
contents. It overlaps heap backing and must not be added to physical totals.
Ramfs data is authoritative and cannot be discarded as cache. Borrowed
boot-archive storage remains in reserved RAM. File-cache index arrays and page
descriptors use a separate heap pool; their
physical backing is included in `buffered/cached`, not `kernel`. Cache totals
include mapped or pinned data and metadata; `file-reclaimable` remains a
conservative estimate of immediately discardable payload pages.

`top` reports total, allocated (including reserved), free and reserved bytes;
its allocated count includes reclaimable caches. Default units retain small
KiB quantities; `free --bytes` avoids rounding.

## Console and storage visibility

Stage-1 init owns raw recovery before handoff. Formal init owns errors after
handoff and launches session-manager for interactive service. Session-manager
restarts a fresh terminal/shell session after exit or failure, with bounded
retries and a delay. It revokes the old raw binding before stopping the terminal
and session groups, waits for retirement, discards stale input, then attaches a fresh epoch.
A missing formal shell never selects bootstrap recovery. The physical Console
transport and independent background log channels survive session restarts.

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
This changes affinity, not guest CPU topology. The kernel's automatic Fair
balancer may subsequently move a Ready vCPU within the allowed set; select one
CPU to prevent that movement. Affinity persists across guest CPU off/on cycles.

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
packaged content. Normal `make` refreshes host-side kernel, ramdisk and system
staging, then rejects a preserved disk whose system payload is stale.

`vmm list` and `vmm status NAME` distinguish RAM capacity from **allocated VM
backing**. Allocation is a live kernel snapshot of resident primary backing:
RAM, uploaded image pages not yet accessed by the guest, and explicitly admitted
shared pools. The I/O VM currently has 64 MiB of boot RAM, a 1 MiB management pool, and a
separate 1 MiB Native storage initiator pool for each Native volume. The default
split layout has two data pools, for `hyper-data` and `hyper-system`, so its
allocated backing is 67 MiB;
each additional Native volume adds a separate shared pool. Dynamically
attached alias windows are not primary backing. The
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
