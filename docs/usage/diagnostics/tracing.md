<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Kernel timelines with Perfetto

`perf trace` records static kernel events into bounded CPU-local rings and exports
Chrome Trace Event JSON, which [Perfetto imports directly](https://perfetto.dev/docs/getting-started/other-formats).
Open the exported file at <https://ui.perfetto.dev>. PMU samples/flame graphs remain
available through the independent [PMU commands](profiling.md).

## Build and capture

Enable `kernel-trace` in the kernel feature menu and deploy `perf` in the system
application menu. The feature is off by default. It requires neither
`pmu-profiling` nor test features. For a QEMU development fixture:

```sh
make test-trace ARCH=aarch64
```

On HypeR, from the authorized formal shell:

```sh
perf trace record --duration-ms 5000 --categories scheduler,syscall,ipc,io \
  --cpus all --buffer-records 4096 --poll-ms 10 \
  --max-events 262144 --output /tmp/trace.json

perf trace record --duration-ms 10000 --categories scheduler,io \
  --output /tmp/storage.json -- /bin/storage-qual read /data/storage-qual.bin 101
```

The workload must be deployed separately. Recording stops when the command exits,
the recording deadline arrives, or the collector limit is reached. A running
command continues after the recording deadline; `perf` waits for it and propagates
its exit status. Children receive ordinary process capabilities, not the shell's
privileged-tool delegation; profile `vmm` with separate `start` and `dump` calls.
The deadline bounds tracing, not workload execution. Any
collector error closes recording and cleans up its own child.

For a retained-tail capture without a resident collector:

```sh
perf trace start --duration-ms 10000 --categories scheduler,vm --cpus 0-3
vmm list
perf trace status
perf trace dump --output /data/trace.json
```

`dump` stops recording before reading. `stop` is also available separately.
Both accept `--generation N` to reject an unexpected replacement capture.
Starting again discards the previous capture. Reading is non-destructive, so
another dump is possible until the next start. The kernel stops at the deadline
even if the collecting process exits unexpectedly.

`/tmp` is volatile. To export via a serial harness, use `cat /tmp/trace.json` after
capture; preserve the file bytes without prompts, terminal escape sequences or
concurrent logs. A writable persistent filesystem is also suitable. `-o -` emits
JSON on stdout, with diagnostics on stderr; it cannot be combined with a workload
command because child output would corrupt that stream.

## Capture controls

| Option | Default | Meaning |
| --- | --- | --- |
| `--duration-ms` | 10000 | Kernel-enforced duration, 1–300000 ms |
| `--categories` | `scheduler,vm,ipc,io` | Comma-separated categories below, or `all` |
| `--cpus` | `all` | Online indices/ranges, e.g. `0,2-3`; offline CPUs fail |
| `--buffer-records` | 4096 | Retained records per CPU, 64–4096 |
| `--poll-ms` | 10 | `record` drain interval, 1–1000 ms |
| `--max-events` | 262144 | Userspace event bound, 1–4000000 |
| `--output`, `-o` | `/tmp/trace.json` | File or `-` for stdout |
| `--format` | `perfetto` | Perfetto JSON or raw `jsonl` |
| `--no-names` | off | Skip best-effort live Thread name lookup |
| `--fail-on-loss` | off | Return failure if overwrite, incomplete stream or collector limit occurred; retain output |
| `--overwrite` | off | Explicitly allow replacing an existing file |

Options belong to `start`, `record` or `dump` as appropriate; use
`perf trace COMMAND --help` for the exact syntax. `status` shows the generation,
selected CPUs, kernel identity, deadline and buffer bounds. Captures are global,
not a per-process private resource. Start/stop require CpuInspector
SET_ATTRIBUTES; reads require INSPECT_DETAILS. Copying `perf` outside the trusted
`/bin/perf` launch path does not grant authority.

## Events and interpretation

| Category | Recorded observations |
| --- | --- |
| `scheduler` | Committed switches, wait-queue wakeups and migration publication |
| `syscall` | Ordinary Native dispatch entry/return, operation and object kind, signed status |
| `ipc` | Byte/capability channel operation outcomes, endpoint KOID, transferred length |
| `io` | Native block submissions, completion/abort, per-device trace identity, tag, bytes and direction |
| `memory` | Native page-fault handling entry/return and resolution |
| `irq` | IRQ service entry/return with interrupt ID |
| `vm` | vCPU run entry/return to its kernel runner with exit reason |

Events are scalar observations; they contain no kernel addresses, user payloads
or raw handle values. Successful receives report delivered bytes; failed receives report zero.
Opposite channel endpoints have different KOIDs;
these observations alone do not establish message-level causality. Immediate fast-path syscalls and trace-control
methods are excluded to avoid collection feedback. Other collector activity is
visible and can perturb the workload.
`syscall_exit` marks completion of ordinary dispatch: `returned=1` carries the
ABI status, including a successful yield; `returned=0` carries the requested
thread/process exit code. Subsequent yield scheduling or process teardown is
outside that dispatch interval.

CPU lanes show observed switch-to-switch execution intervals. Thread lanes show
elapsed syscall/fault/vCPU time and runnable latency. Elapsed time can include
sleep and descheduling; it is not CPU usage. vCPU boundaries are returns to the
runner, not every hardware exit handled by the HAL fast path. IRQ spans cover
ordinary routed service through acknowledgement, excluding scheduling postludes
and architecture-private RPC doorbells. Idle Threads
are named like other Threads. Thread states and scheduling classes are numeric
kernel enum values, interpreted against the included build identity.

Block spans and observed-outstanding counters describe Native block transport,
not Linux I/O VM or physical controller internals. Requests already in flight at
capture start are not included in the initial counter. Raw events, all scalar
arguments and source locations remain available by selecting an event. IDs are
hex strings to preserve 64-bit values in JavaScript; viewer lane numbers are
export-local identifiers. JSONL retains exact monotonic nanoseconds; Perfetto JSON
uses microseconds relative to capture start, retaining fractional precision.

## Bounds and loss

Compiled-in storage is 4096 × 56 bytes per configured CPU (224 KiB plus ring
headers), even with tracing disabled at runtime. Reducing `--buffer-records`
changes the overwrite window, not this reservation. Without `kernel-trace`,
record storage and instrumentation compile out. Runtime-disabled points only
check an atomic category flag; enabled points mask local IRQs briefly and use
try-only ring access, without allocation or cross-CPU producer locking.

The collector stores approximately 64 bytes per event plus interned names/sites.
It stops before exporting the file, avoiding export I/O inside the measured
interval. Large JSON output can be substantially larger than that compact
collection. Duration, ring capacity and event count are independent bounds.

Overwrites are counted per CPU. Contention/recursion/clock failure is a sticky
`incomplete` flag, not an exact dropped-event count. Loss is in capture metadata
and `TRACE LOSS`/`TRACE GAP` markers. Missing events invalidate pending spans;
any capture loss suppresses inferred cross-CPU durations, since a missing prefix
or tail on one CPU can hide intervening operations of a migrated thread.
CPU spans on an incomplete CPU are suppressed. Selecting only part of the online CPU set also
suppresses thread/I/O durations: a migrated thread may have unmatched boundaries
on an unselected CPU. Local CPU and IRQ intervals remain available. Neither unmatched starts nor truncated tails are
invented into complete intervals. Increase capacity, reduce categories or drain
more frequently to reduce overwrite; collecting itself also has overhead.

These are HypeR tracks in Perfetto's `slice`/`args` tables. They do not fabricate
Linux ftrace records or populate Linux-specific scheduler-analysis tables. No
live Perfetto daemon or tracing service is installed on HypeR.
