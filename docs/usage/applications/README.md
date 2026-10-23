<!--
SPDX-FileCopyrightText: 2026 roolrz
SPDX-License-Identifier: Apache-2.0
-->

# Native applications

Formal commands are installed under `/bin` on the independent system root.
Run `COMMAND --help` for command options; `echo` treats unknown options as text.
The shell also has builtins, including `echo` and `printf`.

| Task | Manual |
| --- | --- |
| Interactive commands, history, completion, scripts and redirection | [Shell](shell.md) |
| Edit a text file | [editor](editor.md) |
| Page through files or pipeline output | [less](less.md) |
| List, copy, rename, remove and search files | [File tools](files.md) |
| Mount and unmount authorized volumes | [mount and umount](mount.md) |
| Inspect processes and threads | [ps](ps.md) |
| Watch CPU and process activity | [top](top.md) |
| Read physical memory totals and cache accounting | [free](free.md) |
| Compare allocation/cache snapshots | [memstat](../diagnostics/memory-diagnostics.md) |
| Sample CPU activity or record kernel event timelines | [perf / flame graphs](../diagnostics/profiling.md), [perf trace / Perfetto](../diagnostics/tracing.md) |
| Inspect objects, handles and permissions | [handle](handle.md) |
| Inspect executable/shared-library dependencies | [ldd](ldd.md) |
| Start, stop, inspect or attach to a VM | [vmm](vmm.md) |
| Query service state, control components or read logs | [componentctl and journalctl](services.md) |

[Application I/O and authority](execution.md) explains terminal ownership,
standard streams and which diagnostic capabilities a launched command receives.
[Native application development](../../../app/README.md) covers the SDK and build
workspace; [component design](../../design/services/components.md) covers background
service configuration and lifecycle.

Bootstrap recovery is separate: stage-1 uses `/svc/fallback-sh` before ownership
transfer, while formal interactive sessions use `/bin/sh`. The old archive is
visible under `/boot` only until retirement. See
[boot and system layout](../../architecture/boot-layout.md).
