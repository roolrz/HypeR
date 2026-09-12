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
| `ps` | `ps -T`, `ps -p KOID`, `ps --name vm-runtime`; select a process or filter names, optionally including threads. |
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
vmm create test --image /vm/alpine.itb
vmm create worker --image /vm/alpine.itb --start
vmm delete test
```

`vmm` without a subcommand lists definitions. Every control operation requires
an explicit name from that list. Console attachment applies to that VM alone;
Ctrl-] opens the detach menu. Delete requires the VM to be stopped and removes
only its definition, leaving its image intact. Stop cancels a pending restart.
Start/restart acceptance means the lifecycle request was accepted; use status to
observe `starting`, `running`, `stopping`, `stopped`, or `failed`.

Each VM owns a separate runtime process, resource domain, task group, lifecycle
tracker, and console session. The current policy allows eight definitions and
budgets two active VM instances. Guest memory and vCPU configuration still come
from the supported FIT image profile (currently 128 MiB and one vCPU); the CLI
has no misleading memory or CPU overrides.

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
    { "name": "alpine", "image": "/vm/alpine.itb", "autostart": true },
    { "name": "test", "image": "/vm/alpine.itb", "autostart": false }
  ]
}
```

Names are unique, at most 32 ASCII letters, digits, underscores, dots, or hyphens,
and start with a letter or digit. Images use absolute paths without parent
components or control characters. Unknown fields are errors. The manager validates
and opens every image before publishing a batch of definitions. If autostart
fails afterward, the definitions remain visible with the failed VM state.

`vmm load FILE` imports a config into the running manager. Existing names are
rejected; a rejected batch adds no definitions. `vmm create` changes the running
manager only. `vmm save FILE` writes its current definitions to a **new** config
file. It refuses to overwrite an existing configuration. Failed writes remove
the new file. Saved files can be
inspected with `cat` and later imported with `vmm load`.

The current filesystem is ramfs: all changes, including saved configurations,
disappear at reboot. To change the next image's boot configuration, edit
`app/init/config/vms.json` on the host and rebuild the initramfs. The first
autostart VM retains init's boot-critical supervision lease until it first stops;
later instances and additional VMs are supervised independently by the manager.

## Validation

`make test-apps ARCH=aarch64` checks file tools, error statuses, batch monitoring,
two simultaneous VMs, name isolation, create/delete, and config save/load through
the real shell. It runs as part of Native CI, alongside the existing startup and
repeated runtime-crash cleanup tests.

## Interactive transport

Session routing and shell child supervision use nonblocking byte-channel
relays. Each direction retains at most one message and rotates I/O priority;
input, stdout, and stderr continue independently under backpressure. A readiness
notification is an observation, so handlers retry the complete wait set when a
subsequent nonblocking read finds no message. The shell drains child output
before presenting the next prompt and stops a supervised child if its terminal
connection fails.

`make test-console ARCH=aarch64` checks randomized, individually echoed keystrokes,
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
