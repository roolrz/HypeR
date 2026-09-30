<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Native applications

Applications use the installed SDK, Rust std, and clap. Run `APP --help` for
options (echo treats option-looking arguments as literal text).

## Files and system information

| Command | Examples and behavior |
| --- | --- |
| `cat` | `cat FILE...`, `cat -n FILE`, `cat -`; streams bytes without UTF-8 conversion, with continuous optional line numbering. No files means stdin. |
| `grep` | `grep -in PATTERN FILE`, `grep -F TEXT`, `grep -v PATTERN`; filters files or stdin, with counting, filename and quiet modes. |
| `ls` | `ls /etc/hyper`, `ls FILE DIRECTORY`, `ls -a`, `ls --sort size -r`; defaults to permission mode, IEC size, and sorted names. `--bytes` gives exact file sizes; `-1` prints names only. Directory sizes are shown as `-`. |
| `ps` | `ps -T`, `ps -p KOID`, `ps --name vm-runtime`; select a process or filter names, optionally including threads. Thread rows include `process=NAME`; kernel-owned threads use `OWNER=kernel`, and per-CPU idle threads are named `idle/CPU`. |
| `free` | `free`, `free --bytes`; physical totals, ownership and reclaimable cache pages, in human-readable units or exact bytes. |
| `top` | `top -d 0.5`, `top -b -n 3`; interactive refresh with q/Ctrl-C to quit, or plain finite batch snapshots. |
| `handle` | `handle KOID`, `handle --objects --kind process`; inspect process capabilities or filter the object registry by its printed kind. |
| `echo` | `echo hello world`; prints arguments literally, followed by a newline. |

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
inside Linux. Attaching an I/O-backed disk currently populates the guest's
entire RAM VMO. See [I/O allocation and zero-copy](io-vm.md#allocation-and-zero-copy-scope)
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
      "disk": { "client": 1, "volume": "alpine" }
    }
  ]
}
```

Names are unique, at most 32 ASCII letters, digits, underscores, dots, or hyphens,
and start with a letter or digit. Images use absolute paths without parent
components or control characters. Unknown fields are errors. The manager validates
and opens every image before publishing a batch of definitions. If autostart
fails afterward, the definitions remain visible with the failed VM state.

VM configuration is managed at deployment time. Board images provide
`/data/vms.json`; update the board's VM definitions before building the image.
`vmm create` and `vmm delete` only change the running manager and do not persist
across reboot. `vmm create NAME --config FILE` reads that named definition from
FILE; `--from SOURCE` copies another named definition under NAME. Disk assignments
and all runtime settings are copied together; `--start` starts the new instance.
Exclusive disk assignments cannot be shared by two definitions. Native test images
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
object; generated configuration files and ITBs need no manual edits.

Migration: rebuild old ITBs with the current packer and add `configuration` to
existing JSON definitions. The v2 ITB contract rejects v1 bundles and embedded
runtime policy; it never falls back to values from an image.

Init waits for the manager to accept the complete fleet configuration, including
an empty fleet or definitions with no autostart VMs. It continues supervising
the manager as a critical service. Every guest lifecycle belongs to the manager:
an autostart or later runtime failure marks that VM failed without stopping init
or unrelated guests. Configuration admission errors still fail system startup.

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
Its `cache` column counts complete physical pages recoverable by draining the
allocator's CPU-local magazines. A page counts only when all its outstanding
central reservations are cached tokens: a live caller or an in-flight token
keeps it out of this total. Tokens distributed across CPUs are combined and
physical pages are counted once. These bytes are already included in `used`.

The census uses bounded preallocated scratch storage, preserves the caches and
adds no global atomic operation to allocation fast paths. Magazine epochs,
updated under their existing locks, validate a coherent capture. If concurrent
mutation prevents validation after bounded retries, `cache` and `reclaimable`
show `—` rather than claiming a zero or counting a partial observation. Physical
`free` remains available independently. The snapshot is not a reservation against
subsequent allocations and does not guarantee contiguous allocation success.

`buffers` represents an independent block-I/O buffer pool, currently zero because
no such pool exists. Ramfs content is authoritative data, not discardable cache.
The existing immutable file-page cache is bypassed by ramfs; no current backend
populates it. Enabling it for a future backend also requires integrating its
reclaimable storage into memory accounting.
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
